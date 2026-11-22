//! The offline process-tree fixture and the SQLite reads of its samples.
use crate::{
    journey::{self, Sandbox},
    runtime::{child_watchdog, instrumentation, Fixture},
};
use arc_core::process_identity::{probe_live, IdentityStatus, ProcessIdentity};
use rusqlite::{params, Connection};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

pub const PROGRAM: &str = "arc-tree-fixture";

/// Compile `tree.c` onto the sandbox PATH with a cleared environment.
pub fn compile(f: &Fixture, s: &Sandbox) {
    let source = f.root.join("tree.c");
    fs::write(&source, include_str!("tree.c")).unwrap();
    let mut compiler = Command::new("/usr/bin/cc");
    compiler
        .env_clear()
        .env("HOME", &s.home)
        .env("PATH", "/usr/bin:/bin");
    instrumentation::forward(&mut compiler);
    compiler
        .args(["-O1", "-Wall", "-Werror", "-o"])
        .arg(s.bin.join(PROGRAM))
        .arg(&source);
    let output = child_watchdog::output(&mut compiler, "tree fixture compiler").unwrap();
    assert!(output.status.success(), "{output:?}");
}

/// A fresh directory the fixture publishes into.
pub fn directory(f: &Fixture, name: &str) -> PathBuf {
    let path = f.root.join(name);
    fs::create_dir_all(&path).unwrap();
    path
}

/// A value the fixture published by rename.
pub fn published(directory: &Path, name: &str) -> u64 {
    let mut value = None;
    journey::eventually(&format!("fixture {name}"), || {
        value = fs::read_to_string(directory.join(name))
            .ok()
            .and_then(|text| text.trim().parse().ok());
        value.is_some()
    });
    value.unwrap()
}

pub fn release(directory: &Path) {
    fs::write(directory.join("release"), "").unwrap();
}

/// Kill a fixture process this test does not reap, only while it is still
/// the same process (never a recycled PID).
pub struct Reaped(pub ProcessIdentity);
impl Reaped {
    pub fn capture(pid: u64) -> Self {
        Self(ProcessIdentity::capture(pid as u32).unwrap())
    }
}
impl Drop for Reaped {
    fn drop(&mut self) {
        if probe_live(&self.0) == IdentityStatus::VerifiedMatch {
            // SAFETY: a plain signal to a verified fixture process.
            unsafe { libc::kill(self.0.pid as libc::pid_t, libc::SIGKILL) };
        }
    }
}

/// The RUNNING job whose recorded child is `pid`.
pub fn running_job(db: &Connection, pid: u64) -> String {
    let mut job = None;
    journey::eventually("job RUNNING with its recorded child", || {
        job = db
            .query_row(
                "SELECT j.id FROM jobs j JOIN job_lifecycle l ON l.job_id=j.id
                 WHERE j.state='RUNNING' AND l.child_pid=?1",
                [pid as i64],
                |row| row.get(0),
            )
            .ok();
        job.is_some()
    });
    job.unwrap()
}

/// One stored `resource_samples` row.
#[derive(Debug, Clone, PartialEq)]
pub struct Row {
    pub timestamp_ns: u64,
    pub identity: ProcessIdentity,
    pub cpu_time_ns: u64,
    pub rss_bytes: u64,
    pub disk_read_bytes: u64,
    pub disk_write_bytes: u64,
    pub confidence: String,
    pub cpu_core_percent: Option<f64>,
}

pub fn rows(db: &Connection, job: &str) -> Vec<Row> {
    let mut statement = db
        .prepare(
            "SELECT timestamp_ns,process_identity,cpu_time_ns,rss_bytes,disk_read_bytes,
                    disk_write_bytes,attribution_confidence,cpu_core_percent
             FROM resource_samples WHERE job_id=?1
             ORDER BY CAST(timestamp_ns AS INTEGER),id",
        )
        .unwrap();
    statement
        .query_map([job], |row| {
            let text = |i| row.get::<_, String>(i);
            Ok(Row {
                timestamp_ns: text(0)?.parse().unwrap(),
                identity: serde_json::from_str(&text(1)?).unwrap(),
                cpu_time_ns: text(2)?.parse().unwrap(),
                rss_bytes: row.get::<_, i64>(3)? as u64,
                disk_read_bytes: row.get::<_, i64>(4)? as u64,
                disk_write_bytes: row.get::<_, i64>(5)? as u64,
                confidence: text(6)?,
                cpu_core_percent: row.get(7)?,
            })
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

/// Every PID any job's samples name.
pub fn sampled_pids(db: &Connection) -> Vec<u32> {
    let mut statement = db
        .prepare("SELECT json_extract(process_identity,'$.pid') FROM resource_samples")
        .unwrap();
    statement
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

/// Simulate a recycled PID: the recorded child identity of a RUNNING job now
/// names another process (same PID, another start time). Returns the true
/// token and the job's sample count at that commit.
pub fn recycle(db: &mut Connection, job: &str) -> (String, i64) {
    let tx = db.transaction().unwrap();
    let token: String = tx
        .query_row(
            "SELECT child_start_token FROM job_lifecycle WHERE job_id=?1",
            [job],
            |row| row.get(0),
        )
        .unwrap();
    let other = (token.parse::<u64>().unwrap() + 1).to_string();
    tx.execute(
        "UPDATE job_lifecycle SET child_start_token=?2 WHERE job_id=?1",
        params![job, other],
    )
    .unwrap();
    let count = tx
        .query_row(
            "SELECT COUNT(*) FROM resource_samples WHERE job_id=?1",
            [job],
            |row| row.get(0),
        )
        .unwrap();
    tx.commit().unwrap();
    (token, count)
}

pub fn restore(db: &Connection, job: &str, token: &str) {
    db.execute(
        "UPDATE job_lifecycle SET child_start_token=?2 WHERE job_id=?1",
        params![job, token],
    )
    .unwrap();
}

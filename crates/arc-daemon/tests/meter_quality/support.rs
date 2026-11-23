//! The offline quality fixture, native allocated-byte observations and the
//! SQLite reads of the `meter_quality` target.
use crate::{
    journey::{self, Sandbox},
    runtime::{child_watchdog, instrumentation, Fixture},
};
use rusqlite::Connection;
use serde_json::Value;
use std::{
    collections::HashSet,
    fs,
    io::Write,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    process::Command,
};

pub const PROGRAM: &str = "arc-quality-fixture";
pub const MIB: u64 = 1 << 20;

/// Compile `quality.c` onto the sandbox PATH with a cleared environment.
pub fn compile(f: &Fixture, s: &Sandbox) {
    let source = f.root.join("quality.c");
    fs::write(&source, include_str!("quality.c")).unwrap();
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
    let output = child_watchdog::output(&mut compiler, "quality fixture compiler").unwrap();
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

/// A synced file of `bytes` bytes.
pub fn write_synced(path: &Path, bytes: u64) {
    let mut file = fs::File::create(path).unwrap();
    file.write_all(&vec![0x5a; bytes as usize]).unwrap();
    file.sync_all().unwrap();
}

/// The test's own native observation of the allocated bytes under `root`:
/// `st_blocks * 512` of every entry by lstat, on the root's device, each
/// inode once. Independent of `arc_platform::footprint`.
pub fn allocated(root: &Path) -> u64 {
    let top = fs::symlink_metadata(root).unwrap();
    let device = top.dev();
    let mut seen = HashSet::new();
    let mut total = 0;
    let mut pending = vec![(root.to_path_buf(), top)];
    while let Some((path, metadata)) = pending.pop() {
        if metadata.dev() != device || !seen.insert(metadata.ino()) {
            continue;
        }
        total += metadata.blocks() * 512;
        if metadata.file_type().is_dir() {
            for entry in fs::read_dir(&path).unwrap() {
                let entry = entry.unwrap().path();
                let metadata = fs::symlink_metadata(&entry).unwrap();
                pending.push((entry, metadata));
            }
        }
    }
    total
}

/// The job whose recorded child is `pid`, once it is RUNNING.
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

/// `(sample count, largest disk_write_bytes)` stored for `job`.
pub fn samples(db: &Connection, job: &str) -> (u64, u64) {
    db.query_row(
        "SELECT count(*),COALESCE(max(disk_write_bytes),0) FROM resource_samples WHERE job_id=?1",
        [job],
        |r| Ok((r.get::<_, i64>(0)? as u64, r.get::<_, i64>(1)? as u64)),
    )
    .unwrap()
}

/// Every raw sample as `(job, wall ms)`.
pub fn sample_times(db: &Connection) -> Vec<(String, u64)> {
    db.prepare(
        "SELECT job_id,CAST(timestamp_ns AS INTEGER)/1000000 FROM resource_samples
         WHERE job_id IS NOT NULL AND length(timestamp_ns)=19",
    )
    .unwrap()
    .query_map([], |r| Ok((r.get(0)?, r.get::<_, i64>(1)? as u64)))
    .unwrap()
    .map(Result::unwrap)
    .collect()
}

/// The `job_count` of one rollup bucket.
pub fn job_count(db: &Connection, granularity: &str, bucket: u64) -> Option<i64> {
    db.query_row(
        "SELECT job_count FROM resource_rollups WHERE granularity=?1 AND bucket_start_ms=?2",
        rusqlite::params![granularity, bucket as i64],
        |r| r.get(0),
    )
    .ok()
}

/// Built `arcctl` with `args` in `cwd`, which must succeed with one JSON
/// document on stdout and nothing on stderr.
pub fn arcctl_json(f: &Fixture, s: &Sandbox, cwd: &Path, args: &[&str]) -> Value {
    let output = child_watchdog::output(s.arcctl(f, cwd).args(args), "arcctl read").unwrap();
    assert!(output.status.success(), "{args:?}: {output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    serde_json::from_slice(&output.stdout).unwrap()
}

/// `arcctl meter summary --by <by>` over `range` in hour buckets.
pub fn summary(f: &Fixture, s: &Sandbox, cwd: &Path, by: &str, range: (u64, u64)) -> Value {
    let (from, to) = (range.0.to_string(), range.1.to_string());
    let value = arcctl_json(
        f,
        s,
        cwd,
        &[
            "meter", "summary", "--by", by, "--from", &from, "--to", &to, "--bucket", "hour",
            "--json",
        ],
    );
    assert_eq!(value["schema_version"], 1, "{value}");
    value
}

/// The one group of `summary` whose `key` is `value`.
#[track_caller]
pub fn group<'a>(summary: &'a Value, key: &str, value: &str) -> &'a Value {
    let found: Vec<_> = summary["groups"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|g| g[key] == value)
        .collect();
    assert_eq!(found.len(), 1, "{key}={value}: {summary}");
    found[0]
}

/// Every flagged value is consistent: availability is one of the three,
/// `null` exactly when unavailable and then with a reason, and a basis.
#[track_caller]
pub fn assert_flagged(label: &str, value: &Value) {
    let availability = value["availability"]
        .as_str()
        .unwrap_or_else(|| panic!("{label}: {value}"));
    assert!(
        ["complete", "partial", "unavailable"].contains(&availability),
        "{label}: {value}"
    );
    assert!(
        !value["basis"].as_str().unwrap().is_empty(),
        "{label}: {value}"
    );
    let unavailable = availability == "unavailable";
    assert_eq!(value["value"].is_null(), unavailable, "{label}: {value}");
    assert_eq!(value["reason"].is_string(), unavailable, "{label}: {value}");
}

/// The metered values of every v2 aggregate.
pub const FLAGGED: [&str; 6] = [
    "cpu_core_seconds",
    "job.duration_ms",
    "memory_peak_bytes",
    "disk_read_bytes",
    "disk_write_bytes",
    "footprint_delta_bytes",
];

/// Every group of `summary` flags every metered value on both sides.
#[track_caller]
pub fn assert_all_flagged(summary: &Value) {
    for group in summary["groups"].as_array().unwrap() {
        for side in ["local", "remote"] {
            for metric in FLAGGED {
                assert_flagged(&format!("{side} {metric} of {group}"), &group[side][metric]);
            }
        }
    }
}

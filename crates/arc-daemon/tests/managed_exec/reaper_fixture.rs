//! Shared harness for the bounded Reaper acceptance tests.
//!
//! The Reaper fixture daemon shares production startup, IPC, storage,
//! validation and the wrapper round trip. A test controls only its clock, when
//! a pass runs, and explicitly named injected evidence faults.
use super::{cli, protocol, runtime};
use arc_core::process_identity::ProcessIdentity;
use arc_ipc::jobs::{JobId, Phase};
use std::{
    fs,
    os::unix::net::UnixListener,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

/// SRS 12.2: an ended session's orphan becomes eligible after ten minutes.
pub const SESSION_GRACE_MS: u64 = 600_000;

pub fn until<T>(label: &str, mut observe: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(value) = observe() {
            return value;
        }
        assert!(Instant::now() < deadline, "{label}: 60s watchdog expired");
        thread::sleep(Duration::from_millis(1));
    }
}

fn publish(path: &Path, value: u64) {
    let pending = path.with_extension("tmp");
    fs::write(&pending, value.to_string()).unwrap();
    fs::rename(pending, path).unwrap();
}

/// The fixture daemon with the bounded Reaper explicitly configured.
pub struct Reaper {
    pub f: runtime::Fixture,
    pub control: PathBuf,
    pub daemon: runtime::OwnedChild,
    sequence: u64,
}

impl Reaper {
    pub fn start() -> Self {
        let f = runtime::Fixture::with_binary(PathBuf::from(env!(
            "CARGO_BIN_EXE_arc-reaper-daemon-fixture"
        )));
        let control = f.root.join("reaper");
        fs::create_dir(&control).unwrap();
        publish(&control.join("clock"), 1_000);
        let mut command = f.isolated_command("test");
        command
            .env("ARC_REAPER", "process")
            .arg("--fixture-control")
            .arg(&control);
        let mut daemon = runtime::OwnedChild::new(command.spawn().unwrap());
        f.wait_for(&mut daemon, "Sweeper off. Process Reaper on.");
        Self {
            f,
            control,
            daemon,
            sequence: 0,
        }
    }
    pub fn clock(&self, now_ms: u64) {
        publish(&self.control.join("clock"), now_ms);
    }
    /// Run exactly one pass and wait until the daemon reports it finished.
    pub fn pass(&mut self) {
        self.sequence += 1;
        publish(&self.control.join("pass"), self.sequence);
        let done = self.control.join("done");
        until("Reaper pass", || {
            let finished = fs::read_to_string(&done).ok()?.trim().parse::<u64>().ok()?;
            (finished == self.sequence).then_some(())
        });
    }
    pub fn database(&self) -> rusqlite::Connection {
        rusqlite::Connection::open(&self.f.database).unwrap()
    }
    /// Recorded decisions for one job, oldest first: (state, reason, observed).
    pub fn decisions(&self, job: &JobId) -> Vec<(String, String, u64)> {
        self.database()
            .prepare(
                "SELECT g.state,g.error_code,g.started_at FROM gc_operations g \
                 JOIN resource_objects r ON r.id=g.resource_id \
                 WHERE r.resource_identity=?1 ORDER BY g.rowid",
            )
            .unwrap()
            .query_map([job.as_str()], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get::<_, i64>(2)? as u64))
            })
            .unwrap()
            .map(Result::unwrap)
            .collect()
    }
    pub fn resource(&self, job: &JobId) -> Option<(String, String)> {
        use rusqlite::OptionalExtension;
        self.database()
            .query_row(
                "SELECT ownership_level,state FROM resource_objects WHERE resource_identity=?1",
                [job.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .unwrap()
    }
    pub fn stop(mut self) {
        self.daemon.stop(libc::SIGTERM);
        self.f.report();
    }
}

pub fn managed(f: &runtime::Fixture, control: &Path, home: &Path) -> Command {
    let mut command = Command::new(cli::binary("arcctl"));
    isolate(&mut command, f, control, home);
    command
        .args(["exec", "--"])
        .arg(cli::binary("arc-exec-fixture"));
    command
}

pub fn isolate(command: &mut Command, f: &runtime::Fixture, control: &Path, home: &Path) {
    command
        .env_clear()
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join("xdg"))
        .env("PATH", "/usr/bin:/bin")
        .env(arc_core::ARC_SOCKET_PATH, &f.socket)
        .env(arc_core::ARC_DB_PATH, &f.database)
        .env("ARC_FIXTURE_MODE", "echo")
        .env("ARC_FIXTURE_CONTROL", control)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    runtime::instrumentation::forward(command);
}

pub fn running(f: &runtime::Fixture, child: &ProcessIdentity) -> arc_ipc::jobs::JobSnapshot {
    until("managed child attached", || {
        let job = protocol::status(f, &cli::recent(f));
        (job.phase == Phase::Running && job.child.as_ref() == Some(child)).then_some(job)
    })
}

// On unwind, terminate only the fixture-owned wrapper. Its native waiter owns
// and forwards to the child; the test never signals an observed child PID.
pub struct ManagedCli {
    pub child: runtime::OwnedChild,
    pub completed: bool,
}

impl Drop for ManagedCli {
    fn drop(&mut self) {
        if self.completed {
            return;
        }
        if matches!(self.child.child.try_wait(), Ok(None)) {
            let result = unsafe { libc::kill(self.child.child.id() as i32, libc::SIGTERM) };
            if result == 0 {
                if let Err(error) = runtime::child_watchdog::wait_child(
                    &mut self.child.child,
                    "managed fixture cleanup",
                ) {
                    eprintln!("managed fixture cleanup: {error}");
                }
            }
        }
    }
}

/// Start a registered wrapper under a launcher that ends when its standard
/// input closes, and return (launcher, child control stream, running job).
pub fn orphanable(
    reaper: &Reaper,
    mode: &str,
) -> (
    runtime::OwnedChild,
    std::os::unix::net::UnixStream,
    arc_ipc::jobs::JobSnapshot,
) {
    let home = reaper.f.root.join("cli-home");
    fs::create_dir(&home).unwrap();
    let control = reaper.f.root.join("managed-child.sock");
    let listener = UnixListener::bind(&control).unwrap();
    let mut launcher = Command::new("/bin/sh");
    isolate(&mut launcher, &reaper.f, &control, &home);
    launcher
        .env("ARC_FIXTURE_MODE", mode)
        .args([
            "-c",
            r#""$0" exec -- "$1" </dev/null >/dev/null 2>&1 & read _"#,
        ])
        .arg(cli::binary("arcctl"))
        .arg(cli::binary("arc-exec-fixture"))
        .stdin(Stdio::piped());
    let launcher = runtime::OwnedChild::new(launcher.spawn().unwrap());
    // Closing this stream on unwind ends the fixture child, and the wrapper
    // then exits with it; the test never signals an observed child PID.
    let (gate, message) = cli::accept(&listener);
    let child: ProcessIdentity = serde_json::from_value(message["identity"].clone()).unwrap();
    let job = running(&reaper.f, &child);
    assert_eq!(message["ppid"].as_u64(), Some(job.observer.pid as u64));
    until("verified registration", || {
        (reaper.resource(&job.job_id)? == ("PROVEN".to_owned(), "TRACKED".to_owned())).then_some(())
    });
    (launcher, gate, job)
}

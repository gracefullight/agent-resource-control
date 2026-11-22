//! A real daemon, built `arcctl exec` wrappers and offline stub children.
//!
//! `World` owns one isolated daemon and can restart it two ways: after a
//! forced kill, and after a normal stop with the database stepped back to an
//! earlier schema version. `Job` is one wrapper whose stub child, once
//! admitted and spawned, waits on a barrier this test holds, so a job can be
//! kept RUNNING with an ACTIVE permit for as long as a test needs it.
use crate::{
    journey::{self, eventually, output_with_input, Sandbox},
    native_exec,
    runtime::{child_watchdog::Owned, Fixture, OwnedChild},
    schema_step_back,
};
use arc_core::process_identity::{probe_live, IdentityStatus, ProcessIdentity};
use std::{
    io::Write,
    os::unix::net::{UnixListener, UnixStream},
    path::{Path, PathBuf},
    process::Stdio,
};

pub struct World {
    // Declared first: the daemon is reaped before its fixture directory goes.
    daemon: Option<OwnedChild>,
    pub s: Sandbox,
    pub f: Fixture,
    pub project: PathBuf,
    runs: usize,
    jobs: usize,
}

impl World {
    pub fn new() -> Self {
        let f = Fixture::new();
        let s = Sandbox::new(&f);
        s.stub("cargo");
        let project = s.project("project");
        let daemon = Some(f.spawn_isolated(""));
        Self {
            daemon,
            s,
            f,
            project,
            runs: 0,
            jobs: 0,
        }
    }

    pub fn db(&self) -> rusqlite::Connection {
        journey::database(&self.f)
    }

    /// `arcctl exec -- cargo <subcommand>` whose stub child reports to a
    /// barrier and exits 0 only when released.
    pub fn start(&mut self, subcommand: &str) -> Job {
        self.spawn(subcommand, true)
    }

    /// The same wrapper with no `cargo` on its PATH: its one spawn fails.
    pub fn start_unspawnable(&mut self, subcommand: &str) -> Job {
        self.spawn(subcommand, false)
    }

    fn spawn(&mut self, subcommand: &str, spawnable: bool) -> Job {
        self.jobs += 1;
        let base = self.f.root.join(format!("job-{}", self.jobs));
        let (marker, stderr, control) = (
            base.with_extension("starts"),
            base.with_extension("stderr"),
            base.with_extension("control"),
        );
        let listener = UnixListener::bind(&control).unwrap();
        let mut command = self.s.arcctl(&self.f, &self.project);
        command
            .args(["exec", "--", "cargo", subcommand])
            .env("ARC_FIXTURE_MODE", "success")
            .env("ARC_FIXTURE_MARKER", &marker)
            .env("ARC_FIXTURE_CONTROL", &control)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(std::fs::File::create(&stderr).unwrap());
        if !spawnable {
            let empty = self.f.root.join("no-tools");
            std::fs::create_dir_all(&empty).unwrap();
            command.env("PATH", empty);
        }
        let wrapper = Owned::spawn(&mut command).unwrap();
        let observer = ProcessIdentity::capture(wrapper.child.id()).ok();
        Job {
            wrapper,
            observer,
            listener,
            barrier: None,
            child: None,
            marker,
            stderr,
            id: None,
        }
    }

    /// Stop the daemon and check its exit report. A daemon writes the report
    /// once and never over an existing one, so it is set aside.
    pub fn stop(&mut self) {
        let mut daemon = self.daemon.take().expect("a running daemon");
        daemon.stop(libc::SIGTERM);
        self.f.report();
        self.set_report_aside();
    }

    /// SIGKILL the daemon: no shutdown path runs and no report is written.
    pub fn kill(&mut self) {
        use std::os::unix::process::ExitStatusExt;
        let mut daemon = self.daemon.take().expect("a running daemon");
        daemon.child.kill().unwrap();
        let status =
            crate::runtime::child_watchdog::wait_child(&mut daemon.child, "killed daemon").unwrap();
        assert_eq!(status.signal(), Some(libc::SIGKILL), "{status:?}");
        self.set_report_aside();
    }

    fn set_report_aside(&mut self) {
        self.runs += 1;
        let report = self.f.root.join("runtime.json");
        if report.exists() {
            std::fs::rename(
                report,
                self.f.root.join(format!("runtime-{}.json", self.runs)),
            )
            .unwrap();
        }
    }

    /// Start the current daemon binary on the database as it now is.
    pub fn boot(&mut self) {
        assert!(self.daemon.is_none(), "the previous daemon still runs");
        self.daemon = Some(self.f.spawn_isolated(""));
    }

    /// Stop the daemon and put its database
    /// into the shape of schema `version`. `boot` then starts the current
    /// daemon binary on it, which has to migrate before it serves.
    pub fn stop_and_step_back(&mut self, version: u32) {
        self.stop();
        schema_step_back::step_back(&self.f.database, version);
    }

    /// Any built `arcctl` read, isolated like every wrapper.
    pub fn arcctl(&self, args: &[&str]) -> std::process::Output {
        let mut command = self.s.arcctl(&self.f, &self.s.home);
        command.args(args);
        output_with_input(&mut command, b"", "arcctl read")
    }

    /// `arcctl job status <id>`: the daemon reconciles before it answers.
    pub fn status(&self, id: &str) -> String {
        let output = self.arcctl(&["job", "status", id]);
        assert!(
            output.status.success() && output.stderr.is_empty(),
            "{output:?}"
        );
        String::from_utf8(output.stdout).unwrap()
    }

    /// Stop the daemon for the last time; its report must be clean.
    pub fn finish(mut self) {
        self.stop();
    }
}

pub struct Job {
    pub wrapper: Owned,
    /// The wrapper's identity, when it lived long enough to be captured.
    pub observer: Option<ProcessIdentity>,
    listener: UnixListener,
    barrier: Option<UnixStream>,
    /// The stub child's own report of its PID and start time.
    pub child: Option<ProcessIdentity>,
    pub marker: PathBuf,
    pub stderr: PathBuf,
    id: Option<String>,
}

impl Job {
    pub fn pid(&self) -> u32 {
        self.wrapper.child.id()
    }

    /// The job row of this wrapper, once the daemon recorded it.
    pub fn id(&mut self, db: &rusqlite::Connection) -> Option<String> {
        if self.id.is_none() {
            self.id = match &self.observer {
                Some(observer) => db
                    .query_row(
                        "SELECT job_id FROM job_lifecycle WHERE observer_pid=?1 AND observer_start_token=?2",
                        (observer.pid, &observer.start_token),
                        |row| row.get(0),
                    )
                    .ok(),
                None => db
                    .query_row(
                        "SELECT job_id FROM job_lifecycle WHERE observer_pid=?1",
                        [self.pid()],
                        |row| row.get(0),
                    )
                    .ok(),
            };
        }
        self.id.clone()
    }

    /// The queue state of this job's entry.
    pub fn state(&mut self, db: &rusqlite::Connection) -> Option<String> {
        let id = self.id(db)?;
        db.query_row(
            "SELECT state FROM traffic_entries WHERE job_id=?1",
            [id],
            |row| row.get(0),
        )
        .ok()
    }

    /// Wait until the stub child reached its barrier, however it was started.
    pub fn reach_barrier(&mut self) -> ProcessIdentity {
        let (barrier, report) = native_exec::accept(&self.listener);
        let child: ProcessIdentity = serde_json::from_value(report["identity"].clone()).unwrap();
        self.barrier = Some(barrier);
        self.child = Some(child.clone());
        child
    }

    /// Wait until the stub child reached its barrier and the daemon recorded
    /// that very process as the job's child: RUNNING, permit ACTIVE.
    pub fn running(&mut self, db: &rusqlite::Connection) -> ProcessIdentity {
        let child = self.reach_barrier();
        let label = format!("job of wrapper {} attached", self.pid());
        eventually(&label, || {
            if let Some(status) = self.wrapper.child.try_wait().unwrap() {
                panic!(
                    "{label}: wrapper left with {status:?}: {:?}",
                    std::fs::read_to_string(&self.stderr)
                );
            }
            self.recorded_child(db) == Some((child.pid, child.start_token.clone()))
                && self.state(db).as_deref() == Some("RUNNING")
        });
        child
    }

    /// The child the daemon holds for this job: PID and start time.
    pub fn recorded_child(&mut self, db: &rusqlite::Connection) -> Option<(u32, String)> {
        let id = self.id(db)?;
        db.query_row(
            "SELECT child_pid,child_start_token FROM job_lifecycle WHERE job_id=?1 AND child_pid IS NOT NULL",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .ok()
    }

    /// Let the stub child exit 0.
    pub fn release(&mut self) {
        let mut barrier = self.barrier.take().expect("a child at its barrier");
        barrier.write_all(b"x").unwrap();
    }

    /// SIGKILL the wrapper alone and reap it; a spawned child lives on.
    pub fn kill_wrapper(&mut self) {
        self.wrapper.child.kill().unwrap();
        self.wrapper.wait("killed wrapper").unwrap();
    }

    /// Wait until the released child is gone from the process table.
    pub fn child_gone(&self) {
        let child = self.child.as_ref().expect("a spawned child");
        eventually("released child exited", || {
            matches!(
                probe_live(child),
                IdentityStatus::Missing | IdentityStatus::Mismatch
            )
        });
    }

    pub fn starts(&self) -> usize {
        journey::starts(&self.marker)
    }

    pub fn diagnostics(&self) -> String {
        std::fs::read_to_string(&self.stderr).unwrap_or_default()
    }
}

/// Whether the process `identity` names is alive now.
pub fn alive(identity: &ProcessIdentity) -> bool {
    probe_live(identity) == IdentityStatus::VerifiedMatch
}

pub fn file(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

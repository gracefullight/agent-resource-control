//! A real daemon held at its fast sampling cadence by one managed job, and the
//! test's own OS readings interleaved with its Meter snapshots.
//!
//! Every ordering used by an assertion is derived from the snapshot
//! `sequence`, never from a clock: the sleeps here only pace the polling and
//! the deadlines are diagnostic watchdogs.
use crate::{
    journey::{self, Sandbox},
    native, native_exec, os,
    runtime::{child_watchdog::Owned, Fixture},
    snapshot,
};
use arc_core::meter::{MeterSnapshot, SamplingMode};
use std::{
    io::Write,
    os::unix::net::{UnixListener, UnixStream},
    process::Stdio,
    thread,
    time::{Duration, Instant},
};

const WATCHDOG: Duration = Duration::from_secs(60);
const POLL: Duration = Duration::from_millis(100);

/// One `arcctl exec -- cargo build` whose offline cargo stub stays alive at a
/// barrier, so the daemon has a RUNNING job and samples at its fast cadence.
pub struct Held {
    wrapper: Owned,
    release: UnixStream,
}

impl Held {
    pub fn start(f: &Fixture, s: &Sandbox) -> Self {
        s.stub("cargo");
        let barrier = f.root.join("held-build");
        let listener = UnixListener::bind(&barrier).unwrap();
        let mut command = s.arcctl(f, &s.project("held"));
        command
            .args(["exec", "--", "cargo", "build"])
            .env("ARC_FIXTURE_MODE", "success")
            .env("ARC_FIXTURE_MARKER", f.root.join("held-starts"))
            .env("ARC_FIXTURE_CONTROL", &barrier)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(std::fs::File::create(f.root.join("held-stderr")).unwrap());
        let mut wrapper = Owned::spawn(&mut command).unwrap();
        // Spawn precedes the wrapper's attachment request, so the stub can
        // reach the barrier while the daemon still records RESERVED.
        let (release, _) = native_exec::accept(&listener);
        let db = journey::database(f);
        journey::eventually("daemon attaches the held build", || {
            assert!(
                wrapper.child.try_wait().unwrap().is_none(),
                "held wrapper exited before attachment"
            );
            match journey::queue_state(&db, wrapper.child.id()).as_deref() {
                Some("RUNNING") => true,
                Some("RESERVED") | None => false,
                other => panic!("unexpected held-build state before attachment: {other:?}"),
            }
        });
        Self { wrapper, release }
    }

    /// Let the stub exit; the managed command succeeds and ran exactly once.
    pub fn finish(mut self, f: &Fixture) {
        self.release.write_all(b"x").unwrap();
        let status = self.wrapper.wait("held build").unwrap();
        assert!(
            status.success(),
            "{status:?}: {:?}",
            std::fs::read_to_string(f.root.join("held-stderr"))
        );
        assert_eq!(journey::starts(&f.root.join("held-starts")), 1);
    }
}

/// One pass of the test's own OS readings: the host counters and the CPU time
/// the kernel has charged to the daemon process.
#[derive(Debug, Clone, Copy)]
pub struct Reading {
    pub host: os::Host,
    pub daemon_cpu_ns: u64,
}

fn reading(daemon: u32) -> Reading {
    Reading {
        host: os::host(),
        daemon_cpu_ns: native::cpu_time_ns(daemon).unwrap(),
    }
}

/// OS readings and built-CLI snapshot reads, strictly alternating: reading 0,
/// read 0, reading 1, read 1, ..., and one closing reading after the last read.
pub struct Observation {
    readings: Vec<Reading>,
    /// `sequences[i]` is the sequence returned by the read after `readings[i]`.
    sequences: Vec<u64>,
    /// Each distinct snapshot the reads returned, in sequence order.
    pub snapshots: Vec<MeterSnapshot>,
}

impl Observation {
    /// The sequence of the first read. Samples from `base() + 2` on are bracketed.
    pub fn base(&self) -> u64 {
        self.sequences[0]
    }

    /// The test's readings from one taken before the daemon's sample `first`
    /// through one taken after its sample `last`.
    ///
    /// Before: a read that returned sequence `q` proves sample `q + 1` was not
    /// yet published, and the sampler takes sample `q + 2` only after it has
    /// published `q + 1`. So every sample from `q + 2` on is taken after the
    /// reading that preceded that read. The latest such reading is used.
    /// After: a read that returned at least `last` proves that sample was
    /// taken before it, hence before the reading that follows the read.
    pub fn around(&self, first: u64, last: u64) -> &[Reading] {
        let start = self
            .sequences
            .iter()
            .rposition(|sequence| sequence + 2 <= first)
            .unwrap_or_else(|| panic!("no reading before sample {first}: {:?}", self.sequences));
        let end = self
            .sequences
            .iter()
            .position(|sequence| *sequence >= last)
            .unwrap_or_else(|| panic!("no reading after sample {last}: {:?}", self.sequences));
        &self.readings[start..=end + 1]
    }
}

/// Wait until the running job has moved the daemon off its idle cadence, then
/// alternate OS readings with snapshot reads until the daemon has published
/// `samples` further samples. The daemon is never asked to sample.
pub fn observe(f: &Fixture, s: &Sandbox, daemon: u32, samples: u64) -> Observation {
    // Restarted whenever the daemon publishes a sample: it bounds one wait.
    let mut deadline = Instant::now() + WATCHDOG;
    while snapshot::read(f, s).cadence.mode == SamplingMode::Idle {
        assert!(
            Instant::now() < deadline,
            "daemon cadence stayed idle with a RUNNING job: 60 s watchdog"
        );
        thread::sleep(POLL);
    }
    let mut observation = Observation {
        readings: Vec::new(),
        sequences: Vec::new(),
        snapshots: Vec::new(),
    };
    loop {
        observation.readings.push(reading(daemon));
        let snapshot = snapshot::read(f, s);
        assert_eq!(snapshot.daemon_instance.pid, daemon, "{snapshot:?}");
        assert_ne!(snapshot.cadence.mode, SamplingMode::Idle, "{snapshot:?}");
        let sequence = snapshot.sequence;
        let previous = observation.sequences.last().copied();
        assert!(
            previous.is_none_or(|previous| previous <= sequence),
            "sequence went back: {previous:?} then {sequence}"
        );
        observation.sequences.push(sequence);
        if previous != Some(sequence) {
            observation.snapshots.push(snapshot);
            deadline = Instant::now() + WATCHDOG;
        }
        if sequence >= observation.base() + samples {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "daemon published no sample after {sequence}: 60 s watchdog"
        );
        thread::sleep(POLL);
    }
    observation.readings.push(reading(daemon));
    observation
}

/// One measured figure per line on the test binary's own standard error, so
/// it reaches the runner log on every run. `eprintln!` would not: the test
/// harness captures it and drops the output of a passing test.
#[allow(clippy::explicit_write)]
pub fn report(name: &str, value: &serde_json::Value) {
    writeln!(std::io::stderr(), "meter-readings-{name}: {value}").unwrap();
}

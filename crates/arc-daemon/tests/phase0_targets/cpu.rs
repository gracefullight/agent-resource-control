//! CPU time the OS charged to the idle daemon over a wall window (NFR-001).
//!
//! Consumed CPU time is a property of the daemon alone: a loaded host can
//! delay the daemon but cannot add to what it consumed.
use crate::{native, runtime::Fixture, watchdog};
use arc_core::meter::{MeterSnapshot, SamplingMode};
use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    thread,
    time::{Duration, Instant},
};

/// One child's CPU time read two independent ways.
pub struct Calibration {
    /// `cpu_time_ns` while the child was alive and had finished its work.
    pub counters_ns: u64,
    /// `wait4` resource usage (user plus system) after it exited.
    pub rusage_ns: u64,
}

/// Burn CPU in a shell built-in loop, read the live counters once the loop is
/// done, then let the child exit and read what `wait4` charged to it.
pub fn calibrate() -> Calibration {
    let mut command = Command::new("/bin/sh");
    command
        .args([
            "-c",
            "i=0; while [ $i -lt 80000 ]; do i=$((i+1)); done; echo burned; read line",
        ])
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    let mut child = command.spawn().unwrap();
    let pid = child.id();
    let guard = watchdog::Watchdog::arm(pid, "CPU calibration child");
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    assert_eq!(line, "burned\n");
    let counters_ns = native::cpu_time_ns(pid).unwrap();
    drop(child.stdin.take()); // End of input: the blocked `read` returns and the shell exits.
    let (status, usage) = reap(child);
    guard.disarm();
    assert!(libc::WIFEXITED(status), "calibration child status {status}");
    let nanos = |time: libc::timeval| {
        u64::try_from(time.tv_sec).unwrap() * 1_000_000_000
            + u64::try_from(time.tv_usec).unwrap() * 1_000
    };
    Calibration {
        counters_ns,
        rusage_ns: nanos(usage.ru_utime) + nanos(usage.ru_stime),
    }
}

/// Reap `child` with `wait4` and return its wait status and the resource
/// usage the kernel charged to it. The handle is consumed: the child is gone
/// afterwards and the standard library must not wait for it again.
fn reap(child: Child) -> (libc::c_int, libc::rusage) {
    let pid = child.id() as libc::pid_t;
    let mut status = 0;
    // SAFETY: an all-zero rusage is a valid value for wait4 to overwrite.
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    // SAFETY: both out-pointers are valid and this process owns the unreaped child.
    let reaped = unsafe { libc::wait4(pid, &mut status, 0, &mut usage) };
    assert_eq!(reaped, pid, "wait4: {}", std::io::Error::last_os_error());
    (status, usage)
}

/// What the daemon consumed while nothing was asked of it.
pub struct Idle {
    pub cpu_ns: u64,
    /// Wall time between the two snapshot reads; the CPU readings bracket it.
    pub window: Duration,
    /// Meter samples the daemon published between the two snapshot reads.
    pub ticks: u64,
    pub mode: SamplingMode,
    pub interval_ms: u64,
}

fn snapshot(fixture: &Fixture) -> Option<MeterSnapshot> {
    let request = arc_ipc::meter::snapshot_request("idle-window");
    // A fresh connection per read: none is left open across the idle window.
    let response = arc_ipc::exchange(&mut fixture.connect(), &request).unwrap();
    arc_ipc::meter::interpret_snapshot(&request, &response).ok()
}

/// Wait for the first published Meter sample, let startup work finish for
/// `settle`, then measure across `window`. The two snapshot reads are the
/// only requests and lie inside the bracket, so their cost is charged to the
/// daemon and a sequence advance between them is a tick inside the window.
pub fn idle(fixture: &Fixture, pid: u32, settle: Duration, window: Duration) -> Idle {
    let deadline = Instant::now() + watchdog::LIMIT;
    while snapshot(fixture).is_none() {
        assert!(Instant::now() < deadline, "no Meter sample: 60 s watchdog");
        thread::sleep(Duration::from_millis(20));
    }
    thread::sleep(settle);
    let cpu_before = native::cpu_time_ns(pid).unwrap();
    let start = Instant::now();
    let first = snapshot(fixture).expect("Meter snapshot at window start");
    thread::sleep(window.saturating_sub(start.elapsed()));
    let last = snapshot(fixture).expect("Meter snapshot at window end");
    let window = start.elapsed();
    let cpu_after = native::cpu_time_ns(pid).unwrap();
    for snapshot in [&first, &last] {
        assert_eq!(snapshot.daemon_instance.pid, pid, "another daemon answered");
    }
    assert_eq!(first.daemon_instance, last.daemon_instance);
    Idle {
        cpu_ns: cpu_after - cpu_before,
        window,
        ticks: last.sequence - first.sequence,
        mode: last.cadence.mode,
        interval_ms: last.cadence.interval_ms,
    }
}

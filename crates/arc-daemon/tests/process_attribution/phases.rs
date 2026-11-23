//! The envelope, identity-change, failure and clock-reversal phases of the
//! named attribution test, each over observed daemon state: the
//! self-observer's lines about a fixture's own reads, the daemon's interval
//! store and summaries read through built `arcctl`.
use crate::{
    fixture::{self, Line, Probe, Scan},
    journey::{self, Sandbox},
    runtime::Fixture,
    summary::*,
    tree,
};
use arc_core::process_identity::ProcessIdentity;
use arc_ipc::meter::quality::Availability;
use arc_storage::attribution::IntervalStore;
use std::{
    fs,
    io::{Read, Write},
    os::unix::{fs::PermissionsExt, net::UnixStream},
    path::Path,
    time::{Duration, Instant},
};

pub fn minute(ms: u64) -> u64 {
    ms / 60_000
}

/// One counted read of a fixture identity and the scan that read it before.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reading {
    pub previous: Scan,
    pub scan: Scan,
    pub delta_ns: u64,
    pub interval: (u64, u64),
}

/// Counted reads whose previous read was the immediately preceding pass.
pub fn readings(probe: &Probe) -> Vec<Reading> {
    probe
        .lines
        .windows(2)
        .filter_map(|pair| match (pair[0].scan(), &pair[1]) {
            (
                Some(previous),
                Line::Counted {
                    delta_ns,
                    scan,
                    interval,
                    ..
                },
            ) if scan.seq == previous.seq + 1 => Some(Reading {
                previous,
                scan: *scan,
                delta_ns: *delta_ns,
                interval: *interval,
            }),
            _ => None,
        })
        .collect()
}

pub fn protect(store: &IntervalStore, read_only: bool) {
    let mode = if read_only { 0o400 } else { 0o600 };
    fs::set_permissions(store.path(), fs::Permissions::from_mode(mode)).unwrap();
}

/// A scan whose recorded whole-ms bounds hold a boundary strictly inside.
fn interior(scan: &Scan) -> bool {
    scan.end_ms >= scan.start_ms + 2
}

/// Envelopes of the outside burner's own reads: an interval spans the
/// earlier scan's start to the later scan's end, a window whose boundary
/// lies strictly inside either scan's recorded bounds leaves it out, and two
/// overlapping envelopes count each delta once.
pub fn assert_envelopes(
    f: &Fixture,
    s: &Sandbox,
    cwd: &Path,
    store: &IntervalStore,
    request: &Path,
) {
    let mut pair = None;
    journey::eventually("two consecutive intervals of the outside burner", || {
        let read = readings(&fixture::probe(request).unwrap());
        // Both reads of both deltas have an interior boundary to cut at.
        pair = read
            .windows(2)
            .rev()
            .find(|w| {
                w[1].previous == w[0].scan
                    && [w[0].previous, w[0].scan, w[1].scan].iter().all(interior)
            })
            .map(|w| (w[0], w[1]));
        pair.is_some()
    });
    let (first, second) = pair.unwrap();
    let stored = intervals(store);
    for read in [&first, &second] {
        let (from, to) = read.interval;
        assert!(read.previous.start_ms < read.previous.end_ms, "{read:?}");
        assert!(read.previous.end_ms <= read.scan.start_ms, "{read:?}");
        assert!(
            from <= read.previous.start_ms && read.scan.end_ms <= to,
            "the envelope contains both scans that read this delta: {read:?}"
        );
        assert!(
            stored
                .iter()
                .any(|i| i.measured && (i.start, i.end) == (from, to)),
            "{read:?} {stored:?}"
        );
    }
    assert!(
        second.interval.0 < first.interval.1,
        "consecutive envelopes overlap"
    );
    let mut exact = Vec::new();
    for read in [&first, &second] {
        let (from, to) = read.interval;
        let section = attribution(f, s, cwd, from, to);
        assert_eq!(section.window.scans, 1, "{section:?}");
        assert_eq!(
            (section.window.first_scan_ms, section.window.last_scan_ms),
            (Some(from), Some(to))
        );
        assert!(section.unattributed.value.unwrap() + ROUNDING >= read.delta_ns as f64 / 1e9);
        assert_own_ratio(&section);
        assert_matches_store(&section, store);
        // A boundary strictly inside the earlier, then the later scan's
        // recorded bounds, and so strictly inside the envelope.
        let inside_earlier = read.previous.start_ms + 1;
        let inside_later = read.scan.end_ms - 1;
        assert!(
            read.previous.start_ms < inside_earlier && inside_earlier < read.previous.end_ms,
            "{read:?}"
        );
        assert!(
            read.scan.start_ms < inside_later && inside_later < read.scan.end_ms,
            "{read:?}"
        );
        assert!(from < inside_earlier && inside_later < to, "{read:?}");
        for (cut_from, cut_to) in [(inside_earlier, to), (from, inside_later)] {
            let cut = attribution(f, s, cwd, cut_from, cut_to);
            assert_eq!(cut.window.scans, 0, "[{cut_from}, {cut_to}) {cut:?}");
            assert_eq!(cut.unattributed.reason.as_deref(), Some("no_scan"));
        }
        exact.push(section.unattributed.value.unwrap());
    }
    let both = attribution(f, s, cwd, first.interval.0, second.interval.1);
    assert_eq!(both.window.scans, 2);
    assert_eq!(
        both.unattributed_ratio.reason, None,
        "an overlap is no hole"
    );
    assert!(
        (both.unattributed.value.unwrap() - exact[0] - exact[1]).abs() < 2.0 * ROUNDING,
        "each delta is counted once: {both:?} {exact:?}"
    );
    assert!(
        both.unattributed.value.unwrap() + ROUNDING
            >= (first.delta_ns + second.delta_ns) as f64 / 1e9
    );
    assert_own_ratio(&both);
    assert_matches_store(&both, store);
}

/// The role's two lifetimes received one PID and were read by consecutive
/// scans; the replacement's first reading counted nothing although its CPU
/// exceeded the old lifetime's last reading, which a PID-only key would
/// have counted as a delta.
pub fn assert_same_pid_replacement(
    dir: &Path,
    first_life: &ProcessIdentity,
    deadline: Instant,
) -> ProcessIdentity {
    let reused = loop {
        if let Some(value) = fs::read_to_string(dir.join("reuse"))
            .ok()
            .and_then(|value| value.trim().parse::<u64>().ok())
        {
            break match value {
                0 => false,
                1 => true,
                _ => panic!("invalid fixture PID-reuse result: {value}"),
            };
        }
        if Instant::now() >= deadline {
            // This is an observation deadline, not a child-wait watchdog.
            // Cancellation remains owned and reaped on the failure path.
            tree::release(dir);
            panic!("process_attribution: 16 workers attempted to cycle the native PID allocator; the same-PID replacement was not observed within this target's time budget");
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    if !reused {
        panic!("process_attribution: 16 workers cycled the native PID allocator, but the target PID was allocated elsewhere; the same-PID observation is missing");
    }
    let second = identity(tree::published(dir, "respawn-2.pid"));
    assert_eq!(second.pid, first_life.pid, "one PID");
    assert_ne!(second.start_token, first_life.start_token, "two lifetimes");
    let first = fixture::probe(&fixture::observation(dir, "respawn-1")).unwrap();
    assert_counted_only_after_first_sighting(&first, "respawn-1");
    let (last_scan, last_cpu) = first
        .lines
        .iter()
        .rev()
        .find_map(|line| Some((line.scan()?, Probe::cpu_ns(line)?)))
        .unwrap();
    let replacement = fixture::scanned(&fixture::observation(dir, "respawn-2"), 1);
    let Line::First { cpu_ns, scan, .. } = &replacement.lines[0] else {
        panic!("the replacement's first reading was counted: {replacement:?}");
    };
    assert_eq!(
        scan.seq,
        last_scan.seq + 1,
        "consecutive scans read the two lifetimes: {first:?} {replacement:?}"
    );
    assert!(
        *cpu_ns > last_cpu,
        "the replacement's first reading ({cpu_ns} ns) exceeds the old baseline ({last_cpu} ns)"
    );
    assert_counted_only_after_first_sighting(&replacement, "respawn-2");
    assert_eq!(replacement.only(), ("unattributed", ""));
    eprintln!("process_attribution: same-PID replacement observed across consecutive native scans");
    second
}

/// A failed pass right after two committed intervals of the outside burner.
#[derive(Debug, Clone, Copy)]
pub struct Failure {
    pub earlier: Reading,
    pub last: Reading,
    pub seq: u64,
    pub at_ms: u64,
}
impl Failure {
    /// The last committed scan: the scanner's durable frontier.
    pub fn frontier_ms(&self) -> u64 {
        self.last.scan.start_ms
    }
}

/// Write-protect the store until a pass fails right after two consecutive
/// committed intervals in the failure's calendar minute; otherwise recover
/// and try again. The store stays write-protected on return.
pub fn unresolved_failure(store: &IntervalStore, request: &Path) -> Failure {
    for _ in 0..3 {
        let before = fixture::probe(request).unwrap().lines.len();
        protect(store, true);
        let mut failed = None;
        journey::eventually("a failed attribution pass", || {
            failed = fixture::probe(request).unwrap().lines[before..]
                .iter()
                .find_map(|line| match line {
                    Line::Failed { seq, at_ms } => Some((*seq, *at_ms)),
                    _ => None,
                });
            failed.is_some()
        });
        let (seq, at_ms) = failed.unwrap();
        let read = readings(&fixture::probe(request).unwrap());
        let committed: Vec<_> = read.iter().filter(|r| r.scan.seq < seq).collect();
        if let [.., earlier, last] = committed.as_slice() {
            // An earlier attempt's failed interval must not reach the pair.
            let clear = intervals(store)
                .iter()
                .all(|i| i.measured || i.end <= earlier.interval.0);
            if last.scan.seq + 1 == seq
                && last.previous == earlier.scan
                && minute(earlier.interval.0) == minute(at_ms)
                && clear
            {
                return Failure {
                    earlier: **earlier,
                    last: **last,
                    seq,
                    at_ms,
                };
            }
        }
        protect(store, false);
        journey::eventually(
            "a new baseline and three counted scans after the failure",
            || {
                let probe = fixture::probe(request).unwrap();
                let after = probe
                    .lines
                    .iter()
                    .skip_while(|l| l.scan().is_none_or(|s| s.seq <= seq));
                after.filter(|l| matches!(l, Line::Counted { .. })).count() >= 3
            },
        );
    }
    panic!("no failed pass shared a calendar minute with two committed intervals in 3 attempts");
}

/// While the failure is unresolved: a same-minute window holding both
/// committed intervals and the failure is pending, never complete; with
/// `clear_before`, the window ending at the frontier still holds a complete
/// interval (before the wall clock stepped back over it).
pub fn assert_unresolved(
    f: &Fixture,
    s: &Sandbox,
    cwd: &Path,
    store: &IntervalStore,
    failure: &Failure,
    clear_before: bool,
    when: &str,
) {
    let (from, to) = (failure.earlier.interval.0, failure.at_ms + 1);
    let pending = attribution(f, s, cwd, from, to);
    assert_eq!(pending.window.scans, 2, "{when}: {pending:?}");
    assert_eq!(pending.unattributed_ratio.value, None, "{when}");
    assert_eq!(
        pending.unattributed_ratio.reason.as_deref(),
        Some("pending_scan"),
        "{when}: {pending:?}"
    );
    assert_eq!(
        pending.unattributed.availability,
        Availability::Partial,
        "{when}"
    );
    assert_matches_store(&pending, store);
    if clear_before {
        let clear = attribution(f, s, cwd, from, failure.frontier_ms());
        assert_eq!(clear.window.scans, 1, "{when}: {clear:?}");
        assert_eq!(clear.window.last_scan_ms, Some(failure.earlier.interval.1));
        assert_own_ratio(&clear);
    }
}

/// The scanner's wall clock stepped back over an earlier interval.
#[derive(Debug, Clone, Copy)]
pub struct Reversal {
    /// The earlier measured interval's envelope, as a window.
    pub window: (u64, u64),
    /// The reversed wall time of the first pass that saw the step.
    pub at_ms: u64,
    /// The scanner's durable frontier when the clock stepped back.
    pub frontier_ms: u64,
}

/// Step the daemon's scanner wall clock (only its opt-in seam; no CPU
/// reading) back to 3 s before the failure's earlier interval, while the
/// store stays write-protected, and wait for the pass that reads it.
pub fn reverse_clock(f: &Fixture, request: &Path, failure: &Failure) -> Reversal {
    let window = failure.earlier.interval;
    let shift = now_ms() - window.0 + 3_000;
    let before = fixture::probe(request).unwrap().lines.len();
    fs::write(fixture::wall_shift(f), format!("-{shift}\n")).unwrap();
    let mut at = None;
    journey::eventually("a failed pass on the reversed wall clock", || {
        at = fixture::probe(request).unwrap().lines[before..]
            .iter()
            .find_map(|line| match line {
                Line::Failed { at_ms, .. } if *at_ms < window.0 => Some(*at_ms),
                _ => None,
            });
        at.is_some()
    });
    Reversal {
        window,
        at_ms: at.unwrap(),
        frontier_ms: failure.frontier_ms(),
    }
}

/// Correct the scanner's wall clock (remove its shift) while the store
/// stays write-protected, and wait for a pass on the corrected clock, past
/// the old frontier, that fails as well.
pub fn correct_clock(f: &Fixture, request: &Path, reversal: &Reversal) {
    let before = fixture::probe(request).unwrap().lines.len();
    fs::remove_file(fixture::wall_shift(f)).unwrap();
    journey::eventually("a failed pass on the corrected wall clock", || {
        fixture::probe(request).unwrap().lines[before..]
            .iter()
            .any(|line| matches!(line, Line::Failed { at_ms, .. } if *at_ms > reversal.frontier_ms))
    });
}

/// The earlier interval's own window ends between the reversed time and the
/// old frontier; once the clock stepped back over it, it is never complete.
pub fn assert_reversed(
    f: &Fixture,
    s: &Sandbox,
    cwd: &Path,
    store: &IntervalStore,
    reversal: &Reversal,
    reason: &str,
    when: &str,
) {
    let (from, to) = reversal.window;
    assert!(
        reversal.at_ms < to && to < reversal.frontier_ms,
        "{when}: {reversal:?}"
    );
    let section = attribution(f, s, cwd, from, to);
    assert_eq!(section.window.scans, 1, "{when}: {section:?}");
    assert_eq!(section.unattributed_ratio.value, None, "{when}");
    assert_eq!(
        section.unattributed_ratio.reason.as_deref(),
        Some(reason),
        "{when}: {section:?}"
    );
    assert_eq!(section.unattributed.availability, Availability::Partial);
    assert_matches_store(&section, store);
}

/// A caller cannot choose a different target by sending its identity. The
/// observer binds to the connecting test process, which is outside the
/// managed root's tree, even when it sends that root's identity as data.
pub fn observer_with_sent_target(f: &Fixture, root: &ProcessIdentity) -> UnixStream {
    let mut caller = UnixStream::connect(f.root.join("self.sock")).unwrap();
    caller.set_nonblocking(true).unwrap();
    writeln!(caller, "{}.{}", root.pid, root.start_token).unwrap();
    caller
}

/// Finish observing the caller while the other fixtures use the same scans.
pub fn assert_observer_ignores_a_sent_target(mut caller: UnixStream) {
    let mut received = Vec::new();
    journey::eventually("self-observer ignores a foreign target", || {
        let mut bytes = [0u8; 1024];
        match caller.read(&mut bytes) {
            Ok(n) if n > 0 => received.extend_from_slice(&bytes[..n]),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => (),
            other => panic!("self-observer closed unexpectedly: {other:?}"),
        }
        received.iter().filter(|&&b| b == b'\n').count() >= 2
    });
    let lines = std::str::from_utf8(&received).unwrap();
    assert!(
        lines.lines().all(|line| !line.contains(" proven ")),
        "{lines}"
    );
}

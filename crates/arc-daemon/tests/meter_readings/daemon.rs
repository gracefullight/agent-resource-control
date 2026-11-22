//! MET-024 and MET-025: the daemon's own CPU time and peak memory as it
//! reports them, checked against the kernel's figures for the daemon process.
use crate::{
    journey::Sandbox,
    observation::{self, Held, Observation},
    runtime::Fixture,
    support,
};
use arc_core::meter::{DaemonCpuAccounting, MeterSnapshot, Provenance, Unit};
use serde_json::json;

/// Samples the daemon publishes after the first read.
const SAMPLES: u64 = 3;

/// Allowed distance of the reported core-seconds outside the kernel's CPU
/// time for the daemon process read before and after that sample. The daemon
/// reports `getrusage` user and system time, each truncated to a microsecond;
/// the test reads `proc_pidinfo` in nanoseconds. Worst measured distance: 0 ns
/// (calibration as described in `host.rs`); the value lay at least 933,041 ns
/// inside readings 10 to 12 ms apart.
const CPU_TOLERANCE_NS: u64 = 10_000;

fn value<T: Copy>(
    name: &str,
    snapshot: &MeterSnapshot,
    measurement: &arc_core::meter::Measurement<T>,
    unit: Unit,
    provenance: Provenance,
) -> T {
    assert_eq!(
        (measurement.unit, measurement.provenance),
        (unit, provenance),
        "{name}"
    );
    *measurement
        .reading
        .value()
        .unwrap_or_else(|| panic!("{name} has no value: {snapshot:?}"))
}

/// The reported core-seconds of each bracketed sample lies between the
/// kernel's CPU time for the daemon process before and after that sample,
/// and never goes back from one snapshot to the next.
fn cpu(observation: &Observation) {
    let mut previous = 0.0;
    for snapshot in &observation.snapshots {
        assert_eq!(
            snapshot.daemon.cpu_accounting,
            DaemonCpuAccounting::ProcessLifetime
        );
        let seconds = value(
            "arc.daemon.cpu.core_seconds",
            snapshot,
            &snapshot.daemon.cpu_core_seconds,
            Unit::CoreSeconds,
            Provenance::Native,
        );
        assert!(seconds >= previous, "{seconds} after {previous}");
        previous = seconds;
        if snapshot.sequence < observation.base() + 2 {
            continue;
        }
        let window = observation.around(snapshot.sequence, snapshot.sequence);
        let before = window.first().unwrap().daemon_cpu_ns;
        let after = window.last().unwrap().daemon_cpu_ns;
        let reported = (seconds * 1e9).round() as u64;
        let deviation = before
            .saturating_sub(reported)
            .max(reported.saturating_sub(after));
        let figures = json!({
            "sequence": snapshot.sequence, "unit": "core_seconds", "reported": seconds,
            "reported_ns": reported, "os_before_ns": before, "os_after_ns": after,
            "deviation_ns": deviation, "tolerance_ns": CPU_TOLERANCE_NS,
        });
        observation::report("arc.daemon.cpu.core_seconds", &figures);
        assert!(
            deviation <= CPU_TOLERANCE_NS,
            "arc.daemon.cpu.core_seconds is outside the OS readings: {figures}"
        );
    }
}

/// The reported peak is never below the reported RSS, nor below any RSS an
/// earlier snapshot reported, and never decreases. Returns the last peak.
fn peak(observation: &Observation) -> u64 {
    let (mut highest_rss, mut previous_peak) = (0, 0);
    for snapshot in &observation.snapshots {
        let rss = value(
            "arc.daemon.memory.rss_bytes",
            snapshot,
            &snapshot.daemon.rss_bytes,
            Unit::Bytes,
            Provenance::Native,
        );
        let peak = value(
            "arc.daemon.memory.peak_bytes",
            snapshot,
            &snapshot.daemon.observed_peak_rss_bytes,
            Unit::Bytes,
            Provenance::Derived,
        );
        highest_rss = highest_rss.max(rss);
        assert!(rss > 0 && rss <= peak, "RSS {rss} above peak {peak}");
        assert!(
            peak >= highest_rss,
            "peak {peak} below an earlier RSS {highest_rss}"
        );
        assert!(
            peak >= previous_peak,
            "peak went back: {previous_peak} then {peak}"
        );
        previous_peak = peak;
        observation::report(
            "arc.daemon.memory.peak_bytes",
            &json!({
                "sequence": snapshot.sequence, "unit": "bytes", "reported": peak,
                "reported_rss": rss, "peak_above_rss_bytes": peak - rss,
            }),
        );
    }
    previous_peak
}

pub fn run() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    let mut daemon = f.spawn_isolated("");
    let held = Held::start(&f, &s);

    let observation = observation::observe(&f, &s, daemon.child.id(), SAMPLES);
    assert!(observation.snapshots.len() >= 2, "one snapshot only");
    cpu(&observation);
    let reported_peak = peak(&observation);

    held.finish(&f);
    // The kernel's lifetime high-water mark of the reaped daemon process. The
    // reported peak is the maximum of the daemon's own samples, so it can be
    // lower but never higher.
    let (_, lifetime_peak) = daemon.stop(libc::SIGTERM);
    observation::report(
        "arc.daemon.memory.peak_bytes",
        &json!({
            "unit": "bytes", "reported": reported_peak, "os_lifetime_peak": lifetime_peak,
            "deviation_bytes": reported_peak.saturating_sub(lifetime_peak),
            "tolerance_bytes": 0,
            "os_peak_above_reported_bytes": lifetime_peak.saturating_sub(reported_peak),
        }),
    );
    assert!(
        reported_peak <= lifetime_peak,
        "reported peak {reported_peak} above the OS lifetime peak {lifetime_peak}"
    );
    f.report();
}

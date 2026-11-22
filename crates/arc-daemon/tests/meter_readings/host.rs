//! MET-001, MET-006 and MET-008: the host values a real daemon reports,
//! checked against the test's own OS readings around the daemon's samples.
use crate::{
    journey::Sandbox,
    observation::{self, Held, Observation, Reading as OsReading},
    os,
    runtime::Fixture,
    support,
};
use arc_core::meter::{CpuNormalization, MeterSnapshot, Provenance, Unit};
use serde_json::json;

/// Samples the daemon publishes after the first read. The CPU interval of the
/// third lies wholly after the test's first reading.
const SAMPLES: u64 = 3;

// Calibration behind every tolerance in this target: Actor-measured, 5
// consecutive runs, one macOS arm64 host (8 logical CPUs, 16 GiB), debug
// build, while the arc-storage test binaries ran in a loop. The figures are
// the worst of two such series. A tolerance is kept only at five times the
// worst deviation or more.

/// Allowed excess of the reported busy or idle share over the test's own tick
/// count, per logical CPU. The share is rebuilt from the reported percentage
/// and `elapsed_ms`, which the daemon reads beside (not inside) the kernel's
/// integral 10 ms tick counters: up to three counters per CPU truncate at each
/// end of the interval. Worst measured excess: 0 ticks (host 59% to 99% busy).
const CPU_TOLERANCE_TICKS_PER_CPU: f64 = 8.0;

/// Allowed distance of the reported available memory outside the envelope of
/// the test's readings around that sample. Worst measured distance: 1,982,464
/// bytes, while adjacent readings of the test differed by up to 112,017,408.
const MEMORY_TOLERANCE_BYTES: u64 = 64 * 1024 * 1024;

/// Allowed distance of the reported used swap outside the envelope of the
/// test's readings around that sample. Worst measured distance: 0 bytes, with
/// about 11.2 GB of swap in use.
const SWAP_TOLERANCE_BYTES: u64 = 16 * 1024 * 1024;

/// The closed range of the test's readings, how far `value` lies outside it,
/// and the largest change between two adjacent readings.
fn outside(value: u64, readings: &[u64]) -> (u64, u64, u64, u64) {
    let low = *readings.iter().min().unwrap();
    let high = *readings.iter().max().unwrap();
    let distance = low.saturating_sub(value).max(value.saturating_sub(high));
    let step = readings
        .windows(2)
        .map(|pair| pair[0].abs_diff(pair[1]))
        .max()
        .unwrap();
    (low, high, distance, step)
}

/// The reported utilization is a value in 0 to 100 over a host-capacity
/// interval, and its busy and idle shares of that interval fit inside the
/// busy and idle ticks the test counted over a window containing it.
fn cpu(snapshot: &MeterSnapshot, window: &[OsReading]) -> serde_json::Value {
    let measurement = &snapshot.host.cpu_utilization;
    assert_eq!(
        (measurement.unit, measurement.provenance),
        (Unit::Percent, Provenance::Derived)
    );
    let percent = *measurement
        .reading
        .value()
        .unwrap_or_else(|| panic!("host.cpu.utilization has no value: {snapshot:?}"));
    assert!((0.0..=100.0).contains(&percent), "{percent}");
    let interval = snapshot
        .host
        .cpu_interval
        .as_ref()
        .unwrap_or_else(|| panic!("no CPU interval beside a value: {snapshot:?}"));
    let cpus = os::logical_cpu_count();
    assert_eq!(interval.logical_cpu_count, cpus);
    assert_eq!(interval.normalization, CpuNormalization::HostCapacity);
    assert!(interval.elapsed_ms > 0, "{interval:?}");
    assert_eq!(
        interval.ended_monotonic_ms - interval.started_monotonic_ms,
        interval.elapsed_ms
    );
    assert_eq!(
        interval.ended_monotonic_ms, snapshot.sampled_monotonic_ms,
        "the interval ends at this sample"
    );

    let (first, last) = (window.first().unwrap(), window.last().unwrap());
    let (busy, idle) = last.host.cpu.since(&first.host.cpu);
    let capacity =
        interval.elapsed_ms as f64 * f64::from(cpus) * os::ticks_per_second() as f64 / 1000.0;
    let reported_busy = capacity * percent / 100.0;
    let reported_idle = capacity - reported_busy;
    let busy_excess = (reported_busy - busy as f64).max(0.0);
    let idle_excess = (reported_idle - idle as f64).max(0.0);
    let tolerance = CPU_TOLERANCE_TICKS_PER_CPU * f64::from(cpus);
    let figures = json!({
        "sequence": snapshot.sequence, "unit": "percent", "reported_percent": percent,
        "interval_ms": interval.elapsed_ms, "logical_cpus": cpus,
        "reported_busy_ticks": reported_busy, "reported_idle_ticks": reported_idle,
        "window_busy_ticks": busy, "window_idle_ticks": idle,
        "window_percent": busy as f64 / (busy + idle).max(1) as f64 * 100.0,
        "deviation_busy_ticks": busy_excess, "deviation_idle_ticks": idle_excess,
        "tolerance_ticks": tolerance,
    });
    assert!(
        busy_excess <= tolerance && idle_excess <= tolerance,
        "host.cpu.utilization does not fit the OS counters: {figures}"
    );
    figures
}

/// A reported byte value lies inside the envelope of the test's readings
/// around the sample that produced it, and never above `ceiling`.
fn bytes(
    name: &str,
    snapshot: &MeterSnapshot,
    select: fn(&MeterSnapshot) -> &arc_core::meter::Measurement<u64>,
    readings: &[u64],
    tolerance: u64,
    ceiling: u64,
) -> serde_json::Value {
    let measurement = select(snapshot);
    assert_eq!(
        (measurement.unit, measurement.provenance),
        (Unit::Bytes, Provenance::Native),
        "{name}"
    );
    let value = *measurement
        .reading
        .value()
        .unwrap_or_else(|| panic!("{name} has no value: {snapshot:?}"));
    let (low, high, deviation, step) = outside(value, readings);
    let figures = json!({
        "sequence": snapshot.sequence, "unit": "bytes", "reported": value,
        "os_low": low, "os_high": high, "os_largest_step_bytes": step,
        "deviation_bytes": deviation, "tolerance_bytes": tolerance,
        "ceiling_bytes": ceiling,
    });
    assert!(
        deviation <= tolerance,
        "{name} is outside the OS readings: {figures}"
    );
    assert!(value <= ceiling, "{name} exceeds its ceiling: {figures}");
    figures
}

fn check(observation: &Observation) {
    let physical = os::physical_memory_bytes();
    let last = observation.snapshots.last().unwrap();
    assert!(
        last.sequence >= 2,
        "read before the second sample: {last:?}"
    );
    for snapshot in &observation.snapshots {
        let sequence = snapshot.sequence;
        // A value sampled at `sequence` is bracketed from `base + 2` on; the
        // CPU interval began one sample earlier.
        if sequence >= observation.base() + 3 {
            let window = observation.around(sequence - 1, sequence);
            observation::report("host.cpu.utilization", &cpu(snapshot, window));
        }
        if sequence >= observation.base() + 2 {
            let window = observation.around(sequence, sequence);
            let readings = |select: fn(&os::Host) -> u64| -> Vec<u64> {
                window.iter().map(|reading| select(&reading.host)).collect()
            };
            observation::report(
                "host.memory.available_bytes",
                &bytes(
                    "host.memory.available_bytes",
                    snapshot,
                    |snapshot| &snapshot.host.memory_available_bytes,
                    &readings(|host| host.memory_available_bytes),
                    MEMORY_TOLERANCE_BYTES,
                    physical,
                ),
            );
            // Total swap is resized by the kernel while it is in use.
            let total = readings(|host| host.swap_total_bytes);
            observation::report(
                "host.swap.used_bytes",
                &bytes(
                    "host.swap.used_bytes",
                    snapshot,
                    |snapshot| &snapshot.host.swap_used_bytes,
                    &readings(|host| host.swap_used_bytes),
                    SWAP_TOLERANCE_BYTES,
                    *total.iter().max().unwrap(),
                ),
            );
        }
    }
}

pub fn run() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    let mut daemon = f.spawn_isolated("");
    let held = Held::start(&f, &s);

    let observation = observation::observe(&f, &s, daemon.child.id(), SAMPLES);
    check(&observation);

    held.finish(&f);
    daemon.stop(libc::SIGTERM);
    f.report();
}

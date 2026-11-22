//! Meter readings of a real daemon read through built `arcctl meter snapshot
//! --json` (MET-001, MET-006, MET-008, MET-024, MET-025).
#[path = "meter_readings/daemon.rs"]
mod daemon;
#[path = "meter_readings/host.rs"]
mod host;
#[path = "support/journey.rs"]
mod journey;
#[path = "support/native.rs"]
mod native;
#[allow(dead_code)] // Only the admission barrier is used here.
#[path = "support/native_exec.rs"]
mod native_exec;
#[path = "meter_readings/observation.rs"]
mod observation;
#[path = "meter_readings/os.rs"]
mod os;
#[allow(dead_code)] // The shared fixture also serves unrelated workloads.
#[path = "support/runtime.rs"]
mod runtime;
#[path = "meter_readings/snapshot.rs"]
mod snapshot;
mod support;

use arc_core::meter::{Provenance, Unit, METER_SCHEMA_VERSION};
use journey::Sandbox;
use runtime::Fixture;

/// The snapshot of a real daemon reaches the built CLI as one typed document
/// that names this daemon process and carries each selected reading with its
/// SRS unit. Whether a reading has a value yet is not asserted here.
#[test]
fn seam_real_daemon_meter_snapshot_json_parses_through_built_arcctl() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    let mut daemon = f.spawn_isolated("");

    let first = snapshot::read(&f, &s);
    assert_eq!(first.schema_version, METER_SCHEMA_VERSION);
    assert_eq!(first.daemon_instance.pid, daemon.child.id());
    assert!(first.sequence >= 1, "{first:?}");
    assert_eq!(
        (
            first.host.cpu_utilization.unit,
            first.host.cpu_utilization.provenance
        ),
        (Unit::Percent, Provenance::Derived)
    );
    for (name, unit) in [
        (
            "memory_available_bytes",
            first.host.memory_available_bytes.unit,
        ),
        ("swap_used_bytes", first.host.swap_used_bytes.unit),
        ("rss_bytes", first.daemon.rss_bytes.unit),
        (
            "observed_peak_rss_bytes",
            first.daemon.observed_peak_rss_bytes.unit,
        ),
    ] {
        assert_eq!(unit, Unit::Bytes, "{name}");
    }
    assert_eq!(first.daemon.cpu_core_seconds.unit, Unit::CoreSeconds);

    // A second read names the same daemon and never goes back in sequence.
    let second = snapshot::read(&f, &s);
    assert_eq!(second.daemon_instance, first.daemon_instance);
    assert!(second.sequence >= first.sequence, "{first:?} {second:?}");

    daemon.stop(libc::SIGTERM);
    f.report();
}

/// MET-001, MET-006, MET-008. With one managed job RUNNING, the real daemon's
/// `host.cpu.utilization`, `host.memory.available_bytes` and
/// `host.swap.used_bytes`, read through built `arcctl meter snapshot --json`,
/// each carry a value in their SRS unit that fits the test's own OS readings
/// taken around the daemon's samples.
#[test]
fn met_host_cpu_memory_and_swap_values_agree_with_os_readings() {
    host::run();
}

/// MET-024, MET-025. The real daemon's `arc.daemon.cpu.core_seconds` lies
/// between the kernel's CPU time for the daemon process before and after the
/// sample, and its `arc.daemon.memory.peak_bytes` is never below its RSS,
/// never decreases and never exceeds the kernel's lifetime peak of the process.
#[test]
fn met_daemon_cpu_seconds_and_peak_memory_agree_with_os_readings() {
    daemon::run();
}

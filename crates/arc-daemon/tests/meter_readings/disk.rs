//! MTR-001 item 3: a real disk workload, checked against fresh native
//! filesystem observations bracketing the daemon's published samples.
//!
//! The daemon samples `/`. On macOS the writable fixture can live on a
//! different APFS volume, so matching its container is part of the evidence.
use crate::{
    disk_held::{Held, Isolated},
    disk_native::{apfs_container, pressure, rank, statfs, NativeFs, WriteFixture},
    observation,
    runtime::Fixture,
    support,
};
use arc_core::meter::{FilesystemIdentity, MeterSnapshot, Provenance, SamplingMode, Unit};
use std::{
    path::Path,
    thread,
    time::{Duration, Instant},
};

const WATCHDOG: Duration = Duration::from_secs(60);
const POLL: Duration = Duration::from_millis(150);
// Other processes may allocate or release space between adjacent observations.
// This is less than 0.004% of a 500 GB volume; report the actual deviation.
const AVAILABLE_TOLERANCE: u64 = 16 * 1024 * 1024;

/// Bytes the workload had written and allocated around one checked sample.
struct Progress {
    before: u64,
    at_read: u64,
    after: u64,
    allocated_before: u64,
    allocated_after: u64,
}

fn check(snapshot: &MeterSnapshot, readings: &[NativeFs], progress: &Progress) {
    let first = readings.first().unwrap();
    let fs = &snapshot.host.filesystem;
    assert_eq!(
        fs.identity.as_ref(),
        Some(&FilesystemIdentity {
            path: "/".into(),
            mount_point: first.mount.clone(),
            filesystem_type: first.kind.clone(),
            volume_id: first.volume.clone(),
            is_local: first.local,
        })
    );
    assert!(first.local);
    assert!(readings.iter().all(|value| value.mount == first.mount
        && value.kind == first.kind
        && value.volume == first.volume
        && value.capacity == first.capacity));
    for (name, unit, provenance, expected) in [
        (
            "capacity",
            fs.capacity_bytes.unit,
            fs.capacity_bytes.provenance,
            (Unit::Bytes, Provenance::Native),
        ),
        (
            "available",
            fs.available_bytes.unit,
            fs.available_bytes.provenance,
            (Unit::Bytes, Provenance::Native),
        ),
        (
            "pressure",
            fs.pressure.unit,
            fs.pressure.provenance,
            (Unit::Level, Provenance::Derived),
        ),
    ] {
        assert_eq!((unit, provenance), expected, "{name}");
    }
    assert_eq!(fs.capacity_bytes.reading.value(), Some(&first.capacity));
    let available = *fs
        .available_bytes
        .reading
        .value()
        .expect("available disk bytes");
    let judgment = *fs.pressure.reading.value().expect("derived disk pressure");
    assert_eq!(judgment, pressure(available, first.capacity));
    let low = readings.iter().map(|value| value.available).min().unwrap();
    let high = readings.iter().map(|value| value.available).max().unwrap();
    let deviation = low
        .saturating_sub(available)
        .max(available.saturating_sub(high));
    assert!(
        deviation <= AVAILABLE_TOLERANCE,
        "available bytes {available} outside native [{low}, {high}] by {deviation}"
    );
    let lo = rank(pressure(
        low.saturating_sub(AVAILABLE_TOLERANCE),
        first.capacity,
    ));
    let hi = rank(pressure(
        high.saturating_add(AVAILABLE_TOLERANCE).min(first.capacity),
        first.capacity,
    ));
    assert!(
        (lo..=hi).contains(&rank(judgment)),
        "disk judgment does not fit independently observed ratios"
    );
    assert!(
        progress.before < progress.at_read
            && progress.at_read < progress.after
            && progress.allocated_after > progress.allocated_before,
        "disk bytes did not advance across this sample"
    );
    observation::report(
        "disk",
        &serde_json::json!({
            "sequence": snapshot.sequence, "capacity_bytes": first.capacity,
            "reported_available_bytes": available, "native_available_min_bytes": low,
            "native_available_max_bytes": high, "available_deviation_bytes": deviation,
            "tolerance_bytes": AVAILABLE_TOLERANCE, "judgment": judgment,
            "written_before_bytes": progress.before, "written_at_read_bytes": progress.at_read,
            "written_after_bytes": progress.after,
            "allocated_before_bytes": progress.allocated_before,
            "allocated_after_bytes": progress.allocated_after,
        }),
    );
}

/// A sample at least two sequences beyond a read began after the native
/// observation before that read, even if one sampling call was already in flight.
/// The first write precedes that read and the last follows the returned sample.
fn observe(s: &Isolated, writer: &mut WriteFixture, pid: u32) -> u64 {
    let (before, allocated_before) = (writer.written, writer.allocated());
    writer.write_chunk();
    let mut readings = vec![statfs(Path::new("/"))];
    let start = s.snapshot();
    assert_eq!(start.daemon_instance.pid, pid);
    let deadline = Instant::now() + WATCHDOG;
    let sample = loop {
        writer.write_chunk();
        readings.push(statfs(Path::new("/")));
        let current = s.snapshot();
        assert_eq!(current.daemon_instance.pid, pid);
        assert_ne!(current.cadence.mode, SamplingMode::Idle);
        if current.sequence >= start.sequence + 2 {
            break current;
        }
        assert!(
            Instant::now() < deadline,
            "daemon published no bracketed disk sample within 60 seconds"
        );
        thread::sleep(POLL);
    };
    let at_read = writer.written;
    writer.write_chunk();
    readings.push(statfs(Path::new("/")));
    let progress = Progress {
        before,
        at_read,
        after: writer.written,
        allocated_before,
        allocated_after: writer.allocated(),
    };
    check(&sample, &readings, &progress);
    sample.sequence
}

/// The fixture's filesystem is the sampled one, or a distinct APFS volume in
/// the same container whose capacity the daemon reports.
fn relate(s: &Isolated, path: &Path) {
    let root = statfs(Path::new("/"));
    let fixture = statfs(path);
    assert!(root.local && fixture.local);
    let same_volume = root.volume == fixture.volume;
    if !same_volume {
        assert_eq!(
            (root.kind.as_str(), fixture.kind.as_str()),
            ("apfs", "apfs"),
            "fixture does not share the sampled filesystem"
        );
        assert_eq!(
            apfs_container(&s.home, &root.mount),
            apfs_container(&s.home, &fixture.mount),
            "fixture does not share the sampled APFS container"
        );
    }
    observation::report(
        "disk-filesystem",
        &serde_json::json!({
            "sampled_mount": root.mount, "sampled_type": root.kind, "fixture_type": fixture.kind,
            "same_volume": same_volume, "separate_volumes_same_apfs_container": !same_volume,
            "sampled_capacity_bytes": root.capacity, "fixture_capacity_bytes": fixture.capacity,
            "sampled_available_bytes": root.available, "fixture_available_bytes": fixture.available,
        }),
    );
}

pub fn run() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Isolated::new(&f);
    let mut daemon = f.spawn_isolated("");
    let held = Held::start(&f, &s);
    let path = f.root.join("disk-pressure-workload");
    let mut writer = WriteFixture::new(path.clone());
    relate(&s, &path);
    let deadline = Instant::now() + WATCHDOG;
    while s.snapshot().cadence.mode == SamplingMode::Idle {
        assert!(
            Instant::now() < deadline,
            "running fixture did not activate the meter within 60 seconds"
        );
        thread::sleep(POLL);
    }
    let first = observe(&s, &mut writer, daemon.child.id());
    let second = observe(&s, &mut writer, daemon.child.id());
    assert!(second > first);
    drop(writer);
    assert!(!path.exists(), "owned disk workload remains after cleanup");
    held.finish();
    daemon.stop(libc::SIGTERM);
    f.report();
}

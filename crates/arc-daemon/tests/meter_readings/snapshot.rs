//! One `arcctl meter snapshot --json` read of the built CLI.
use crate::{
    journey::Sandbox,
    runtime::{child_watchdog, Fixture},
};
use arc_core::meter::{MeterSnapshot, MAX_SNAPSHOT_BYTES};

/// The typed snapshot the real daemon returned to the built `arcctl`. The
/// output is exactly one JSON document and a newline, with no diagnostics.
pub fn read(f: &Fixture, s: &Sandbox) -> MeterSnapshot {
    let output = child_watchdog::output(
        s.arcctl(f, &s.home).args(["meter", "snapshot", "--json"]),
        "meter snapshot",
    )
    .unwrap();
    assert!(
        output.status.success() && output.stderr.is_empty(),
        "{output:?}"
    );
    let json = output
        .stdout
        .strip_suffix(b"\n")
        .unwrap_or_else(|| panic!("no trailing newline: {output:?}"));
    assert!(json.len() <= MAX_SNAPSHOT_BYTES, "{} bytes", json.len());
    let mut value: serde_json::Value = serde_json::from_slice(json)
        .unwrap_or_else(|error| panic!("{error}: {}", String::from_utf8_lossy(json)));
    // The CLI adds `traffic` beside the v1 fields when the daemon advertises
    // it (MET-017); the v1 snapshot itself stays strictly typed.
    if let Some(traffic) = value
        .as_object_mut()
        .and_then(|object| object.remove(arc_ipc::meter::usage::SNAPSHOT_TRAFFIC_KEY))
    {
        serde_json::from_value::<arc_ipc::meter::usage::SnapshotTraffic>(traffic)
            .unwrap_or_else(|error| panic!("{error}: {}", String::from_utf8_lossy(json)));
    }
    serde_json::from_value(value)
        .unwrap_or_else(|error| panic!("{error}: {}", String::from_utf8_lossy(json)))
}

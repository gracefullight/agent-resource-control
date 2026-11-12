use super::*;
use serde_json::{json, Value};

fn fixture() -> MeterSnapshot {
    serde_json::from_str(include_str!("../../tests/fixtures/meter-snapshot.json")).unwrap()
}

#[test]
fn serialization_preserves_zero_and_each_unknown_reason() {
    let snapshot = fixture();
    assert!(snapshot.is_valid());
    assert_eq!(
        snapshot.host.memory_available_bytes.reading.value(),
        Some(&0)
    );
    assert_eq!(snapshot.host.cpu_utilization.reading.value(), None);
    assert_eq!(snapshot.host.memory_pressure.reading.value(), None);
    assert_eq!(snapshot.host.swap_used_bytes.reading.value(), None);
    assert_eq!(snapshot.host.filesystem.free_bytes.reading.value(), None);
    let value = serde_json::to_value(&snapshot).unwrap();
    assert_eq!(
        value["host"]["cpu_utilization"]["reading"],
        json!({"availability":"first_interval"})
    );
    for path in [
        "/host/cpu_utilization/reading",
        "/host/memory_pressure/reading",
        "/host/swap_used_bytes/reading",
        "/host/filesystem/free_bytes/reading",
    ] {
        assert!(value.pointer(path).unwrap().get("value").is_none());
    }
    let bytes = serde_json::to_vec(&snapshot).unwrap();
    assert!(bytes.len() < MAX_SNAPSHOT_BYTES);
    assert_eq!(
        serde_json::from_slice::<MeterSnapshot>(&bytes).unwrap(),
        snapshot
    );
}

#[test]
fn unknown_readings_cannot_smuggle_values_or_missing_available_values() {
    for availability in ["first_interval", "unsupported", "denied", "failed"] {
        let mut value = json!({"availability":availability, "value":0});
        if availability == "failed" {
            value["diagnostic"] = json!("native_failed");
        }
        assert!(serde_json::from_value::<Reading<u64>>(value).is_err());
    }
    for value in [
        json!({"availability":"available"}),
        json!({"availability":"available","value":null}),
        json!({"availability":"unrecognized"}),
    ] {
        assert!(serde_json::from_value::<Reading<u64>>(value).is_err());
    }
}

#[test]
fn diagnostic_limits_apply_before_retention_and_when_decoding() {
    let bounded = Diagnostic::bounded(&"측정\n".repeat(100));
    assert!(bounded.as_str().len() <= MAX_DIAGNOSTIC_BYTES);
    assert!(!bounded.as_str().chars().any(char::is_control));
    assert_eq!(Diagnostic::bounded("").as_str(), "measurement_failed");
    for message in [
        String::new(),
        "x".repeat(MAX_DIAGNOSTIC_BYTES + 1),
        "bad\ntext".to_owned(),
    ] {
        assert!(Diagnostic::try_from(message.clone()).is_err());
        assert!(serde_json::from_value::<Diagnostic>(json!(message)).is_err());
    }
    let diagnostic = Diagnostic::try_from("native_failed".to_owned()).unwrap();
    assert_eq!(String::from(diagnostic.clone()), "native_failed");
    assert_eq!(
        serde_json::to_value(diagnostic).unwrap(),
        json!("native_failed")
    );
}

fn interval_snapshot() -> MeterSnapshot {
    let mut snapshot = fixture();
    snapshot.sampled_monotonic_ms = 15_000;
    snapshot.sampled_at_ms = Some(1_699_999_000_000);
    snapshot.host.cpu_utilization.observed_monotonic_ms = 15_000;
    snapshot.host.cpu_utilization.reading = Reading::Available { value: 25.0 };
    snapshot.host.cpu_interval = Some(CpuInterval {
        started_at_ms: Some(1_700_000_000_000),
        ended_at_ms: snapshot.sampled_at_ms,
        started_monotonic_ms: 0,
        ended_monotonic_ms: 15_000,
        elapsed_ms: 15_000,
        logical_cpu_count: 8,
        normalization: CpuNormalization::HostCapacity,
    });
    snapshot
}

#[test]
fn cpu_interval_uses_monotonic_time_despite_wall_clock_changes() {
    let snapshot = interval_snapshot();
    assert!(snapshot.is_valid());
    let mut no_wall_clock = snapshot.clone();
    no_wall_clock.sampled_at_ms = None;
    no_wall_clock.host.cpu_utilization.observed_at_ms = None;
    no_wall_clock
        .host
        .cpu_interval
        .as_mut()
        .unwrap()
        .ended_at_ms = None;
    assert!(no_wall_clock.is_valid());
    for value in [f64::NAN, f64::INFINITY, -1.0, 100.1] {
        let mut invalid = snapshot.clone();
        invalid.host.cpu_utilization.reading = Reading::Available { value };
        assert!(!invalid.is_valid());
    }
    for value in [0.0, 100.0] {
        let mut valid = snapshot.clone();
        valid.host.cpu_utilization.reading = Reading::Available { value };
        assert!(valid.is_valid());
    }
    let mut invalid = snapshot.clone();
    invalid.host.cpu_interval.as_mut().unwrap().elapsed_ms = 14_999;
    assert!(!invalid.is_valid());
    invalid = snapshot.clone();
    invalid
        .host
        .cpu_interval
        .as_mut()
        .unwrap()
        .logical_cpu_count = 0;
    assert!(!invalid.is_valid());
    invalid = snapshot.clone();
    invalid.host.cpu_interval = None;
    assert!(!invalid.is_valid());
    invalid = fixture();
    invalid.host.cpu_interval = snapshot.host.cpu_interval;
    assert!(!invalid.is_valid());
}

fn filesystem_snapshot() -> MeterSnapshot {
    let mut snapshot = fixture();
    let filesystem = &mut snapshot.host.filesystem;
    filesystem.identity = Some(FilesystemIdentity {
        path: "/".to_owned(),
        mount_point: "/".to_owned(),
        filesystem_type: "apfs".to_owned(),
        volume_id: "1:2".to_owned(),
        is_local: true,
    });
    filesystem.capacity_bytes.reading = Reading::Available { value: 1000 };
    filesystem.free_bytes.reading = Reading::Available { value: 200 };
    filesystem.available_bytes.reading = Reading::Available { value: 150 };
    filesystem.pressure.reading = Reading::Available {
        value: DiskPressure::Normal,
    };
    snapshot
}

#[test]
fn filesystem_values_require_local_identity_and_label_derived_pressure() {
    let snapshot = filesystem_snapshot();
    assert!(snapshot.is_valid());
    for (available, pressure) in [
        (50, DiskPressure::Critical),
        (100, DiskPressure::Warning),
        (101, DiskPressure::Normal),
    ] {
        let mut valid = snapshot.clone();
        valid.host.filesystem.available_bytes.reading = Reading::Available { value: available };
        valid.host.filesystem.pressure.reading = Reading::Available { value: pressure };
        assert!(valid.is_valid());
    }
    for pointer in [
        "/host/filesystem/identity/is_local",
        "/host/filesystem/identity/path",
        "/host/filesystem/identity/volume_id",
    ] {
        let mut value = serde_json::to_value(&snapshot).unwrap();
        *value.pointer_mut(pointer).unwrap() = if pointer.ends_with("is_local") {
            json!(false)
        } else {
            json!("")
        };
        assert!(!serde_json::from_value::<MeterSnapshot>(value)
            .unwrap()
            .is_valid());
    }
    for (pointer, value) in [
        ("/host/filesystem/identity", Value::Null),
        ("/host/filesystem/free_bytes/reading/value", json!(1001)),
        ("/host/filesystem/available_bytes/reading/value", json!(201)),
        ("/host/filesystem/capacity_bytes/reading/value", json!(0)),
        ("/host/filesystem/pressure/reading/value", json!("critical")),
        ("/host/filesystem/pressure/provenance", json!("native")),
    ] {
        let mut invalid = serde_json::to_value(&snapshot).unwrap();
        *invalid.pointer_mut(pointer).unwrap() = value;
        assert!(!serde_json::from_value::<MeterSnapshot>(invalid)
            .unwrap()
            .is_valid());
    }
}

#[test]
fn snapshot_rejects_bad_metadata_units_peaks_and_unbounded_numbers() {
    let snapshot = fixture();
    for (pointer, value) in [
        ("/schema_version", json!(2)),
        ("/sequence", json!(0)),
        ("/sequence", json!(MAX_SAFE_INTEGER + 1)),
        ("/age_ms", json!(MAX_SAFE_INTEGER + 1)),
        ("/sampled_at_ms", json!(MAX_SAFE_INTEGER + 1)),
        ("/sampled_monotonic_ms", json!(MAX_SAFE_INTEGER + 1)),
        ("/host/memory_available_bytes/unit", json!("percent")),
        (
            "/host/memory_available_bytes/observed_monotonic_ms",
            json!(1),
        ),
        (
            "/host/memory_available_bytes/observed_at_ms",
            json!(MAX_SAFE_INTEGER + 1),
        ),
        (
            "/host/memory_available_bytes/reading/value",
            json!(MAX_SAFE_INTEGER + 1),
        ),
        (
            "/host/memory_available_bytes/reading",
            json!({"availability":"first_interval"}),
        ),
        ("/daemon/cpu_core_seconds/reading/value", json!(-1.0)),
        ("/daemon/rss_bytes/reading/value", json!(1025)),
        ("/daemon/peak_observation_started_monotonic_ms", json!(1)),
        (
            "/daemon/peak_observation_started_at_ms",
            json!(MAX_SAFE_INTEGER + 1),
        ),
    ] {
        let mut invalid = serde_json::to_value(&snapshot).unwrap();
        *invalid.pointer_mut(pointer).unwrap() = value;
        assert!(
            !serde_json::from_value::<MeterSnapshot>(invalid)
                .unwrap()
                .is_valid(),
            "{pointer}"
        );
    }
    for mode in [
        SamplingMode::Idle,
        SamplingMode::Active,
        SamplingMode::Pressure,
    ] {
        assert!(SamplingCadence::new(mode, u64::MAX).is_err());
        assert!(SamplingCadence::new(mode, 0).is_err());
    }
    let mut invalid = snapshot;
    invalid.cadence.stale_after_ms = 1;
    assert!(!invalid.is_valid());
}

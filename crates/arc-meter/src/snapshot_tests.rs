use super::*;
use crate::fixtures::{identity, sample};

fn take(sampler: &mut Sampler, sample: NativeSample, active: bool) -> MeterSnapshot {
    let snapshot = sampler
        .observe(sample, active, SamplingPolicy::default())
        .unwrap();
    assert!(snapshot.is_valid(), "{snapshot:?}");
    snapshot
}
#[test]
fn every_field_has_time_units_provenance_and_legitimate_zero_is_available() {
    assert!(Sampler::new(ProcessIdentity {
        pid: 0,
        ..identity("1")
    })
    .is_err());
    let mut sampler = Sampler::new(identity("1")).unwrap();
    let first = take(&mut sampler, sample(0), false);
    assert_eq!(
        first.host.cpu_utilization.reading,
        Reading::FirstInterval {}
    );
    assert_eq!(first.host.swap_used_bytes.reading.value(), Some(&0));
    assert_eq!(first.host.swap_used_bytes.unit, Unit::Bytes);
    assert_eq!(first.host.memory_pressure.provenance, Provenance::Native);
    assert_eq!(
        first.host.filesystem.pressure.provenance,
        Provenance::Derived
    );
    assert_eq!(first.daemon.cpu_core_seconds.unit, Unit::CoreSeconds);
    let next = take(&mut sampler, sample(1000), true);
    assert_eq!(next.sequence, 2);
    assert_eq!(next.host.cpu_utilization.reading.value(), Some(&50.0));
    assert_eq!(next.cadence.mode, SamplingMode::Active);
    assert_eq!(
        next.host.memory_available_bytes.observed_at_ms,
        Some(11_000)
    );
    assert_eq!(next.host.memory_available_bytes.observed_monotonic_ms, 1000);
}

#[test]
fn partial_failure_preserves_usable_fields_and_peak_keeps_its_real_observation_time() {
    let mut sampler = Sampler::new(identity("1")).unwrap();
    take(&mut sampler, sample(0), false);
    let mut failed_sample = sample(1000);
    failed_sample.daemon_rss_bytes = Reading::Denied {};
    failed_sample.memory_pressure = Reading::Unsupported {};
    failed_sample.host_cpu = Reading::Failed {
        diagnostic: Diagnostic::bounded("cpu failed"),
    };
    let failed = take(&mut sampler, failed_sample, false);
    assert_eq!(failed.daemon.rss_bytes.reading, Reading::Denied {});
    assert_eq!(
        failed.daemon.observed_peak_rss_bytes.reading.value(),
        Some(&4096)
    );
    assert_eq!(
        failed.daemon.observed_peak_rss_bytes.observed_monotonic_ms,
        0
    );
    assert_eq!(
        failed.daemon.observed_peak_rss_bytes.observed_at_ms,
        Some(10_000)
    );
    assert_eq!(
        failed.host.memory_available_bytes.reading.value(),
        Some(&1024)
    );
    assert_eq!(failed.host.memory_pressure.reading, Reading::Unsupported {});
    let mut higher = sample(2000);
    higher.daemon_rss_bytes = Reading::Available { value: 8192 };
    assert_eq!(
        take(&mut sampler, higher, false)
            .daemon
            .observed_peak_rss_bytes
            .reading
            .value(),
        Some(&8192)
    );
    let recovered = take(&mut sampler, sample(3000), false);
    assert_eq!(
        recovered.daemon.observed_peak_rss_bytes.reading.value(),
        Some(&8192)
    );
    assert_eq!(
        recovered
            .daemon
            .observed_peak_rss_bytes
            .observed_monotonic_ms,
        3000
    );
}

#[test]
fn sampled_peak_unknown_until_first_success_and_restart_resets_the_window() {
    let mut sampler = Sampler::new(identity("1")).unwrap();
    let mut initial = sample(50);
    initial.daemon_rss_bytes = Reading::Unsupported {};
    let first = take(&mut sampler, initial, false);
    assert_eq!(
        first.daemon.observed_peak_rss_bytes.reading,
        Reading::Unsupported {}
    );
    let mut high = sample(1000);
    high.daemon_rss_bytes = Reading::Available { value: 8192 };
    take(&mut sampler, high, false);
    let mut restarted = Sampler::new(identity("2")).unwrap();
    let first = take(&mut restarted, sample(0), false);
    assert_eq!(first.sequence, 1);
    assert_eq!(first.daemon_instance.start_token, "2");
    assert_eq!(first.daemon.peak_observation_started_monotonic_ms, 0);
    assert_eq!(
        first.daemon.observed_peak_rss_bytes.reading.value(),
        Some(&4096)
    );
}

#[test]
fn wall_clock_is_optional_and_regressing_monotonic_observations_do_not_replace_cache() {
    let mut sampler = Sampler::new(identity("1")).unwrap();
    take(&mut sampler, sample(1000), false);
    assert!(sampler
        .observe(sample(999), false, SamplingPolicy::default())
        .is_none());
    let mut next = sample(2000);
    next.observed_at_ms = Some(1);
    let next = take(&mut sampler, next, false);
    assert_eq!(next.host.cpu_utilization.reading, Reading::FirstInterval {});
    let mut next = sample(3000);
    next.observed_at_ms = Some(MAX_SAFE_INTEGER + 1);
    let next = take(&mut sampler, next, false);
    assert_eq!(next.sampled_at_ms, None);
    assert_eq!(next.host.cpu_interval.unwrap().elapsed_ms, 1000);
    assert!(sampler
        .observe(
            sample(MAX_SAFE_INTEGER + 1),
            false,
            SamplingPolicy::default()
        )
        .is_none());
    sampler.sequence = MAX_SAFE_INTEGER;
    assert!(sampler
        .observe(sample(4000), false, SamplingPolicy::default())
        .is_none());
}

#[test]
fn impossible_native_values_do_not_poison_other_measurements() {
    let mut sampler = Sampler::new(identity("1")).unwrap();
    take(&mut sampler, sample(1000), false);
    for value in [
        f64::NAN,
        f64::INFINITY,
        -1.0,
        MAX_SAFE_INTEGER as f64 + 1.0,
        0.0,
    ] {
        let mut input = sample(2000);
        input.daemon_cpu_core_seconds = Reading::Available { value };
        input.memory_available_bytes = Reading::Available { value: u64::MAX };
        input.memory_pressure = Reading::FirstInterval {};
        let result = take(&mut sampler, input, false);
        assert!(matches!(
            result.daemon.cpu_core_seconds.reading,
            Reading::Failed { .. }
        ));
        assert!(matches!(
            result.host.memory_available_bytes.reading,
            Reading::Failed { .. }
        ));
        assert!(matches!(
            result.host.memory_pressure.reading,
            Reading::Failed { .. }
        ));
        assert_eq!(result.daemon.rss_bytes.reading.value(), Some(&4096));
    }
    let mut first_interval = sample(3000);
    first_interval.daemon_cpu_core_seconds = Reading::FirstInterval {};
    assert!(matches!(
        take(&mut sampler, first_interval, false)
            .daemon
            .cpu_core_seconds
            .reading,
        Reading::Failed { .. }
    ));
}

#[test]
fn disk_judgments_are_derived_and_native_pressure_controls_only_sampling() {
    let mut sampler = Sampler::new(identity("1")).unwrap();
    for (available, expected) in [
        (0, DiskPressure::Critical),
        (50, DiskPressure::Critical),
        (51, DiskPressure::Warning),
        (100, DiskPressure::Warning),
        (101, DiskPressure::Normal),
    ] {
        let mut input = sample(1000);
        input.filesystem.available_bytes = Reading::Available { value: available };
        let result = take(&mut sampler, input, false);
        assert_eq!(
            result.host.filesystem.pressure.reading.value(),
            Some(&expected)
        );
        assert_eq!(result.cadence.mode, SamplingMode::Idle);
    }
    for pressure in [MemoryPressure::Warning, MemoryPressure::Critical] {
        let mut input = sample(2000);
        input.memory_pressure = Reading::Available { value: pressure };
        assert_eq!(
            take(&mut sampler, input, false).cadence.mode,
            SamplingMode::Pressure
        );
    }
}

#[test]
fn absent_identity_partial_filesystem_failure_and_impossible_space_stay_unknown() {
    for case in 0..6 {
        let mut sampler = Sampler::new(identity("1")).unwrap();
        let mut input = sample(0);
        match case {
            0 => input.filesystem.identity = None,
            1 => input.filesystem.identity.as_mut().unwrap().is_local = false,
            2 => input.filesystem.capacity_bytes = Reading::Available { value: 0 },
            3 => input.filesystem.free_bytes = Reading::Available { value: 1001 },
            4 => input.filesystem.available_bytes = Reading::Available { value: 201 },
            _ => input.filesystem.capacity_bytes = Reading::Denied {},
        }
        let result = take(&mut sampler, input, false);
        if case != 3 {
            assert!(result.host.filesystem.pressure.reading.value().is_none());
        }
        if case == 3 {
            assert!(result.host.filesystem.free_bytes.reading.value().is_none());
        }
        assert_eq!(
            result.host.memory_available_bytes.reading.value(),
            Some(&1024)
        );
    }
}

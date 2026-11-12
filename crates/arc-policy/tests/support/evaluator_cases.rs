use super::support::*;
use arc_core::meter::*;
use arc_policy::*;

#[test]
fn every_pressure_and_unavailable_reading_has_a_truthful_versioned_result() {
    for (reading, mode, reason, availability) in [
        (
            Reading::Available {
                value: MemoryPressure::Normal,
            },
            PolicyMode::L0,
            PolicyReason::PressureNormal,
            ObservationAvailability::Available,
        ),
        (
            Reading::Available {
                value: MemoryPressure::Warning,
            },
            PolicyMode::L0,
            PolicyReason::PressureWarning,
            ObservationAvailability::Available,
        ),
        (
            Reading::Available {
                value: MemoryPressure::Critical,
            },
            PolicyMode::MemoryHold,
            PolicyReason::MemoryCritical,
            ObservationAvailability::Available,
        ),
        (
            Reading::Unsupported {},
            PolicyMode::L0,
            PolicyReason::ObservationUnsupported,
            ObservationAvailability::Unsupported,
        ),
        (
            Reading::Denied {},
            PolicyMode::L0,
            PolicyReason::ObservationDenied,
            ObservationAvailability::Denied,
        ),
        (
            Reading::Failed {
                diagnostic: Diagnostic::bounded("failed"),
            },
            PolicyMode::L0,
            PolicyReason::ObservationFailed,
            ObservationAvailability::Failed,
        ),
    ] {
        let mut state = State::new(MemoryPressure::Critical);
        state.sample = Some(snapshot(reading));
        let result = state.policy();
        assert!(result.is_valid(), "{result:?}");
        assert_eq!(
            (result.mode, result.reason, result.observation.availability),
            (mode, reason, availability)
        );
        assert_eq!(result.profile, L0_PROFILE);
        assert_eq!(result.policy_version, POLICY_VERSION);
        if availability != ObservationAvailability::Available {
            assert_eq!(result.observation.pressure, None);
        }
    }
}
#[test]
fn pressure_field_age_is_independent_of_sample_health_and_exact_boundary_is_fresh() {
    let mut state = State::new(MemoryPressure::Critical);
    state
        .sample
        .as_mut()
        .unwrap()
        .host
        .memory_pressure
        .observed_monotonic_ms = 900;
    state.time(3900);
    let exact = state.policy();
    assert_eq!(exact.mode, PolicyMode::MemoryHold);
    assert_eq!(exact.observation.age_ms, Some(3000));
    assert_eq!(state.sample.as_ref().unwrap().age_ms, 2900);
    state.time(3901);
    let stale = state.policy();
    assert!(stale.is_valid());
    assert_eq!(stale.reason, PolicyReason::ObservationStale);
    assert_eq!(stale.mode, PolicyMode::L0);
    assert_eq!(stale.observation.pressure, Some(MemoryPressure::Critical));
    assert_eq!(stale.observation.age_ms, Some(3001));
}
#[test]
fn failure_stop_and_missing_do_not_keep_cached_critical_alive() {
    for (status, reason) in [
        (SamplerStatus::Missing, PolicyReason::ObservationMissing),
        (SamplerStatus::Failed, PolicyReason::ObservationFailed),
        (SamplerStatus::Stopped, PolicyReason::ObservationFailed),
    ] {
        let mut state = State::new(MemoryPressure::Critical);
        state.status = status;
        let result = state.policy();
        assert!(result.is_valid());
        assert_eq!(result.mode, PolicyMode::L0);
        assert_eq!(result.reason, reason);
        assert_eq!(result.observation.pressure, None);
    }
    let mut state = State::new(MemoryPressure::Critical);
    state.sample = None;
    assert_eq!(state.policy().reason, PolicyReason::ObservationMissing);
    state.supported = false;
    assert_eq!(state.policy().reason, PolicyReason::ObservationUnsupported);
}
#[test]
fn identities_epoch_generation_and_sequence_are_validated() {
    for mutate in [
        |s: &mut State| s.published.generation += 1,
        |s: &mut State| s.published.monotonic_epoch += 1,
        |s: &mut State| s.active.generation = 0,
        |s: &mut State| s.sample.as_mut().unwrap().daemon_instance.start_token = "789".into(),
    ] {
        let mut state = State::new(MemoryPressure::Critical);
        mutate(&mut state);
        let result = state.policy();
        assert!(result.is_valid());
        assert_eq!(result.reason, PolicyReason::SamplerMismatch);
    }
    let mut state = State::new(MemoryPressure::Normal);
    state.cursor = Some(ObservationCursor {
        sampler: state.active.clone(),
        sequence: 2,
        sampled_monotonic_ms: 1000,
    });
    assert_eq!(state.policy().reason, PolicyReason::ObservationInvalid);
    state.sample.as_mut().unwrap().sequence = 2;
    assert_eq!(state.policy().reason, PolicyReason::PressureNormal);
    state.sample.as_mut().unwrap().sampled_monotonic_ms = 999;
    state.time(1000);
    assert_eq!(state.policy().reason, PolicyReason::ClockInvalid);
    state
        .sample
        .as_mut()
        .unwrap()
        .host
        .memory_pressure
        .observed_monotonic_ms = 999;
    assert_eq!(state.policy().reason, PolicyReason::ObservationInvalid);
    // New generation is a new bounded cache; cursor/sample are cleared together.
    state.active.generation += 1;
    state.published = state.active.clone();
    state.sample = None;
    state.cursor = None;
    assert_eq!(state.policy().reason, PolicyReason::ObservationMissing);
    state.sample = Some(snapshot(Reading::Available {
        value: MemoryPressure::Normal,
    }));
    assert_eq!(state.policy().reason, PolicyReason::PressureNormal);
}
#[test]
fn wall_clock_changes_cannot_refresh_and_invalid_monotonic_arithmetic_falls_back() {
    let mut state = State::new(MemoryPressure::Critical);
    state.time(4001);
    for wall in [None, Some(0), Some(9_000_000_000)] {
        let sample = state.sample.as_mut().unwrap();
        sample.sampled_at_ms = wall;
        sample.host.memory_pressure.observed_at_ms = wall;
        assert_eq!(state.policy().reason, PolicyReason::ObservationStale);
    }
    for mutate in [
        |s: &mut State| s.now = None,
        |s: &mut State| s.now = Some(MAX_SAFE_INTEGER + 1),
        |s: &mut State| s.now = Some(999),
        |s: &mut State| s.sample.as_mut().unwrap().age_ms = 0,
        |s: &mut State| {
            s.sample
                .as_mut()
                .unwrap()
                .host
                .memory_pressure
                .observed_monotonic_ms = 1001
        },
    ] {
        let mut state = State::new(MemoryPressure::Critical);
        state.time(1001);
        mutate(&mut state);
        let result = state.policy();
        assert!(result.is_valid());
        assert_eq!(result.reason, PolicyReason::ClockInvalid);
    }
}
#[test]
fn malformed_cadence_safe_numbers_and_snapshot_are_never_authority() {
    for mutate in [
        |s: &mut MeterSnapshot| s.sequence = 0,
        |s: &mut MeterSnapshot| s.sequence = MAX_SAFE_INTEGER + 1,
        |s: &mut MeterSnapshot| s.cadence.stale_after_ms += 1,
        |s: &mut MeterSnapshot| s.cadence.interval_ms = 999,
        |s: &mut MeterSnapshot| s.host.memory_pressure.reading = Reading::FirstInterval {},
        |s: &mut MeterSnapshot| s.schema_version = 2,
    ] {
        let mut state = State::new(MemoryPressure::Critical);
        mutate(state.sample.as_mut().unwrap());
        let result = state.policy();
        assert!(result.is_valid());
        assert_eq!(result.reason, PolicyReason::ObservationInvalid);
    }
    let mut calls = 0;
    evaluator::MissingObservation.with_observation(&mut |input| {
        calls += 1;
        assert!(evaluator::evaluate(input).is_valid());
    });
    assert_eq!(calls, 1);
}

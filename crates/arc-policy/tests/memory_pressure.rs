//! Foundation contract checks. Task 2 adds evaluator/pressure acceptance here.
use arc_core::{meter::*, process_identity::ProcessIdentity};
use arc_policy::*;

fn observation() -> ObservationSummary {
    ObservationSummary {
        availability: ObservationAvailability::Available,
        pressure: Some(MemoryPressure::Critical),
        age_ms: Some(3000),
        stale_after_ms: Some(3000),
        sampler: Some(SamplerIdentity {
            daemon_instance: ProcessIdentity {
                pid: 123,
                start_token: "456".into(),
                boot_id: "00000000-0000-0000-0000-000000000001".into(),
            },
            generation: 1,
            monotonic_epoch: 1,
        }),
        sequence: Some(1),
        observed_at_ms: None,
        observed_monotonic_ms: Some(0),
    }
}
fn policy() -> EffectivePolicy {
    EffectivePolicy {
        schema_version: POLICY_SCHEMA_VERSION,
        policy_version: POLICY_VERSION.into(),
        profile: L0_PROFILE.into(),
        mode: PolicyMode::MemoryHold,
        reason: PolicyReason::MemoryCritical,
        observation: observation(),
        limits: PolicyLimits::default(),
    }
}
#[test]
fn current_contract_accepts_exact_freshness_boundary_and_requires_stale_fallback() {
    let mut value = policy();
    assert!(value.is_valid());
    value.observation.age_ms = Some(3001);
    assert!(!value.is_valid());
    value.observation.availability = ObservationAvailability::Stale;
    assert!(
        !value.is_valid(),
        "stale Critical cannot claim a current hold"
    );
    value.mode = PolicyMode::L0;
    value.reason = PolicyReason::ObservationStale;
    assert!(value.is_valid());
    assert_eq!(value.observation.pressure, Some(MemoryPressure::Critical));
}
#[test]
fn fallback_availability_is_explicit_and_cannot_be_fabricated_normal() {
    for (availability, reason) in [
        (
            ObservationAvailability::Missing,
            PolicyReason::ObservationMissing,
        ),
        (
            ObservationAvailability::Failed,
            PolicyReason::ObservationFailed,
        ),
        (
            ObservationAvailability::Unsupported,
            PolicyReason::ObservationUnsupported,
        ),
        (
            ObservationAvailability::Denied,
            PolicyReason::ObservationDenied,
        ),
        (
            ObservationAvailability::Invalid,
            PolicyReason::ObservationInvalid,
        ),
        (
            ObservationAvailability::Invalid,
            PolicyReason::SamplerMismatch,
        ),
        (ObservationAvailability::Invalid, PolicyReason::ClockInvalid),
    ] {
        let mut value = policy();
        value.mode = PolicyMode::L0;
        value.reason = reason;
        value.observation.availability = availability;
        value.observation.pressure = None;
        value.observation.age_ms = None;
        value.observation.sequence = None;
        assert!(value.is_valid(), "{reason:?}");
        value.observation.pressure = Some(MemoryPressure::Normal);
        assert!(!value.is_valid(), "{reason:?}");
    }
}
#[test]
fn current_policy_rejects_unsafe_identities_versions_and_changed_limits() {
    let original = policy();
    for mutate in [
        |value: &mut EffectivePolicy| value.schema_version = 2,
        |value: &mut EffectivePolicy| value.policy_version = "other".into(),
        |value: &mut EffectivePolicy| value.profile = "other".into(),
        |value: &mut EffectivePolicy| value.limits.build.window = 2,
        |value: &mut EffectivePolicy| value.observation.age_ms = Some(MAX_SAFE_INTEGER + 1),
        |value: &mut EffectivePolicy| value.observation.sequence = Some(0),
        |value: &mut EffectivePolicy| value.observation.sampler.as_mut().unwrap().generation = 0,
        |value: &mut EffectivePolicy| {
            value.observation.sampler.as_mut().unwrap().monotonic_epoch = 0
        },
        |value: &mut EffectivePolicy| {
            value
                .observation
                .sampler
                .as_mut()
                .unwrap()
                .daemon_instance
                .pid = 0
        },
    ] {
        let mut value = original.clone();
        mutate(&mut value);
        assert!(!value.is_valid());
    }
    let limits = PolicyLimits::default();
    assert_eq!(
        (limits.build.window, limits.test.window, limits.lint.window),
        (1, 1, 1)
    );
    assert_eq!(limits.budget.memory_bytes, 4 * 1024 * 1024 * 1024);
    assert_eq!(
        DECISION_RETENTION_MS,
        arc_core::job_meter::JOB_METADATA_RETENTION_MS
    );
}
#[test]
fn decision_contract_preserves_legacy_waits_and_rejects_inconsistent_hold_records() {
    assert_eq!(
        DecisionReason::MemoryPressure.legacy_wait(),
        Some(arc_traffic::WaitReason::MemoryBudget)
    );
    let mut value = RecordedDecision {
        policy_version: POLICY_VERSION.into(),
        kind: DecisionKind::Wait,
        reason: DecisionReason::MemoryPressure,
        decided_at_ms: 5000,
        permit_generation: None,
        observation: observation(),
    };
    assert!(value.is_valid());
    value.observation.pressure = Some(MemoryPressure::Normal);
    assert!(!value.is_valid());
    value.kind = DecisionKind::Admit;
    value.reason = DecisionReason::Admitted;
    value.permit_generation = Some(1);
    assert!(value.is_valid());
    value.permit_generation = Some(0);
    assert!(!value.is_valid());
}
#[test]
fn bounded_provider_is_object_safe_and_invokes_the_missing_observation_callback() {
    struct Missing;
    impl ObservationProvider for Missing {
        fn with_observation(&self, consume: &mut dyn FnMut(ObservationInput<'_>)) {
            consume(ObservationInput {
                supported: true,
                sampler_status: SamplerStatus::Missing,
                active_sampler: None,
                published_sampler: None,
                snapshot: None,
                now_monotonic_ms: None,
                high_water: None,
            });
        }
    }
    let provider: &dyn ObservationProvider = &Missing;
    let mut calls = 0;
    provider.with_observation(&mut |input| {
        calls += 1;
        assert!(input.snapshot.is_none());
    });
    assert_eq!(calls, 1);
}

#[path = "support/evaluator_cases.rs"]
mod evaluator_cases;
#[path = "support/memory_observation.rs"]
mod support;

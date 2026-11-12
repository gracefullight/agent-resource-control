//! Synthetic contract fixture, never a native sampler/daemon acceptance result.
use arc_ipc::policy::*;

pub fn fallback() -> EffectivePolicy {
    EffectivePolicy {
        schema_version: POLICY_SCHEMA_VERSION,
        policy_version: POLICY_VERSION.into(),
        profile: L0_PROFILE.into(),
        mode: PolicyMode::L0,
        reason: PolicyReason::ObservationMissing,
        observation: ObservationSummary {
            availability: ObservationAvailability::Missing,
            pressure: None,
            age_ms: None,
            stale_after_ms: None,
            sampler: None,
            sequence: None,
            observed_at_ms: None,
            observed_monotonic_ms: None,
        },
        limits: PolicyLimits::default(),
    }
}

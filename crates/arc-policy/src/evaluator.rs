//! Pure evaluation of a coherent, borrowed observation. No I/O or wall-clock age.
use crate::*;
use arc_core::meter::{MemoryPressure, Reading, MAX_SAFE_INTEGER};

pub type EvaluateMemory = for<'a> fn(ObservationInput<'a>) -> EffectivePolicy;

pub fn evaluate(input: ObservationInput<'_>) -> EffectivePolicy {
    use ObservationAvailability as A;
    use PolicyReason as R;
    let mut observation = ObservationSummary {
        availability: A::Missing,
        pressure: None,
        age_ms: None,
        stale_after_ms: None,
        sampler: input.active_sampler.filter(|v| v.is_valid()).cloned(),
        sequence: None,
        observed_at_ms: None,
        observed_monotonic_ms: None,
    };
    let unavailable = if !input.supported {
        Some((A::Unsupported, R::ObservationUnsupported))
    } else {
        match input.sampler_status {
            SamplerStatus::Missing => Some((A::Missing, R::ObservationMissing)),
            SamplerStatus::Failed | SamplerStatus::Stopped => {
                Some((A::Failed, R::ObservationFailed))
            }
            SamplerStatus::Running => None,
        }
    };
    if let Some((availability, reason)) = unavailable {
        observation.availability = availability;
        return result(observation, reason);
    }
    let Some(sample) = input.snapshot else {
        return result(observation, R::ObservationMissing);
    };
    let invalid = |mut observation: ObservationSummary, reason| {
        observation.availability = A::Invalid;
        result(observation, reason)
    };
    let Some(active) = input.active_sampler.filter(|v| v.is_valid()) else {
        return invalid(observation, R::SamplerMismatch);
    };
    if input.published_sampler != Some(active) || sample.daemon_instance != active.daemon_instance {
        return invalid(observation, R::SamplerMismatch);
    }
    let Some(now) = input
        .now_monotonic_ms
        .filter(|now| *now <= MAX_SAFE_INTEGER)
    else {
        return invalid(observation, R::ClockInvalid);
    };
    let field = &sample.host.memory_pressure;
    if now.checked_sub(sample.sampled_monotonic_ms) != Some(sample.age_ms)
        || field.observed_monotonic_ms > sample.sampled_monotonic_ms
    {
        return invalid(observation, R::ClockInvalid);
    }
    if !sample.is_valid() {
        return invalid(observation, R::ObservationInvalid);
    }
    if let Some(cursor) = input.high_water {
        if cursor.sampler != *active
            || sample.sequence < cursor.sequence
            || sample.sampled_monotonic_ms < cursor.sampled_monotonic_ms
            || (sample.sequence == cursor.sequence
                && sample.sampled_monotonic_ms != cursor.sampled_monotonic_ms)
        {
            return invalid(observation, R::ObservationInvalid);
        }
    }
    observation.stale_after_ms = Some(sample.cadence.stale_after_ms);
    observation.sequence = Some(sample.sequence);
    observation.observed_at_ms = field.observed_at_ms;
    observation.observed_monotonic_ms = Some(field.observed_monotonic_ms);
    let reason = match &field.reading {
        Reading::Available { value } => {
            let age = now - field.observed_monotonic_ms;
            observation.age_ms = Some(age);
            observation.pressure = Some(*value);
            if age > sample.cadence.stale_after_ms {
                observation.availability = A::Stale;
                R::ObservationStale
            } else {
                observation.availability = A::Available;
                match value {
                    MemoryPressure::Normal => R::PressureNormal,
                    MemoryPressure::Warning => R::PressureWarning,
                    MemoryPressure::Critical => R::MemoryCritical,
                }
            }
        }
        Reading::Unsupported {} => {
            observation.availability = A::Unsupported;
            R::ObservationUnsupported
        }
        Reading::Denied {} => {
            observation.availability = A::Denied;
            R::ObservationDenied
        }
        Reading::Failed { .. } => {
            observation.availability = A::Failed;
            R::ObservationFailed
        }
        Reading::FirstInterval {} => unreachable!("snapshot validation rejects first interval"),
    };
    result(observation, reason)
}

fn result(observation: ObservationSummary, reason: PolicyReason) -> EffectivePolicy {
    EffectivePolicy {
        schema_version: POLICY_SCHEMA_VERSION,
        policy_version: POLICY_VERSION.into(),
        profile: L0_PROFILE.into(),
        mode: if reason == PolicyReason::MemoryCritical {
            PolicyMode::MemoryHold
        } else {
            PolicyMode::L0
        },
        reason,
        observation,
        limits: PolicyLimits::default(),
    }
}

/// Explicit fallback for owners without a sampler, also used by legacy APIs.
#[derive(Default)]
pub struct MissingObservation;
impl ObservationProvider for MissingObservation {
    fn with_observation(&self, consume: &mut dyn FnMut(ObservationInput<'_>)) {
        consume(ObservationInput {
            supported: cfg!(target_os = "macos"),
            sampler_status: SamplerStatus::Missing,
            active_sampler: None,
            published_sampler: None,
            snapshot: None,
            now_monotonic_ms: None,
            high_water: None,
        });
    }
}

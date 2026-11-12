use arc_core::{
    meter::{MemoryPressure, MeterSnapshot, MAX_SAFE_INTEGER},
    process_identity::ProcessIdentity,
};
use serde::{Deserialize, Serialize};

/// Internal cache identity, separate from the unchanged Meter wire schema.
/// Generation changes on sampler replacement; epoch changes on clock reset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SamplerIdentity {
    pub daemon_instance: ProcessIdentity,
    pub generation: u64,
    pub monotonic_epoch: u64,
}
impl SamplerIdentity {
    pub fn is_valid(&self) -> bool {
        self.daemon_instance.is_valid()
            && (1..=MAX_SAFE_INTEGER).contains(&self.generation)
            && (1..=MAX_SAFE_INTEGER).contains(&self.monotonic_epoch)
    }
}

/// One high-water mark, never a sequence-indexed history. Duplicate reads of
/// the identical publication are valid; sequence/time regression is not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservationCursor {
    pub sampler: SamplerIdentity,
    pub sequence: u64,
    pub sampled_monotonic_ms: u64,
}

/// Health belongs to the cache owner, not to a retained snapshot. A stopped or
/// failed worker cannot keep an otherwise fresh cached Critical hold active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SamplerStatus {
    Missing,
    Running,
    Failed,
    Stopped,
}

/// Borrowed only while the provider's publication fence is held. The active
/// identity is configured by the daemon, never inferred from a cached sample.
#[derive(Clone, Copy)]
pub struct ObservationInput<'a> {
    pub supported: bool,
    pub sampler_status: SamplerStatus,
    pub active_sampler: Option<&'a SamplerIdentity>,
    pub published_sampler: Option<&'a SamplerIdentity>,
    pub snapshot: Option<&'a MeterSnapshot>,
    pub now_monotonic_ms: Option<u64>,
    pub high_water: Option<&'a ObservationCursor>,
}

/// Bounded, synchronous cache access. Invoke exactly once, including when no
/// reading exists. Acquire after SQLite's IMMEDIATE lock, retain through the
/// permit/audit commit, and release before returning the grant. Never sample
/// natively here. Publication takes the same fence; lock order is writer then
/// publication, and the publisher never takes the writer lock.
pub trait ObservationProvider: Send + Sync {
    fn with_observation(&self, consume: &mut dyn FnMut(ObservationInput<'_>));
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ObservationAvailability {
    Available,
    Missing,
    Stale,
    Failed,
    Unsupported,
    Denied,
    Invalid,
}

/// Observation facts at a decision/read, not a renewed observation timestamp.
/// A stale Critical value may remain visible while effective admission uses L0.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationSummary {
    pub availability: ObservationAvailability,
    pub pressure: Option<MemoryPressure>,
    pub age_ms: Option<u64>,
    pub stale_after_ms: Option<u64>,
    pub sampler: Option<SamplerIdentity>,
    pub sequence: Option<u64>,
    pub observed_at_ms: Option<u64>,
    pub observed_monotonic_ms: Option<u64>,
}
impl ObservationSummary {
    pub fn is_valid(&self) -> bool {
        if [
            self.age_ms,
            self.stale_after_ms,
            self.observed_at_ms,
            self.observed_monotonic_ms,
        ]
        .into_iter()
        .flatten()
        .any(|value| value > MAX_SAFE_INTEGER)
            || self
                .sampler
                .as_ref()
                .is_some_and(|sampler| !sampler.is_valid())
            || self
                .sequence
                .is_some_and(|sequence| !(1..=MAX_SAFE_INTEGER).contains(&sequence))
        {
            return false;
        }
        match self.availability {
            ObservationAvailability::Available | ObservationAvailability::Stale => {
                self.pressure.is_some()
                    && self.sampler.is_some()
                    && self.sequence.is_some()
                    && self.observed_monotonic_ms.is_some()
                    && self
                        .age_ms
                        .zip(self.stale_after_ms)
                        .is_some_and(|(age, threshold)| {
                            threshold > 0
                                && (age > threshold)
                                    == (self.availability == ObservationAvailability::Stale)
                        })
            }
            ObservationAvailability::Missing => {
                self.pressure.is_none() && self.age_ms.is_none() && self.sequence.is_none()
            }
            _ => self.pressure.is_none(),
        }
    }
}

//! Daemon-owned sampling policy. Native observations are supplied by arc-platform.
use arc_core::meter::{SamplingCadence, SamplingMode};

mod cpu;
mod snapshot;
mod worker;
pub use arc_platform::{NativeSource, SystemSource};
pub use snapshot::Sampler;
pub use worker::{Activity, MeterWorker, SnapshotReader};

#[cfg(test)]
mod fixtures;

pub const LATEST_SNAPSHOT_CAPACITY: usize = 1;
pub const NATIVE_WORK_CAPACITY: usize = 1;
pub const PREVIOUS_CPU_SAMPLE_CAPACITY: usize = 1;
/// This current-value slice persists no resource samples or resource history.
pub const PERSISTED_SAMPLE_CAPACITY: usize = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SamplingPolicy {
    idle: SamplingCadence,
    active: SamplingCadence,
    pressure: SamplingCadence,
}
impl SamplingPolicy {
    pub fn new(idle_ms: u64, active_ms: u64, pressure_ms: u64) -> Result<Self, &'static str> {
        Ok(Self {
            idle: SamplingCadence::new(SamplingMode::Idle, idle_ms)?,
            active: SamplingCadence::new(SamplingMode::Active, active_ms)?,
            pressure: SamplingCadence::new(SamplingMode::Pressure, pressure_ms)?,
        })
    }
    pub fn cadence(&self, active_jobs: bool, elevated_pressure: bool) -> SamplingCadence {
        if elevated_pressure {
            self.pressure
        } else if active_jobs {
            self.active
        } else {
            self.idle
        }
    }
}
impl Default for SamplingPolicy {
    fn default() -> Self {
        Self::new(15_000, 2000, 1000).expect("fixed valid meter policy")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_policy_adapts_without_a_gui_input() {
        let policy = SamplingPolicy::default();
        assert_eq!(policy.cadence(false, false).interval_ms, 15_000);
        assert_eq!(policy.cadence(true, false).mode, SamplingMode::Active);
        assert_eq!(policy.cadence(true, false).interval_ms, 2000);
        assert_eq!(policy.cadence(false, true).mode, SamplingMode::Pressure);
        assert_eq!(policy.cadence(true, true).interval_ms, 1000);
        for (idle, active, pressure) in [
            (9999, 1000, 1000),
            (30_001, 1000, 1000),
            (10_000, 999, 1000),
            (10_000, 3001, 1000),
            (10_000, 1000, 999),
            (10_000, 1000, 3001),
        ] {
            assert!(SamplingPolicy::new(idle, active, pressure).is_err());
        }
        for idle in [10_000, 30_000] {
            for active in [1000, 3000] {
                let policy = SamplingPolicy::new(idle, active, active).unwrap();
                assert_eq!(policy.cadence(false, false).stale_after_ms, idle * 2 + 1000);
            }
        }
        assert_eq!(LATEST_SNAPSHOT_CAPACITY, 1);
        assert_eq!(NATIVE_WORK_CAPACITY, 1);
        assert_eq!(PREVIOUS_CPU_SAMPLE_CAPACITY, 1);
        assert_eq!(PERSISTED_SAMPLE_CAPACITY, 0);
    }
}

//! Synthetic observations, explicitly separate from SystemSource evidence.
#![allow(dead_code)]
use arc_core::{meter::*, process_identity::ProcessIdentity};
use arc_policy::*;
use std::sync::Mutex;
pub fn identity() -> ProcessIdentity {
    ProcessIdentity {
        pid: 123,
        start_token: "456".into(),
        boot_id: "00000000-0000-0000-0000-000000000001".into(),
    }
}
fn field<T>(
    unit: Unit,
    provenance: Provenance,
    reading: Reading<T>,
    monotonic: u64,
) -> Measurement<T> {
    Measurement {
        unit,
        provenance,
        reading,
        observed_at_ms: None,
        observed_monotonic_ms: monotonic,
    }
}
pub fn snapshot(pressure: Reading<MemoryPressure>) -> MeterSnapshot {
    let native_bytes = || field(Unit::Bytes, Provenance::Native, Reading::Unsupported {}, 0);
    MeterSnapshot {
        schema_version: METER_SCHEMA_VERSION,
        daemon_instance: identity(),
        sequence: 1,
        sampled_at_ms: None,
        sampled_monotonic_ms: 1000,
        age_ms: 0,
        cadence: SamplingCadence::new(SamplingMode::Active, 1000).unwrap(),
        host: HostMeasurements {
            cpu_utilization: field(
                Unit::Percent,
                Provenance::Derived,
                Reading::FirstInterval {},
                0,
            ),
            cpu_interval: None,
            memory_available_bytes: native_bytes(),
            memory_pressure: field(Unit::Level, Provenance::Native, pressure, 1000),
            swap_used_bytes: native_bytes(),
            filesystem: FilesystemMeasurements {
                identity: None,
                capacity_bytes: native_bytes(),
                free_bytes: native_bytes(),
                available_bytes: native_bytes(),
                pressure: field(Unit::Level, Provenance::Derived, Reading::Unsupported {}, 0),
            },
        },
        daemon: DaemonMeasurements {
            cpu_accounting: DaemonCpuAccounting::ProcessLifetime,
            peak_observation_started_at_ms: None,
            peak_observation_started_monotonic_ms: 0,
            cpu_core_seconds: field(
                Unit::CoreSeconds,
                Provenance::Native,
                Reading::Unsupported {},
                0,
            ),
            rss_bytes: native_bytes(),
            observed_peak_rss_bytes: field(
                Unit::Bytes,
                Provenance::Derived,
                Reading::Unsupported {},
                0,
            ),
        },
    }
}
#[derive(Clone)]
pub struct State {
    pub active: SamplerIdentity,
    pub published: SamplerIdentity,
    pub sample: Option<MeterSnapshot>,
    pub now: Option<u64>,
    pub status: SamplerStatus,
    pub supported: bool,
    pub cursor: Option<ObservationCursor>,
}
impl State {
    pub fn new(pressure: MemoryPressure) -> Self {
        let active = SamplerIdentity {
            daemon_instance: identity(),
            generation: 1,
            monotonic_epoch: 1,
        };
        Self {
            published: active.clone(),
            active,
            sample: Some(snapshot(Reading::Available { value: pressure })),
            now: Some(1000),
            status: SamplerStatus::Running,
            supported: true,
            cursor: None,
        }
    }
    pub fn input(&self) -> ObservationInput<'_> {
        ObservationInput {
            supported: self.supported,
            sampler_status: self.status,
            active_sampler: Some(&self.active),
            published_sampler: Some(&self.published),
            snapshot: self.sample.as_ref(),
            now_monotonic_ms: self.now,
            high_water: self.cursor.as_ref(),
        }
    }
    pub fn time(&mut self, now: u64) {
        self.now = Some(now);
        if let Some(s) = self.sample.as_mut() {
            s.age_ms = now.saturating_sub(s.sampled_monotonic_ms);
        }
    }
    pub fn policy(&self) -> EffectivePolicy {
        arc_policy::evaluator::evaluate(self.input())
    }
}
pub struct Provider(pub Mutex<State>);
impl Provider {
    pub fn new(pressure: MemoryPressure) -> Self {
        Self(Mutex::new(State::new(pressure)))
    }
    pub fn set(&self, pressure: MemoryPressure) {
        let mut s = self.0.lock().unwrap();
        let sample = s.sample.as_mut().unwrap();
        sample.sequence += 1;
        sample.host.memory_pressure.reading = Reading::Available { value: pressure };
    }
    pub fn policy(&self) -> EffectivePolicy {
        self.0.lock().unwrap().policy()
    }
}
impl ObservationProvider for Provider {
    fn with_observation(&self, consume: &mut dyn FnMut(ObservationInput<'_>)) {
        let s = self.0.lock().unwrap();
        consume(s.input());
    }
}

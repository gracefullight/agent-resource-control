use crate::{
    cpu::{failed, unknown, utilization, PreviousCpu},
    SamplingPolicy,
};
use arc_core::{meter::*, process_identity::ProcessIdentity};
use arc_platform::{NativeFilesystem, NativeSample};

/// One CPU baseline, one peak and one current snapshot. A new instance resets
/// all accounting windows; no job attribution or resource history is retained.
pub struct Sampler {
    identity: ProcessIdentity,
    sequence: u64,
    previous_cpu: Option<PreviousCpu>,
    previous_process_cpu: Option<f64>,
    last_monotonic: Option<u64>,
    window: Option<(Option<u64>, u64)>,
    peak: Option<Measurement<u64>>,
}
impl Sampler {
    pub fn new(identity: ProcessIdentity) -> Result<Self, &'static str> {
        if !identity.is_valid() {
            return Err("invalid daemon identity");
        }
        Ok(Self {
            identity,
            sequence: 0,
            previous_cpu: None,
            previous_process_cpu: None,
            last_monotonic: None,
            window: None,
            peak: None,
        })
    }

    /// Reject a broken monotonic epoch instead of moving cached observations
    /// backwards. Wall-clock changes affect labels only, never arithmetic/age.
    pub fn observe(
        &mut self,
        mut sample: NativeSample,
        active: bool,
        policy: SamplingPolicy,
    ) -> Option<MeterSnapshot> {
        if sample.monotonic_ms > MAX_SAFE_INTEGER
            || self
                .last_monotonic
                .is_some_and(|old| old > sample.monotonic_ms)
            || self.sequence >= MAX_SAFE_INTEGER
        {
            self.previous_cpu = None;
            return None;
        }
        sample.observed_at_ms = sample.observed_at_ms.filter(|v| *v <= MAX_SAFE_INTEGER);
        let wall = sample.observed_at_ms;
        let mono = sample.monotonic_ms;
        let (window_wall, window_mono) = *self.window.get_or_insert((wall, mono));
        let (cpu, cpu_interval) = utilization(&mut self.previous_cpu, &sample.host_cpu, wall, mono);
        let elevated = matches!(
            sample.memory_pressure.value(),
            Some(MemoryPressure::Warning | MemoryPressure::Critical)
        );
        let rss = bounded_bytes(sample.daemon_rss_bytes);
        if let Some(value) = rss.value() {
            let peak = self
                .peak
                .as_ref()
                .and_then(|p| p.reading.value())
                .copied()
                .unwrap_or(0)
                .max(*value);
            self.peak = Some(measurement(
                Reading::Available { value: peak },
                Unit::Bytes,
                Provenance::Derived,
                wall,
                mono,
            ));
        }
        let peak = self.peak.clone().unwrap_or_else(|| {
            measurement(unknown(&rss), Unit::Bytes, Provenance::Derived, wall, mono)
        });
        let process_cpu = match sample.daemon_cpu_core_seconds {
            Reading::Available { value }
                if !value.is_finite() || !(0.0..=MAX_SAFE_INTEGER as f64).contains(&value) =>
            {
                failed("invalid_process_cpu_time")
            }
            Reading::Available { value }
                if self.previous_process_cpu.is_some_and(|old| value < old) =>
            {
                failed("process_cpu_time_regressed")
            }
            Reading::Available { value } => {
                self.previous_process_cpu = Some(value);
                Reading::Available { value }
            }
            Reading::FirstInterval {} => failed("invalid_process_cpu_availability"),
            reading => reading,
        };
        self.sequence += 1;
        self.last_monotonic = Some(mono);
        Some(MeterSnapshot {
            schema_version: METER_SCHEMA_VERSION,
            daemon_instance: self.identity.clone(),
            sequence: self.sequence,
            sampled_at_ms: wall,
            sampled_monotonic_ms: mono,
            age_ms: 0,
            cadence: policy.cadence(active, elevated),
            host: HostMeasurements {
                cpu_utilization: measurement(cpu, Unit::Percent, Provenance::Derived, wall, mono),
                cpu_interval,
                memory_available_bytes: measurement(
                    bounded_bytes(sample.memory_available_bytes),
                    Unit::Bytes,
                    Provenance::Native,
                    wall,
                    mono,
                ),
                memory_pressure: measurement(
                    non_interval(sample.memory_pressure),
                    Unit::Level,
                    Provenance::Native,
                    wall,
                    mono,
                ),
                swap_used_bytes: measurement(
                    bounded_bytes(sample.swap_used_bytes),
                    Unit::Bytes,
                    Provenance::Native,
                    wall,
                    mono,
                ),
                filesystem: filesystem(sample.filesystem, wall, mono),
            },
            daemon: DaemonMeasurements {
                cpu_accounting: DaemonCpuAccounting::ProcessLifetime,
                peak_observation_started_at_ms: window_wall,
                peak_observation_started_monotonic_ms: window_mono,
                cpu_core_seconds: measurement(
                    process_cpu,
                    Unit::CoreSeconds,
                    Provenance::Native,
                    wall,
                    mono,
                ),
                rss_bytes: measurement(rss, Unit::Bytes, Provenance::Native, wall, mono),
                observed_peak_rss_bytes: peak,
            },
        })
    }
}
fn measurement<T>(
    reading: Reading<T>,
    unit: Unit,
    provenance: Provenance,
    wall: Option<u64>,
    mono: u64,
) -> Measurement<T> {
    Measurement {
        unit,
        provenance,
        observed_at_ms: wall,
        observed_monotonic_ms: mono,
        reading,
    }
}
fn non_interval<T>(reading: Reading<T>) -> Reading<T> {
    if matches!(reading, Reading::FirstInterval {}) {
        failed("invalid_native_availability")
    } else {
        reading
    }
}
fn bounded_bytes(reading: Reading<u64>) -> Reading<u64> {
    match reading {
        Reading::Available { value } if value > MAX_SAFE_INTEGER => {
            failed("native_bytes_out_of_range")
        }
        reading => non_interval(reading),
    }
}
fn filesystem(
    mut native: NativeFilesystem,
    wall: Option<u64>,
    mono: u64,
) -> FilesystemMeasurements {
    let identity = native.identity.filter(FilesystemIdentity::is_valid);
    native.capacity_bytes = bounded_bytes(native.capacity_bytes);
    native.free_bytes = bounded_bytes(native.free_bytes);
    native.available_bytes = bounded_bytes(native.available_bytes);
    for reading in [
        &mut native.capacity_bytes,
        &mut native.free_bytes,
        &mut native.available_bytes,
    ] {
        if identity.is_none() && reading.value().is_some() {
            *reading = failed("filesystem_identity_unavailable");
        }
    }
    let (capacity, free, available) = (
        native.capacity_bytes.value().copied(),
        native.free_bytes.value().copied(),
        native.available_bytes.value().copied(),
    );
    if capacity.zip(free).is_some_and(|(c, f)| f > c) {
        native.free_bytes = failed("filesystem_free_exceeds_capacity");
    }
    if free.zip(available).is_some_and(|(f, a)| a > f)
        || capacity.zip(available).is_some_and(|(c, a)| a > c)
    {
        native.available_bytes = failed("filesystem_available_invalid");
    }
    let pressure = match (
        native.capacity_bytes.value(),
        native.available_bytes.value(),
    ) {
        (Some(0), _) => failed("filesystem_capacity_zero"),
        (Some(capacity), Some(available)) => {
            let fraction = *available as f64 / *capacity as f64;
            Reading::Available {
                value: if fraction <= 0.05 {
                    DiskPressure::Critical
                } else if fraction <= 0.10 {
                    DiskPressure::Warning
                } else {
                    DiskPressure::Normal
                },
            }
        }
        (None, _) => unknown(&native.capacity_bytes),
        (_, None) => unknown(&native.available_bytes),
    };
    FilesystemMeasurements {
        identity,
        capacity_bytes: measurement(
            native.capacity_bytes,
            Unit::Bytes,
            Provenance::Native,
            wall,
            mono,
        ),
        free_bytes: measurement(
            native.free_bytes,
            Unit::Bytes,
            Provenance::Native,
            wall,
            mono,
        ),
        available_bytes: measurement(
            native.available_bytes,
            Unit::Bytes,
            Provenance::Native,
            wall,
            mono,
        ),
        pressure: measurement(pressure, Unit::Level, Provenance::Derived, wall, mono),
    }
}

#[cfg(test)]
#[path = "snapshot_tests.rs"]
mod tests;

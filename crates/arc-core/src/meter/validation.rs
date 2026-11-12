use super::*;

fn wall_time(value: Option<u64>) -> bool {
    value.is_none_or(|value| value <= MAX_SAFE_INTEGER)
}
fn text(value: &str, maximum: usize) -> bool {
    !value.is_empty() && value.len() <= maximum && !value.chars().any(char::is_control)
}
fn finite_nonnegative(value: &f64) -> bool {
    value.is_finite() && *value >= 0.0 && *value <= MAX_SAFE_INTEGER as f64
}
fn bytes(value: &u64) -> bool {
    *value <= MAX_SAFE_INTEGER
}
impl SamplingCadence {
    pub fn new(mode: SamplingMode, interval_ms: u64) -> Result<Self, &'static str> {
        let cadence = Self {
            mode,
            interval_ms,
            stale_after_ms: interval_ms.saturating_mul(2).saturating_add(1000),
        };
        cadence
            .is_valid()
            .then_some(cadence)
            .ok_or("invalid meter cadence")
    }
    pub fn is_valid(&self) -> bool {
        let range = match self.mode {
            SamplingMode::Idle => 10_000..=30_000,
            SamplingMode::Active | SamplingMode::Pressure => 1000..=3000,
        };
        range.contains(&self.interval_ms) && self.stale_after_ms == self.interval_ms * 2 + 1000
    }
}
impl FilesystemIdentity {
    pub fn is_valid(&self) -> bool {
        self.is_local
            && self.path.starts_with('/')
            && self.mount_point.starts_with('/')
            && text(&self.path, MAX_PATH_BYTES)
            && text(&self.mount_point, MAX_PATH_BYTES)
            && text(&self.filesystem_type, 64)
            && text(&self.volume_id, 128)
    }
}
impl<T> Measurement<T> {
    fn valid(
        &self,
        unit: Unit,
        provenance: Provenance,
        sampled: u64,
        value: impl FnOnce(&T) -> bool,
    ) -> bool {
        self.unit == unit
            && self.provenance == provenance
            && wall_time(self.observed_at_ms)
            && self.observed_monotonic_ms <= sampled
            && self.reading.value().is_none_or(value)
    }
}

impl<T> Measurement<T> {
    fn is_first_interval(&self) -> bool {
        matches!(self.reading, Reading::FirstInterval {})
    }
}
impl MeterSnapshot {
    /// Run after decoding and before using a peer's snapshot. Failures are local
    /// schema errors; do not present a partially validated wire object.
    pub fn is_valid(&self) -> bool {
        let sample = self.sampled_monotonic_ms;
        let host = &self.host;
        let filesystem = &host.filesystem;
        let daemon = &self.daemon;
        if self.schema_version != METER_SCHEMA_VERSION
            || !self.daemon_instance.is_valid()
            || self.sequence == 0
            || self.sequence > MAX_SAFE_INTEGER
            || !wall_time(self.sampled_at_ms)
            || sample > MAX_SAFE_INTEGER
            || sample
                .checked_add(self.age_ms)
                .is_none_or(|value| value > MAX_SAFE_INTEGER)
            || !self.cadence.is_valid()
            || !wall_time(daemon.peak_observation_started_at_ms)
            || daemon.peak_observation_started_monotonic_ms > sample
        {
            return false;
        }
        let native_bytes = [
            &host.memory_available_bytes,
            &host.swap_used_bytes,
            &filesystem.capacity_bytes,
            &filesystem.free_bytes,
            &filesystem.available_bytes,
            &daemon.rss_bytes,
        ];
        if !native_bytes.iter().all(|field| {
            !field.is_first_interval()
                && field.valid(Unit::Bytes, Provenance::Native, sample, bytes)
        }) || !host
            .cpu_utilization
            .valid(Unit::Percent, Provenance::Derived, sample, |value| {
                value.is_finite() && (0.0..=100.0).contains(value)
            })
            || host.memory_pressure.is_first_interval()
            || !host
                .memory_pressure
                .valid(Unit::Level, Provenance::Native, sample, |_| true)
            || filesystem.pressure.is_first_interval()
            || !filesystem
                .pressure
                .valid(Unit::Level, Provenance::Derived, sample, |_| true)
            || daemon.cpu_core_seconds.is_first_interval()
            || daemon.observed_peak_rss_bytes.is_first_interval()
            || !daemon.observed_peak_rss_bytes.valid(
                Unit::Bytes,
                Provenance::Derived,
                sample,
                bytes,
            )
            || !daemon.cpu_core_seconds.valid(
                Unit::CoreSeconds,
                Provenance::Native,
                sample,
                finite_nonnegative,
            )
        {
            return false;
        }
        if let Some(interval) = &host.cpu_interval {
            if interval.logical_cpu_count == 0
                || !wall_time(interval.started_at_ms)
                || !wall_time(interval.ended_at_ms)
                || interval.ended_monotonic_ms != host.cpu_utilization.observed_monotonic_ms
                || interval.elapsed_ms == 0
                || interval
                    .ended_monotonic_ms
                    .checked_sub(interval.started_monotonic_ms)
                    != Some(interval.elapsed_ms)
            {
                return false;
            }
        }
        if host.cpu_utilization.reading.value().is_some() != host.cpu_interval.is_some() {
            return false;
        }
        let has_filesystem_value = [
            &filesystem.capacity_bytes,
            &filesystem.free_bytes,
            &filesystem.available_bytes,
        ]
        .iter()
        .any(|field| field.reading.value().is_some())
            || filesystem.pressure.reading.value().is_some();
        if filesystem
            .identity
            .as_ref()
            .is_some_and(|identity| !identity.is_valid())
            || (has_filesystem_value && filesystem.identity.is_none())
        {
            return false;
        }
        if let (Some(capacity), Some(free)) = (
            filesystem.capacity_bytes.reading.value(),
            filesystem.free_bytes.reading.value(),
        ) {
            if free > capacity {
                return false;
            }
        }
        if let (Some(free), Some(available)) = (
            filesystem.free_bytes.reading.value(),
            filesystem.available_bytes.reading.value(),
        ) {
            if available > free {
                return false;
            }
        }
        if let (Some(capacity), Some(available)) = (
            filesystem.capacity_bytes.reading.value(),
            filesystem.available_bytes.reading.value(),
        ) {
            if available > capacity {
                return false;
            }
        }
        if let Some(pressure) = filesystem.pressure.reading.value() {
            let (Some(capacity), Some(available)) = (
                filesystem.capacity_bytes.reading.value(),
                filesystem.available_bytes.reading.value(),
            ) else {
                return false;
            };
            if *capacity == 0 {
                return false;
            }
            let ratio = *available as f64 / *capacity as f64;
            let expected = if ratio <= 0.05 {
                DiskPressure::Critical
            } else if ratio <= 0.10 {
                DiskPressure::Warning
            } else {
                DiskPressure::Normal
            };
            if *pressure != expected {
                return false;
            }
        }
        if daemon.observed_peak_rss_bytes.observed_monotonic_ms
            < daemon.peak_observation_started_monotonic_ms
        {
            return false;
        }
        if let (Some(rss), Some(peak)) = (
            daemon.rss_bytes.reading.value(),
            daemon.observed_peak_rss_bytes.reading.value(),
        ) {
            if rss > peak {
                return false;
            }
        }
        true
    }
}

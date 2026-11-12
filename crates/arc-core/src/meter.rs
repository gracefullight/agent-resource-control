//! Bounded current host measurements, separate from job attribution and outcomes.
use crate::process_identity::ProcessIdentity;
use serde::{Deserialize, Serialize};

#[cfg(test)]
mod tests;
mod validation;

pub const METER_SCHEMA_VERSION: u32 = 1;
pub const MAX_SNAPSHOT_BYTES: usize = 16_384;
pub const MAX_DIAGNOSTIC_BYTES: usize = 128;
pub const MAX_PATH_BYTES: usize = 1024;
/// Wire numbers must round-trip exactly through the desktop's JavaScript bridge.
pub const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Diagnostic(String);

impl Diagnostic {
    /// Bound native diagnostics before retaining or serializing them.
    pub fn bounded(message: &str) -> Self {
        let mut value = String::new();
        for character in message.chars() {
            let character = if character.is_control() {
                ' '
            } else {
                character
            };
            if value.len() + character.len_utf8() > MAX_DIAGNOSTIC_BYTES {
                break;
            }
            value.push(character);
        }
        if value.is_empty() {
            value.push_str("measurement_failed");
        }
        Self(value)
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Diagnostic {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty()
            || value.len() > MAX_DIAGNOSTIC_BYTES
            || value.chars().any(char::is_control)
        {
            Err("invalid meter diagnostic")
        } else {
            Ok(Self(value))
        }
    }
}
impl From<Diagnostic> for String {
    fn from(value: Diagnostic) -> Self {
        value.0
    }
}

/// Unknown observations cannot accidentally acquire a numeric zero.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "availability", rename_all = "snake_case", deny_unknown_fields)]
pub enum Reading<T> {
    Available { value: T },
    FirstInterval {},
    Unsupported {},
    Denied {},
    Failed { diagnostic: Diagnostic },
}
impl<T> Reading<T> {
    pub fn value(&self) -> Option<&T> {
        match self {
            Self::Available { value } => Some(value),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    Percent,
    Bytes,
    CoreSeconds,
    Level,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
    Native,
    Derived,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Measurement<T> {
    pub unit: Unit,
    pub provenance: Provenance,
    /// UTC presentation time; null means the wall clock could not be read.
    pub observed_at_ms: Option<u64>,
    /// Milliseconds since this sampler instance started, never wall-clock age.
    pub observed_monotonic_ms: u64,
    pub reading: Reading<T>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryPressure {
    Normal,
    Warning,
    Critical,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiskPressure {
    Normal,
    Warning,
    Critical,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SamplingMode {
    Idle,
    Active,
    Pressure,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SamplingCadence {
    pub mode: SamplingMode,
    pub interval_ms: u64,
    pub stale_after_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CpuNormalization {
    HostCapacity,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CpuInterval {
    pub started_at_ms: Option<u64>,
    pub ended_at_ms: Option<u64>,
    pub started_monotonic_ms: u64,
    pub ended_monotonic_ms: u64,
    pub elapsed_ms: u64,
    pub logical_cpu_count: u32,
    pub normalization: CpuNormalization,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilesystemIdentity {
    pub path: String,
    pub mount_point: String,
    pub filesystem_type: String,
    pub volume_id: String,
    pub is_local: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FilesystemMeasurements {
    pub identity: Option<FilesystemIdentity>,
    pub capacity_bytes: Measurement<u64>,
    /// All unallocated blocks reported by the filesystem, including reservations.
    pub free_bytes: Measurement<u64>,
    /// Unallocated blocks available to this unprivileged caller.
    pub available_bytes: Measurement<u64>,
    /// A separately labeled judgment, never a native pressure observation.
    pub pressure: Measurement<DiskPressure>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostMeasurements {
    pub cpu_utilization: Measurement<f64>,
    pub cpu_interval: Option<CpuInterval>,
    pub memory_available_bytes: Measurement<u64>,
    pub memory_pressure: Measurement<MemoryPressure>,
    pub swap_used_bytes: Measurement<u64>,
    pub filesystem: FilesystemMeasurements,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DaemonCpuAccounting {
    ProcessLifetime,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DaemonMeasurements {
    pub cpu_accounting: DaemonCpuAccounting,
    /// Peak is derived from successful RSS observations since this window began.
    pub peak_observation_started_at_ms: Option<u64>,
    pub peak_observation_started_monotonic_ms: u64,
    pub cpu_core_seconds: Measurement<f64>,
    pub rss_bytes: Measurement<u64>,
    /// Its observation time is the most recent successful RSS pass confirming
    /// this running maximum, not necessarily the time at which the maximum hit.
    pub observed_peak_rss_bytes: Measurement<u64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeterSnapshot {
    pub schema_version: u32,
    /// Existing boot ID + PID + start token; restart invalidates old baselines.
    pub daemon_instance: ProcessIdentity,
    pub sequence: u64,
    pub sampled_at_ms: Option<u64>,
    pub sampled_monotonic_ms: u64,
    /// Recomputed at read time from a monotonic clock, without a new observation.
    pub age_ms: u64,
    pub cadence: SamplingCadence,
    pub host: HostMeasurements,
    pub daemon: DaemonMeasurements,
}

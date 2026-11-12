//! Per-job terminal CPU accounting; independent of host utilization snapshots.
use crate::process_identity::ProcessIdentity;
use serde::{Deserialize, Serialize};

mod validation;
pub use validation::valid_id;
#[cfg(test)]
mod tests;

pub const JOB_METER_SCHEMA_VERSION: u32 = 1;
pub const MAX_JOB_METER_BYTES: usize = 16_384;
pub const JOB_METADATA_RETENTION_MS: u64 = 90 * 24 * 60 * 60 * 1000;
pub const MAX_MISSING_COVERAGE_REASONS: usize = 8;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CpuCoreSeconds {
    #[serde(deserialize_with = "required_option")]
    pub user: Option<f64>,
    #[serde(deserialize_with = "required_option")]
    pub system: Option<f64>,
    #[serde(deserialize_with = "required_option")]
    pub total: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunClock {
    Monotonic,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunElapsed {
    #[serde(deserialize_with = "required_option")]
    pub seconds: Option<f64>,
    pub clock: RunClock,
    /// Spawn attempt through terminal observation, including stopped time;
    /// excludes queue wait and post-termination metadata/reporting.
    pub includes_stopped: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueClock {
    DaemonWallClock,
    NotApplicable,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QueueWaitBasis {
    QueuedToReserved,
    LifecycleOnly,
    Unavailable,
}

/// Derived by the single SQLite writer from durable scheduling facts, never
/// from the reporting client's run timer or mixed daemon monotonic epochs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QueueWait {
    #[serde(deserialize_with = "required_option")]
    pub seconds: Option<f64>,
    pub clock: QueueClock,
    pub basis: QueueWaitBasis,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttributionBasis {
    NativeOwnedChildWait4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountingScope {
    NativeWaitedDescendants,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Completeness {
    Complete,
    Partial,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MissingCoverageReason {
    UnobservedDescendants,
    DetachedDescendants,
    NativeUsageUnavailable,
    RunDurationUnavailable,
    LineageUnverified,
    Interrupted,
    Unsupported,
}

/// Inclusive observations are never summed across these relationships. Only a
/// verified canonical root is eligible for a future nonoverlapping aggregate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AccountingRelation {
    CanonicalRoot { root_job_id: String },
    IncludedInRoot { root_job_id: String },
    Unverified {},
}
impl AccountingRelation {
    pub fn is_additive(&self) -> bool {
        matches!(self, Self::CanonicalRoot { .. })
    }
}

/// A terminal outcome only. Spawn failure has no owned native child usage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum TerminalOutcome {
    Exited { code: u8 },
    Signaled { signal: u8, core_dumped: bool },
}

/// Immutable report, retryable with this exact ID and payload after lost ACKs.
/// Durable job/observer/child/outcome/lineage binding is a storage responsibility;
/// syntactic validation alone does not establish ownership or peer identity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobMeterReport {
    pub schema_version: u32,
    pub report_id: String,
    pub job_id: String,
    pub observer: ProcessIdentity,
    pub child: ProcessIdentity,
    pub cpu_core_seconds: CpuCoreSeconds,
    pub run_elapsed: RunElapsed,
    pub attribution_basis: AttributionBasis,
    pub scope: AccountingScope,
    pub completeness: Completeness,
    pub missing_coverage: Vec<MissingCoverageReason>,
    pub accounting_relation: AccountingRelation,
    pub outcome: TerminalOutcome,
    /// Presentation only; receipt time and this client clock cannot extend TTL.
    #[serde(deserialize_with = "required_option")]
    pub terminated_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnknownReason {
    Legacy,
    Interrupted,
    Unavailable,
    NotTerminated,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum SummaryAccounting {
    Recorded {
        report: Box<JobMeterReport>,
    },
    Unknown {
        reason: UnknownReason,
    },
    /// A missing record proves neither expiry nor nonexecution.
    Missing {},
    /// Emit only with durable evidence that metadata expired under retention.
    Expired {},
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobMeterSummary {
    pub schema_version: u32,
    pub job_id: String,
    pub accounting: SummaryAccounting,
    pub queue_wait: QueueWait,
}

fn required_option<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

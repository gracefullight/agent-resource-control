//! Job projection omits command data; Meter reuses the bounded core schema.
pub use arc_core::job_meter::JobMeterSummary;
pub use arc_core::meter::MeterSnapshot;
use arc_ipc::jobs::{
    IdentityObservation, JobId, JobSnapshot, JobSummary, LifecycleReason, Outcome, Phase,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Received<T> {
    pub data: T,
    pub received_at_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DaemonRunning {
    Running,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SweeperOff {
    Off,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DaemonHealth {
    pub daemon: DaemonRunning,
    pub protocol_version: u32,
    pub sweeper: SweeperOff,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RecentJobs {
    pub jobs: Vec<JobSummary>,
    pub truncated: bool,
    pub observed_at_ms: u64,
    pub liveness_fresh: bool,
    pub limit: u8,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct JobDetail {
    pub job_id: JobId,
    pub phase: Phase,
    pub reason: LifecycleReason,
    pub observation: IdentityObservation,
    pub outcome: Option<Outcome>,
    pub registered_at_ms: u64,
    pub launch_claimed_at_ms: Option<u64>,
    pub completed_at_ms: Option<u64>,
    pub updated_at_ms: u64,
    pub revision: u64,
}

// Only call after full wire snapshot validation, before dropping private fields.
impl From<JobSnapshot> for JobDetail {
    fn from(job: JobSnapshot) -> Self {
        Self {
            job_id: job.job_id,
            phase: job.phase,
            reason: job.reason,
            observation: job.observation,
            outcome: job.outcome,
            registered_at_ms: job.registered_at_ms,
            launch_claimed_at_ms: job.launch_claimed_at_ms,
            completed_at_ms: job.completed_at_ms,
            updated_at_ms: job.updated_at_ms,
            revision: job.revision,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum JobLookup {
    Found { job: JobDetail },
    NotFound {},
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct DesktopMetadata {
    pub product_name: &'static str,
    pub version: &'static str,
    pub build_commit: Option<&'static str>,
    pub build_dirty: Option<bool>,
    pub os: &'static str,
    pub architecture: &'static str,
    pub license: Option<&'static str>,
    pub open_source_notices: &'static str,
}

pub fn desktop_metadata() -> DesktopMetadata {
    let commit = option_env!("ARC_BUILD_COMMIT").filter(|value| {
        matches!(value.len(), 40 | 64)
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    });
    DesktopMetadata {
        product_name: "Agent Resource Control",
        version: env!("CARGO_PKG_VERSION"),
        build_commit: commit,
        build_dirty: match option_env!("ARC_BUILD_DIRTY") {
            Some("true") => Some(true),
            Some("false") => Some(false),
            _ => None,
        },
        os: std::env::consts::OS,
        architecture: std::env::consts::ARCH,
        license: option_env!("CARGO_PKG_LICENSE").filter(|value| !value.is_empty()),
        open_source_notices: include_str!("../assets/open-source-notices.txt"),
    }
}

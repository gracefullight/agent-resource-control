use super::{Classification, ProcessIdentity};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct JobId(String);
impl JobId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl TryFrom<String> for JobId {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.len() == 32
            && value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            Ok(Self(value))
        } else {
            Err("invalid job id")
        }
    }
}
impl From<JobId> for String {
    fn from(id: JobId) -> Self {
        id.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Phase {
    Registered,
    Launching,
    Running,
    Completed,
    Signaled,
    SpawnFailed,
    OutcomeUnknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LifecycleReason {
    None,
    RegisteredNotClaimed,
    ClaimedNotAttached,
    ObserverMissingNotClaimed,
    ObserverMissing,
    ProcessMissing,
    IdentityMismatch,
    IdentityUnavailable,
    ChildVerified,
    ObservedExit,
    ObservedSignal,
    ObservedSpawnFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ObservationTarget {
    Observer,
    Child,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ObservationStatus {
    VerifiedMatch,
    Missing,
    Mismatch,
    Indeterminate,
    NotAttached,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SpawnReason {
    NotFound,
    PermissionDenied,
    InvalidInput,
    ResourceUnavailable,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StartDisposition {
    Created,
    Applied,
    Unchanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CompleteDisposition {
    Applied,
    Unchanged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum Outcome {
    Exited {
        code: u8,
    },
    Signaled {
        signal: u8,
        core_dumped: bool,
    },
    SpawnFailed {
        reason: SpawnReason,
        #[serde(deserialize_with = "super::required_option")]
        os_code: Option<i32>,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IdentityObservation {
    pub target: ObservationTarget,
    pub status: ObservationStatus,
    pub observed_at_ms: u64,
    pub reason: LifecycleReason,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobSnapshot {
    pub job_id: JobId,
    pub phase: Phase,
    #[serde(deserialize_with = "super::object")]
    pub classification: Classification,
    pub observer: ProcessIdentity,
    pub observer_verified_at_ms: u64,
    #[serde(deserialize_with = "super::required_option")]
    pub child: Option<ProcessIdentity>,
    #[serde(deserialize_with = "super::required_option")]
    pub child_verified_at_ms: Option<u64>,
    #[serde(deserialize_with = "super::object")]
    pub observation: IdentityObservation,
    pub reason: LifecycleReason,
    #[serde(deserialize_with = "super::required_option")]
    pub outcome: Option<Outcome>,
    pub registered_at_ms: u64,
    #[serde(deserialize_with = "super::required_option")]
    pub launch_claimed_at_ms: Option<u64>,
    #[serde(deserialize_with = "super::required_option")]
    pub completed_at_ms: Option<u64>,
    pub updated_at_ms: u64,
    pub revision: u64,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobSummary {
    pub job_id: JobId,
    pub phase: Phase,
    pub registered_at_ms: u64,
    pub updated_at_ms: u64,
    pub reason: LifecycleReason,
}

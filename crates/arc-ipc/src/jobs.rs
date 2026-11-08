//! Protocol-1 lifecycle payloads. No execution arguments cross this boundary.
mod types;
mod validation;
use crate::Classification;
pub use arc_core::process_identity::ProcessIdentity;
use serde::{Deserialize, Serialize};
pub use types::{
    CompleteDisposition, IdentityObservation, JobId, JobSnapshot, JobSummary, LifecycleReason,
    ObservationStatus, ObservationTarget, Outcome, Phase, SpawnReason, StartDisposition,
};

pub const OPERATION_JOB_START: &str = "job.start";
pub const OPERATION_JOB_COMPLETE: &str = "job.complete";
pub const OPERATION_JOB_STATUS: &str = "job.status";
pub const ERROR_JOB_CONFLICT: &str = "JOB_CONFLICT";
pub const ERROR_INVALID_JOB_TRANSITION: &str = "INVALID_JOB_TRANSITION";
pub const ERROR_PROCESS_IDENTITY_UNAVAILABLE: &str = "PROCESS_IDENTITY_UNAVAILABLE";
pub const MAX_LIFECYCLE_PAYLOAD_BYTES: usize = 8192;
pub const MAX_TIMESTAMP: u64 = 9_007_199_254_740_991;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "action",
    rename_all = "SCREAMING_SNAKE_CASE",
    deny_unknown_fields
)]
pub enum StartRequest {
    Register {
        job_id: JobId,
        observer: ProcessIdentity,
        #[serde(deserialize_with = "object")]
        classification: Classification,
    },
    Claim {
        job_id: JobId,
        observer: ProcessIdentity,
    },
    Attach {
        job_id: JobId,
        observer: ProcessIdentity,
        child: ProcessIdentity,
    },
}
impl StartRequest {
    pub fn job_id(&self) -> &JobId {
        match self {
            Self::Register { job_id, .. }
            | Self::Claim { job_id, .. }
            | Self::Attach { job_id, .. } => job_id,
        }
    }
    pub fn observer(&self) -> &ProcessIdentity {
        match self {
            Self::Register { observer, .. }
            | Self::Claim { observer, .. }
            | Self::Attach { observer, .. } => observer,
        }
    }
    pub fn is_valid(&self) -> bool {
        self.observer().is_valid()
            && match self {
                Self::Register { classification, .. } => classification.is_consistent(),
                Self::Attach { child, .. } => child.is_valid(),
                Self::Claim { .. } => true,
            }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StartResult {
    pub disposition: StartDisposition,
    #[serde(deserialize_with = "object")]
    pub job: JobSnapshot,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompleteRequest {
    pub job_id: JobId,
    pub observer: ProcessIdentity,
    pub outcome: Outcome,
}
impl CompleteRequest {
    pub fn is_valid(&self) -> bool {
        self.observer.is_valid() && self.outcome.is_valid()
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CompleteResult {
    pub disposition: CompleteDisposition,
    #[serde(deserialize_with = "object")]
    pub job: JobSnapshot,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "query",
    rename_all = "SCREAMING_SNAKE_CASE",
    deny_unknown_fields
)]
pub enum StatusRequest {
    ById { job_id: JobId },
    Recent {},
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum StatusResult {
    Found {
        #[serde(deserialize_with = "object")]
        job: Box<JobSnapshot>,
    },
    NotFound {},
    Recent {
        jobs: Vec<JobSummary>,
        truncated: bool,
        observed_at_ms: u64,
        liveness_fresh: bool,
    },
}
impl StatusResult {
    pub fn is_valid(&self) -> bool {
        match self {
            Self::Found { job } => job.is_valid(),
            Self::NotFound {} => true,
            Self::Recent {
                jobs,
                observed_at_ms,
                liveness_fresh,
                ..
            } => {
                !liveness_fresh
                    && *observed_at_ms <= MAX_TIMESTAMP
                    && jobs.len() <= 50
                    && jobs.iter().all(JobSummary::is_valid)
            }
        }
    }
}

fn required_option<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}
/// Bound and validate the complete request before any worker admission. Decode
/// failures are static; serde diagnostics must not be exposed to users or logs.
pub fn decode_start(payload: &serde_json::Value) -> Result<StartRequest, &'static str> {
    let request: StartRequest = decode_payload(payload)?;
    request
        .is_valid()
        .then_some(request)
        .ok_or(crate::ERROR_INVALID_REQUEST)
}
pub fn decode_complete(payload: &serde_json::Value) -> Result<CompleteRequest, &'static str> {
    let request: CompleteRequest = decode_payload(payload)?;
    request
        .is_valid()
        .then_some(request)
        .ok_or(crate::ERROR_INVALID_REQUEST)
}
pub fn decode_status(payload: &serde_json::Value) -> Result<StatusRequest, &'static str> {
    decode_payload(payload)
}
fn decode_payload<T: serde::de::DeserializeOwned>(
    payload: &serde_json::Value,
) -> Result<T, &'static str> {
    // The transport already caps a frame. Check compact lifecycle size before
    // allocating owned strings for the validated type.
    if !payload.is_object()
        || serde_json::to_vec(payload)
            .map_or(true, |bytes| bytes.len() > MAX_LIFECYCLE_PAYLOAD_BYTES)
    {
        return Err(crate::ERROR_INVALID_REQUEST);
    }
    T::deserialize(payload).map_err(|_| crate::ERROR_INVALID_REQUEST)
}

pub(super) fn object<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    struct Object<T>(std::marker::PhantomData<T>);
    impl<'de, T: serde::Deserialize<'de>> serde::de::Visitor<'de> for Object<T> {
        type Value = T;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("an object")
        }
        fn visit_map<A: serde::de::MapAccess<'de>>(self, map: A) -> Result<T, A::Error> {
            T::deserialize(serde::de::value::MapAccessDeserializer::new(map))
        }
    }
    deserializer.deserialize_map(Object(std::marker::PhantomData))
}

/// Validated payload routed through the daemon's finite database worker.
#[derive(Debug)]
pub enum LifecycleRequest {
    Start(StartRequest),
    Complete(CompleteRequest),
    Status(StatusRequest),
}
impl LifecycleRequest {
    pub fn decode(
        operation: &str,
        payload: &serde_json::Value,
    ) -> Option<Result<Self, &'static str>> {
        Some(match operation {
            OPERATION_JOB_START => decode_start(payload).map(Self::Start),
            OPERATION_JOB_COMPLETE => decode_complete(payload).map(Self::Complete),
            OPERATION_JOB_STATUS => decode_status(payload).map(Self::Status),
            _ => return None,
        })
    }
}

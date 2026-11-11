//! Additive protocol-1 Traffic contracts. No command, path or environment crosses here.
//!
//! These types do not enable a daemon handler. A daemon must advertise the
//! capability only after implementing the complete Traffic operations.
use crate::{
    jobs::{JobId, Outcome, ProcessIdentity, MAX_TIMESTAMP},
    Request, Response, ERROR_INVALID_REQUEST, ERROR_UNSUPPORTED_CAPABILITY, PROTOCOL_VERSION,
    STATUS_ERROR, STATUS_OK,
};
use arc_traffic::MAX_RESOURCE_VALUE;
pub use arc_traffic::{BypassReason, Resources, WaitReason};
use serde::{Deserialize, Serialize};
use std::fmt;

pub const OPERATION_TRAFFIC_CAPABILITIES: &str = "traffic.capabilities";
pub const OPERATION_TRAFFIC_JOB: &str = "traffic.job";
pub const OPERATION_BUN_CAPABILITIES: &str = "traffic.bun.capabilities";
pub const OPERATION_BUN_JOB: &str = "traffic.bun.job";
pub const CAPABILITY: &str = "traffic.v1";
pub const BUN_CAPABILITY: &str = "traffic.bun.v1";
pub const MAX_TRAFFIC_PAYLOAD_BYTES: usize = 8192;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capabilities {
    pub capability: String,
}

impl Capabilities {
    pub fn supported() -> Self {
        Self {
            capability: CAPABILITY.to_owned(),
        }
    }

    pub fn bun_supported() -> Self {
        Self {
            capability: BUN_CAPABILITY.to_owned(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeResult {
    Supported,
    Unsupported,
    Invalid,
}

/// Probe requests have a null payload, as produced by `Request::new`.
pub fn decode_probe(payload: &serde_json::Value) -> Result<(), &'static str> {
    if payload.is_null() {
        Ok(())
    } else {
        Err(ERROR_INVALID_REQUEST)
    }
}

/// Validate the matching response on an already authenticated connection.
/// Only the explicit unsupported response permits a legacy-client fallback.
pub fn interpret_probe(request: &Request, response: &Response) -> ProbeResult {
    if request.protocol_version != PROTOCOL_VERSION
        || request.operation != OPERATION_TRAFFIC_CAPABILITIES
        || decode_probe(&request.payload).is_err()
        || response.protocol_version != PROTOCOL_VERSION
        || request.request_id != response.request_id
    {
        return ProbeResult::Invalid;
    }
    if response.status == STATUS_ERROR && response.result.is_none() {
        return if response
            .error
            .as_ref()
            .is_some_and(|e| e.code == ERROR_UNSUPPORTED_CAPABILITY)
        {
            ProbeResult::Unsupported
        } else {
            ProbeResult::Invalid
        };
    }
    if response.status == STATUS_OK && response.error.is_none() {
        let valid = response
            .result
            .as_ref()
            .filter(|v| v.is_object())
            .and_then(|value| Capabilities::deserialize(value).ok())
            .is_some_and(|result| result.capability == CAPABILITY);
        if valid {
            return ProbeResult::Supported;
        }
    }
    ProbeResult::Invalid
}

/// A 256-bit opaque capability, transmitted only to the owner and descendants.
/// Syntax validation is not authentication or proof of OS ancestry.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct LineageContext(String);

impl LineageContext {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for LineageContext {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("LineageContext([REDACTED])")
    }
}
impl TryFrom<String> for LineageContext {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.len() == 64
            && value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            Ok(Self(value))
        } else {
            Err("invalid lineage context")
        }
    }
}
impl From<LineageContext> for String {
    fn from(value: LineageContext) -> Self {
        value.0
    }
}

/// All operations are bound to the same job ID. The observer field is a claim;
/// the daemon must independently verify the authenticated peer and identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "action",
    rename_all = "SCREAMING_SNAKE_CASE",
    deny_unknown_fields
)]
pub enum JobRequest {
    Enqueue {
        job_id: JobId,
        observer: ProcessIdentity,
        #[serde(deserialize_with = "crate::jobs::object")]
        classification: crate::Classification,
        #[serde(default)]
        lineage_context: Option<LineageContext>,
    },
    Poll {
        job_id: JobId,
        observer: ProcessIdentity,
    },
    Attach {
        job_id: JobId,
        observer: ProcessIdentity,
        child: ProcessIdentity,
        permit_generation: u64,
    },
    Complete {
        job_id: JobId,
        observer: ProcessIdentity,
        outcome: Outcome,
        permit_generation: u64,
    },
    Cancel {
        job_id: JobId,
        observer: ProcessIdentity,
        #[serde(default)]
        permit_generation: Option<u64>,
    },
    Fallback {
        job_id: JobId,
        observer: ProcessIdentity,
        #[serde(default)]
        permit_generation: Option<u64>,
        #[serde(default)]
        child: Option<ProcessIdentity>,
        #[serde(default)]
        outcome: Option<Outcome>,
    },
    Status {
        job_id: JobId,
    },
    /// Opt in to status states introduced after the original Traffic contract.
    StatusV2 {
        job_id: JobId,
    },
}

impl JobRequest {
    pub fn is_valid(&self) -> bool {
        match self {
            Self::Enqueue {
                observer,
                classification,
                ..
            } => observer.is_valid() && classification.is_consistent(),
            Self::Poll { observer, .. } => observer.is_valid(),
            Self::Cancel {
                observer,
                permit_generation,
                ..
            } => observer.is_valid() && permit_generation.is_none_or(valid_generation),
            Self::Attach {
                observer,
                child,
                permit_generation,
                ..
            } => observer.is_valid() && child.is_valid() && valid_generation(*permit_generation),
            Self::Complete {
                observer,
                outcome,
                permit_generation,
                ..
            } => observer.is_valid() && outcome.is_valid() && valid_generation(*permit_generation),
            Self::Fallback {
                observer,
                child,
                outcome,
                permit_generation,
                ..
            } => {
                observer.is_valid()
                    && child.as_ref().is_none_or(ProcessIdentity::is_valid)
                    && outcome.as_ref().is_none_or(Outcome::is_valid)
                    && permit_generation.is_none_or(valid_generation)
                    && !(child.is_some() && matches!(outcome, Some(Outcome::SpawnFailed { .. })))
            }
            Self::Status { .. } | Self::StatusV2 { .. } => true,
        }
    }
}

/// Validate size, object shape, all fields and the detector's classification.
pub fn decode_job(payload: &serde_json::Value) -> Result<JobRequest, &'static str> {
    if !payload.is_object()
        || serde_json::to_vec(payload).map_or(true, |bytes| bytes.len() > MAX_TRAFFIC_PAYLOAD_BYTES)
    {
        return Err(ERROR_INVALID_REQUEST);
    }
    let request = JobRequest::deserialize(payload).map_err(|_| ERROR_INVALID_REQUEST)?;
    request
        .is_valid()
        .then_some(request)
        .ok_or(ERROR_INVALID_REQUEST)
}

/// Bind Bun admission to its additive operation; legacy status remains on traffic.job.
pub fn decode_job_for(
    operation: &str,
    payload: &serde_json::Value,
) -> Result<JobRequest, &'static str> {
    let request = decode_job(payload)?;
    let valid = match (&request, operation) {
        (JobRequest::Enqueue { classification, .. }, OPERATION_BUN_JOB) => {
            classification.reason == crate::Reason::BunTest
        }
        (JobRequest::Enqueue { classification, .. }, OPERATION_TRAFFIC_JOB) => {
            classification.reason != crate::Reason::BunTest
        }
        (JobRequest::Status { .. } | JobRequest::StatusV2 { .. }, OPERATION_BUN_JOB) => false,
        (_, OPERATION_BUN_JOB | OPERATION_TRAFFIC_JOB) => true,
        _ => false,
    };
    valid.then_some(request).ok_or(ERROR_INVALID_REQUEST)
}

/// Replies to the verified owner. Status replies deliberately omit the context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "state",
    rename_all = "SCREAMING_SNAKE_CASE",
    deny_unknown_fields
)]
pub enum JobResult {
    Bypassed {
        reason: BypassReason,
    },
    Queued {
        sequence: u64,
        queued_at_ms: u64,
        reason: WaitReason,
    },
    Granted {
        sequence: u64,
        queued_at_ms: u64,
        granted_at_ms: u64,
        permit_generation: u64,
        #[serde(deserialize_with = "crate::jobs::object")]
        reservation: Resources,
        lineage_context: LineageContext,
    },
    Reused {
        parent_job_id: JobId,
        permit_generation: u64,
    },
    Running {},
    Completed {
        queue_wait_ms: u64,
        outcome: Outcome,
    },
    Cancelled {},
    Abandoned {},
    ReservationRetained {},
    NotFound {},
}

impl JobResult {
    /// Validate untrusted replies before they influence launch or display.
    pub fn is_valid(&self) -> bool {
        match self {
            Self::Queued {
                sequence,
                queued_at_ms,
                ..
            } => valid_queue(*sequence, *queued_at_ms),
            Self::Granted {
                sequence,
                queued_at_ms,
                granted_at_ms,
                permit_generation,
                reservation,
                ..
            } => {
                valid_queue(*sequence, *queued_at_ms)
                    && (*queued_at_ms..=MAX_TIMESTAMP).contains(granted_at_ms)
                    && valid_generation(*permit_generation)
                    && valid_resources(*reservation)
            }
            Self::Reused {
                permit_generation, ..
            } => valid_generation(*permit_generation),
            Self::Completed {
                queue_wait_ms,
                outcome,
            } => *queue_wait_ms <= MAX_TIMESTAMP && outcome.is_valid(),
            Self::Bypassed { .. }
            | Self::Running {}
            | Self::Cancelled {}
            | Self::Abandoned {}
            | Self::ReservationRetained {}
            | Self::NotFound {} => true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "state",
    rename_all = "SCREAMING_SNAKE_CASE",
    deny_unknown_fields
)]
pub enum StatusResult {
    Queued {
        sequence: u64,
        queued_at_ms: u64,
    },
    Reserved {
        sequence: u64,
        queued_at_ms: u64,
        granted_at_ms: u64,
        #[serde(deserialize_with = "crate::jobs::object")]
        reservation: Resources,
    },
    Running {
        queue_wait_ms: u64,
    },
    Completed {
        queue_wait_ms: u64,
        outcome: Outcome,
    },
    Cancelled {},
    Abandoned {},
    /// A grant exists, but there is no verified evidence that a child started.
    /// Its reservation may still be active.
    Unconfirmed {},
    NotFound {},
}

impl StatusResult {
    pub fn is_valid(&self) -> bool {
        match self {
            Self::Queued {
                sequence,
                queued_at_ms,
            } => valid_queue(*sequence, *queued_at_ms),
            Self::Reserved {
                sequence,
                queued_at_ms,
                granted_at_ms,
                reservation,
            } => {
                valid_queue(*sequence, *queued_at_ms)
                    && (*queued_at_ms..=MAX_TIMESTAMP).contains(granted_at_ms)
                    && valid_resources(*reservation)
            }
            Self::Running { queue_wait_ms } => *queue_wait_ms <= MAX_TIMESTAMP,
            Self::Completed {
                queue_wait_ms,
                outcome,
            } => *queue_wait_ms <= MAX_TIMESTAMP && outcome.is_valid(),
            Self::Cancelled {} | Self::Abandoned {} | Self::Unconfirmed {} | Self::NotFound {} => {
                true
            }
        }
    }
}

fn valid_queue(sequence: u64, queued_at_ms: u64) -> bool {
    (1..=MAX_RESOURCE_VALUE).contains(&sequence) && queued_at_ms <= MAX_TIMESTAMP
}
fn valid_generation(generation: u64) -> bool {
    (1..=MAX_RESOURCE_VALUE).contains(&generation)
}
fn valid_resources(resources: Resources) -> bool {
    (1..=MAX_RESOURCE_VALUE).contains(&resources.cpu_millis)
        && (1..=MAX_RESOURCE_VALUE).contains(&resources.memory_bytes)
}

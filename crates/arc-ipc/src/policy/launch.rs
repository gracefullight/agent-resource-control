//! Launch coordination is independent of display capability negotiation.
//! READY is a bounded publication fence, never proof that a child started.
use super::{decode, EffectivePolicy};
use crate::jobs::{JobId, ProcessIdentity};
use arc_core::meter::MAX_SAFE_INTEGER;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CAPABILITY: &str = "traffic.memory-launch.v1";
pub const OPERATION_TRAFFIC_LAUNCH: &str = "traffic.launch";
pub const OPERATION_TRAFFIC_LAUNCH_ACK: &str = "traffic.launch.ack";
pub const MAX_LAUNCH_PAYLOAD_BYTES: usize = 2048;
/// An ACK may confirm no spawn after freshness expires. This does not extend
/// the publication fence or authorize a target execution.
pub const LAUNCH_ACK_GRACE_MS: u64 = 2000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LaunchRequest {
    pub job_id: JobId,
    pub observer: ProcessIdentity,
    pub permit_generation: u64,
}
impl LaunchRequest {
    pub fn is_valid(&self) -> bool {
        self.observer.is_valid() && (1..=MAX_SAFE_INTEGER).contains(&self.permit_generation)
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "state",
    rename_all = "SCREAMING_SNAKE_CASE",
    deny_unknown_fields
)]
pub enum LaunchResult {
    Ready {
        permit_generation: u64,
        policy: EffectivePolicy,
    },
    Held {
        policy: EffectivePolicy,
    },
    Reused {
        permit_generation: u64,
    },
    Cancelled {},
    NotFound {},
}
impl LaunchResult {
    pub fn is_valid(&self) -> bool {
        match self {
            Self::Ready {
                permit_generation,
                policy,
            } => {
                (1..=MAX_SAFE_INTEGER).contains(permit_generation)
                    && policy.is_valid()
                    && policy.mode == super::PolicyMode::L0
            }
            Self::Held { policy } => {
                policy.is_valid() && policy.mode == super::PolicyMode::MemoryHold
            }
            Self::Reused { permit_generation } => {
                (1..=MAX_SAFE_INTEGER).contains(permit_generation)
            }
            Self::Cancelled {} | Self::NotFound {} => true,
        }
    }
}
/// Sent on the same authenticated fenced launch connection, once. A lost
/// acknowledgement cannot cause a second spawn. A confirmed Unspawned ACK
/// retracts only that known-unspawned release so eligibility can be reevaluated.
/// Native evidence is gathered
/// outside the SQLite transaction; normal attach/completion remains authoritative.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "state",
    rename_all = "SCREAMING_SNAKE_CASE",
    deny_unknown_fields
)]
pub enum LaunchAck {
    Started {
        job_id: JobId,
        observer: ProcessIdentity,
        permit_generation: u64,
        child: ProcessIdentity,
    },
    Unspawned {
        job_id: JobId,
        observer: ProcessIdentity,
        permit_generation: u64,
    },
}
impl LaunchAck {
    pub fn matches(&self, request: &LaunchRequest) -> bool {
        let (job_id, observer, generation) = match self {
            Self::Started {
                job_id,
                observer,
                permit_generation,
                child,
            } if child.is_valid() => (job_id, observer, permit_generation),
            Self::Unspawned {
                job_id,
                observer,
                permit_generation,
            } => (job_id, observer, permit_generation),
            _ => return false,
        };
        request.is_valid()
            && *job_id == request.job_id
            && *observer == request.observer
            && *generation == request.permit_generation
    }
}
pub fn decode_launch_request(value: &Value) -> Result<LaunchRequest, &'static str> {
    let request: LaunchRequest = decode(value, MAX_LAUNCH_PAYLOAD_BYTES)?;
    request
        .is_valid()
        .then_some(request)
        .ok_or(crate::ERROR_INVALID_REQUEST)
}
pub fn decode_launch_result(value: &Value) -> Result<LaunchResult, &'static str> {
    let result: LaunchResult = decode(value, super::MAX_POLICY_BYTES)?;
    result
        .is_valid()
        .then_some(result)
        .ok_or(crate::ERROR_INVALID_REQUEST)
}
pub fn decode_launch_ack(
    value: &Value,
    request: &LaunchRequest,
) -> Result<LaunchAck, &'static str> {
    let ack: LaunchAck = decode(value, MAX_LAUNCH_PAYLOAD_BYTES)?;
    ack.matches(request)
        .then_some(ack)
        .ok_or(crate::ERROR_INVALID_REQUEST)
}

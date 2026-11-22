//! Negotiated read of every recorded decision of one job (lane C, TRF-015).
//!
//! v1 policy and diagnostics replies keep their projected vocabulary: a
//! start-rate wait is projected onto `wait`/`fifo` there. The raw reason,
//! including `start_rate`, crosses the wire only in a reply to
//! [`OPERATION_DECISION_HISTORY`], requested after the daemon advertised
//! [`CAPABILITY`] by holding [`ADVERTISEMENT`].
use crate::{jobs::JobId, ERROR_INVALID_REQUEST};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CAPABILITY: &str = "traffic.decision-history.v1";
pub const OPERATION_DECISION_HISTORY: &str = "traffic.decision_history";
pub const ADVERTISEMENT: &str = ".traffic-decision-history-v1";
pub const SCHEMA_VERSION: u32 = 1;
pub const MAX_HISTORY_BYTES: usize = 65_536;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryRequest {
    pub job_id: JobId,
}

/// One recorded decision in recorded order; `kind` and `reason` are raw tags.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryEntry {
    pub index: u64,
    pub decided_at_ms: u64,
    pub kind: String,
    pub reason: String,
}

/// `queue_position` is `(position, of)` while queued; `expected_schedule_ms`
/// is present only when the daemon can derive it, never an invented time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionHistory {
    pub schema_version: u32,
    pub job_id: JobId,
    pub decisions: Vec<HistoryEntry>,
    pub queue_position: Option<(u64, u64)>,
    pub expected_schedule_ms: Option<u64>,
    /// The daemon's frozen Traffic policy and the start-rate switch, so the
    /// reasons above can be read against the settings that produced them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy: Option<PolicySettings>,
}

/// One class of the effective Traffic policy: its window, its per-class
/// resource estimates and its start-rate bucket (`None` is unlimited).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClassSettings {
    pub class: String,
    pub window: u32,
    pub cpu_millis_estimate: u64,
    pub memory_bytes_estimate: u64,
    pub start_tokens_per_minute: Option<u32>,
    pub start_burst: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicySettings {
    pub start_rate_control: bool,
    pub classes: Vec<ClassSettings>,
}

pub fn decode_request(payload: &Value) -> Result<HistoryRequest, &'static str> {
    if !payload.is_object() || payload.to_string().len() > 512 {
        return Err(ERROR_INVALID_REQUEST);
    }
    HistoryRequest::deserialize(payload).map_err(|_| ERROR_INVALID_REQUEST)
}

pub fn decode_reply(value: &Value, job: &JobId) -> Result<DecisionHistory, &'static str> {
    if !value.is_object() || value.to_string().len() > MAX_HISTORY_BYTES {
        return Err(ERROR_INVALID_REQUEST);
    }
    let reply = DecisionHistory::deserialize(value).map_err(|_| ERROR_INVALID_REQUEST)?;
    (reply.schema_version == SCHEMA_VERSION && reply.job_id == *job)
        .then_some(reply)
        .ok_or(ERROR_INVALID_REQUEST)
}

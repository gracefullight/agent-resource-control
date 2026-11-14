//! Negotiated, owner-authenticated, bounded last-recorded admission facts.
//! These declarations enable no daemon handler and expose no lineage secret.
use super::{Resources, MAX_TRAFFIC_PAYLOAD_BYTES};
use crate::{
    jobs::{JobId, Outcome, MAX_TIMESTAMP},
    Classification, Request, Response, ERROR_INVALID_REQUEST, ERROR_UNSUPPORTED_CAPABILITY,
    PROTOCOL_VERSION, STATUS_OK,
};
use arc_policy::{DecisionReason, RecordedDecision, ScriptPolicyLimits};
use serde::{Deserialize, Serialize};

pub const CAPABILITY: &str = "traffic.diagnostics.v1";
pub const OPERATION_CAPABILITIES: &str = "traffic.diagnostics.capabilities";
pub const OPERATION_JOB: &str = "traffic.diagnostics.job";
pub const MAX_QUERY_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct DiagnosticRequest {
    #[cfg_attr(test, ts(type = "string"))]
    pub job_id: JobId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AdmissionState {
    Queued,
    Reserved,
    Running,
    Completed,
    Cancelled,
    Abandoned,
    Unconfirmed,
    Unreserved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct AdmissionDiagnostic {
    pub state: AdmissionState,
    #[cfg_attr(test, ts(type = "number"))]
    pub sequence: u64,
    #[cfg_attr(test, ts(type = "number"))]
    pub queued_at_ms: u64,
    #[serde(deserialize_with = "crate::jobs::required_option")]
    #[cfg_attr(test, ts(type = "number | null"))]
    pub granted_at_ms: Option<u64>,
    #[serde(deserialize_with = "crate::jobs::required_option")]
    #[cfg_attr(test, ts(type = "number | null"))]
    pub queue_wait_ms: Option<u64>,
    /// A historical cause, never a freshly recomputed admission decision.
    #[serde(deserialize_with = "crate::jobs::required_option")]
    #[cfg_attr(test, ts(type = "DecisionReason | null"))]
    pub wait_reason: Option<DecisionReason>,
    #[serde(deserialize_with = "crate::jobs::required_option")]
    #[cfg_attr(test, ts(type = "RecordedDecision | null"))]
    pub last_decision: Option<RecordedDecision>,
    #[serde(deserialize_with = "crate::jobs::required_option")]
    #[cfg_attr(test, ts(type = "string | null"))]
    pub root_job_id: Option<JobId>,
    #[serde(deserialize_with = "crate::jobs::required_option")]
    #[cfg_attr(test, ts(type = "number | null"))]
    pub permit_generation: Option<u64>,
    #[serde(deserialize_with = "crate::jobs::required_option")]
    #[cfg_attr(test, ts(type = "{ cpu_millis: number, memory_bytes: number } | null"))]
    pub reservation: Option<Resources>,
    /// Last committed permit state. Does not imply fresh child liveness.
    pub permit_active: bool,
    #[serde(deserialize_with = "crate::jobs::required_option")]
    #[cfg_attr(test, ts(as = "Option<typescript::outcome::OutcomeType>"))]
    pub outcome: Option<Outcome>,
}
impl AdmissionDiagnostic {
    pub fn is_valid(&self) -> bool {
        use AdmissionState as S;
        if self.sequence == 0
            || self.sequence > MAX_TIMESTAMP
            || self.queued_at_ms > MAX_TIMESTAMP
            || self
                .granted_at_ms
                .is_some_and(|t| t < self.queued_at_ms || t > MAX_TIMESTAMP)
            || self.queue_wait_ms.is_some_and(|t| t > MAX_TIMESTAMP)
            || self
                .granted_at_ms
                .zip(self.queue_wait_ms)
                .is_some_and(|(grant, wait)| grant.checked_sub(self.queued_at_ms) != Some(wait))
            || self
                .permit_generation
                .is_some_and(|v| !(1..=MAX_TIMESTAMP).contains(&v))
            || self.reservation.is_some_and(|r| {
                !super::valid_resources(r)
                    || r.cpu_millis > MAX_TIMESTAMP
                    || r.memory_bytes > MAX_TIMESTAMP
            })
            || self.last_decision.as_ref().is_some_and(|d| !d.is_valid())
            || self.outcome.as_ref().is_some_and(|o| !o.is_valid())
        {
            return false;
        }
        let permit_complete = self.root_job_id.is_some()
            && self.permit_generation.is_some()
            && self.reservation.is_some();
        if (self.root_job_id.is_some()
            || self.permit_generation.is_some()
            || self.reservation.is_some())
            && !permit_complete
        {
            return false;
        }
        if self.permit_active && !permit_complete {
            return false;
        }
        if let Some(wait) = self.wait_reason {
            if self.state != S::Queued
                || wait.legacy_wait().is_none()
                || !self
                    .last_decision
                    .as_ref()
                    .is_some_and(|d| d.kind == arc_policy::DecisionKind::Wait && d.reason == wait)
            {
                return false;
            }
        }
        match self.state {
            S::Queued => {
                !self.permit_active
                    && !permit_complete
                    && self.granted_at_ms.is_none()
                    && self.queue_wait_ms.is_none()
                    && self.outcome.is_none()
            }
            S::Reserved => {
                self.permit_active && self.granted_at_ms.is_some() && self.outcome.is_none()
            }
            S::Running => self.queue_wait_ms.is_some() && self.outcome.is_none(),
            S::Completed => {
                // A completed root/member may still share its active permit
                // with verified descendants. Keep the outcome and permit fact.
                self.queue_wait_ms.is_some() && self.outcome.is_some()
            }
            // Cancellation or recovery can terminate a root while descendants
            // still share its permit. Permit completeness is checked above.
            S::Cancelled | S::Abandoned => self.outcome.is_none(),
            S::Unconfirmed => self.outcome.is_none(),
            S::Unreserved => !self.permit_active && self.outcome.is_none(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(deny_unknown_fields)]
pub struct DiagnosticResult {
    #[cfg_attr(test, ts(type = "1"))]
    pub schema_version: u32,
    #[cfg_attr(test, ts(type = "string"))]
    pub job_id: JobId,
    #[cfg_attr(test, ts(type = "number"))]
    pub recorded_at_ms: u64,
    /// Always false: this read does not perform a native liveness observation.
    #[cfg_attr(test, ts(type = "false"))]
    pub liveness_fresh: bool,
    #[serde(deserialize_with = "crate::jobs::required_option")]
    #[cfg_attr(test, ts(type = "TrafficClassification | null"))]
    pub classification: Option<Classification>,
    #[serde(deserialize_with = "crate::jobs::required_option")]
    pub admission: Option<AdmissionDiagnostic>,
    #[serde(deserialize_with = "crate::jobs::object")]
    #[cfg_attr(test, ts(type = "ScriptPolicyLimits"))]
    pub limits: ScriptPolicyLimits,
}
impl DiagnosticResult {
    pub fn is_valid(&self) -> bool {
        self.schema_version == 1
            && self.recorded_at_ms <= MAX_TIMESTAMP
            && !self.liveness_fresh
            && self.limits.is_valid()
            && self
                .classification
                .as_ref()
                .is_none_or(Classification::is_consistent)
            && self.admission.as_ref().is_none_or(|a| {
                a.is_valid()
                    && self.classification.is_some_and(|c| {
                        c.locality == crate::Locality::HostLocal
                            && matches!(
                                c.classification,
                                crate::JobClass::Build
                                    | crate::JobClass::Test
                                    | crate::JobClass::Lint
                                    | crate::JobClass::Typecheck
                            )
                    })
                    && a.queued_at_ms <= self.recorded_at_ms
                    && a.queue_wait_ms.is_none_or(|wait| {
                        a.queued_at_ms
                            .checked_add(wait)
                            .is_some_and(|started| started <= self.recorded_at_ms)
                    })
                    && a.granted_at_ms.is_none_or(|t| t <= self.recorded_at_ms)
                    && a.last_decision
                        .as_ref()
                        .is_none_or(|d| d.decided_at_ms <= self.recorded_at_ms)
            })
    }
}

pub fn decode_request(payload: &serde_json::Value) -> Result<DiagnosticRequest, &'static str> {
    crate::policy::decode(payload, MAX_QUERY_BYTES)
}
pub fn decode_result(
    payload: &serde_json::Value,
    job_id: &JobId,
) -> Result<DiagnosticResult, &'static str> {
    let result: DiagnosticResult = crate::policy::decode(payload, MAX_TRAFFIC_PAYLOAD_BYTES)?;
    (result.job_id == *job_id && result.is_valid())
        .then_some(result)
        .ok_or(ERROR_INVALID_REQUEST)
}
pub fn interpret_result(
    request: &Request,
    response: &Response,
) -> Result<DiagnosticResult, &'static str> {
    if request.operation != OPERATION_JOB {
        return Err(ERROR_UNSUPPORTED_CAPABILITY);
    }
    if request.protocol_version != PROTOCOL_VERSION || response.protocol_version != PROTOCOL_VERSION
    {
        return Err(crate::ERROR_PROTOCOL_INCOMPATIBLE);
    }
    let query = decode_request(&request.payload)?;
    if request.request_id != response.request_id {
        return Err(ERROR_INVALID_REQUEST);
    }
    if response.status == crate::STATUS_ERROR
        && response.result.is_none()
        && response
            .error
            .as_ref()
            .is_some_and(|e| e.code == ERROR_UNSUPPORTED_CAPABILITY)
    {
        return Err(ERROR_UNSUPPORTED_CAPABILITY);
    }
    if response.status != STATUS_OK || response.error.is_some() {
        return Err(ERROR_INVALID_REQUEST);
    }
    decode_result(
        response.result.as_ref().ok_or(ERROR_INVALID_REQUEST)?,
        &query.job_id,
    )
}

#[cfg(test)]
mod typescript;

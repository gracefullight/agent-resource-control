//! Additive authenticated reads. These helpers register no daemon handlers and
//! alter none of the existing protocol-1 Traffic/Meter reply shapes.
use crate::{
    jobs::JobId, Request, Response, ERROR_INVALID_REQUEST, PROTOCOL_VERSION, STATUS_ERROR,
    STATUS_OK,
};
pub use arc_policy::*;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;

pub mod launch;

pub const CAPABILITY: &str = "policy.memory.v1";
pub const OPERATION_POLICY_CAPABILITIES: &str = "policy.capabilities";
pub const OPERATION_POLICY_CURRENT: &str = "policy.current";
pub const OPERATION_POLICY_JOB: &str = "policy.job";
pub const ERROR_POLICY_UNAVAILABLE: &str = "POLICY_UNAVAILABLE";
pub const MAX_POLICY_QUERY_BYTES: usize = 512;
pub const POLICY_READ_MAX_BUDGET_MS: u64 = 2000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capabilities {
    pub capability: String,
    pub launch_capability: String,
}
impl Capabilities {
    pub fn supported() -> Self {
        Self {
            capability: CAPABILITY.into(),
            launch_capability: launch::CAPABILITY.into(),
        }
    }
    pub fn is_valid(&self) -> bool {
        self.capability == CAPABILITY && self.launch_capability == launch::CAPABILITY
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobPolicyRequest {
    pub job_id: JobId,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionAvailability {
    Available,
    NotRecorded,
    Expired,
    NotFound,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobPolicyResult {
    pub schema_version: u32,
    pub job_id: JobId,
    /// Recomputed from the current bounded observation at read time.
    pub effective: EffectivePolicy,
    pub decision_availability: DecisionAvailability,
    /// Historical facts as committed for this job, with their original age.
    pub last_decision: Option<RecordedDecision>,
}
impl JobPolicyResult {
    pub fn is_valid(&self) -> bool {
        self.schema_version == POLICY_SCHEMA_VERSION
            && self.effective.is_valid()
            && (self.decision_availability == DecisionAvailability::Available)
                == self.last_decision.is_some()
            && self
                .last_decision
                .as_ref()
                .is_none_or(RecordedDecision::is_valid)
    }
}

pub(super) fn object_tree(value: &Value) -> bool {
    match value {
        Value::Array(_) => false,
        Value::Object(map) => map.values().all(object_tree),
        _ => true,
    }
}
pub(super) fn decode<T: DeserializeOwned>(
    value: &Value,
    maximum: usize,
) -> Result<T, &'static str> {
    if !value.is_object()
        || !object_tree(value)
        || serde_json::to_vec(value).map_or(true, |bytes| bytes.len() > maximum)
    {
        return Err(ERROR_INVALID_REQUEST);
    }
    T::deserialize(value).map_err(|_| ERROR_INVALID_REQUEST)
}
pub fn decode_empty(payload: &Value) -> Result<(), &'static str> {
    payload.is_null().then_some(()).ok_or(ERROR_INVALID_REQUEST)
}
pub fn decode_job_request(payload: &Value) -> Result<JobPolicyRequest, &'static str> {
    decode(payload, MAX_POLICY_QUERY_BYTES)
}
pub fn decode_current(payload: &Value) -> Result<EffectivePolicy, &'static str> {
    let policy: EffectivePolicy = decode(payload, MAX_POLICY_BYTES)?;
    policy
        .is_valid()
        .then_some(policy)
        .ok_or(ERROR_INVALID_REQUEST)
}
pub fn decode_job(payload: &Value, job_id: &JobId) -> Result<JobPolicyResult, &'static str> {
    let result: JobPolicyResult = decode(payload, MAX_POLICY_BYTES)?;
    (result.is_valid() && result.job_id == *job_id)
        .then_some(result)
        .ok_or(ERROR_INVALID_REQUEST)
}
pub fn capabilities_request(request_id: impl Into<String>) -> Request {
    Request::new(request_id, OPERATION_POLICY_CAPABILITIES)
}
pub fn current_request(request_id: impl Into<String>) -> Request {
    Request::new(request_id, OPERATION_POLICY_CURRENT)
}
pub fn job_request(request_id: impl Into<String>, job_id: &JobId) -> Request {
    let mut request = Request::new(request_id, OPERATION_POLICY_JOB);
    request.payload = serde_json::json!({ "job_id": job_id });
    request
}
fn valid_request(request: &Request, operation: &str) -> Result<(), &'static str> {
    if request.protocol_version != PROTOCOL_VERSION {
        return Err(crate::ERROR_PROTOCOL_INCOMPATIBLE);
    }
    (request.operation == operation)
        .then_some(())
        .ok_or(ERROR_INVALID_REQUEST)
}
pub fn capabilities_response(request: &Request) -> Response {
    let result = (|| {
        valid_request(request, OPERATION_POLICY_CAPABILITIES)?;
        decode_empty(&request.payload)?;
        Ok(serde_json::json!(Capabilities::supported()))
    })();
    reply(request, result)
}
pub fn current_response(request: &Request, policy: &EffectivePolicy) -> Response {
    let result = (|| {
        valid_request(request, OPERATION_POLICY_CURRENT)?;
        decode_empty(&request.payload)?;
        let value = serde_json::to_value(policy).map_err(|_| ERROR_INVALID_REQUEST)?;
        decode_current(&value)?;
        Ok(value)
    })();
    reply(request, result)
}
pub fn job_response(request: &Request, policy: &JobPolicyResult) -> Response {
    let result = (|| {
        valid_request(request, OPERATION_POLICY_JOB)?;
        let query = decode_job_request(&request.payload)?;
        let value = serde_json::to_value(policy).map_err(|_| ERROR_INVALID_REQUEST)?;
        decode_job(&value, &query.job_id)?;
        Ok(value)
    })();
    reply(request, result)
}
fn reply(request: &Request, value: Result<Value, &'static str>) -> Response {
    match value {
        Ok(value) => Response::ok(&request.request_id, value),
        Err(code) => Response::error(&request.request_id, code),
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyReadError {
    Unsupported,
    Unavailable,
    Protocol,
    InvalidReply,
}
fn response_value<'a>(
    request: &Request,
    response: &'a Response,
    operation: &str,
) -> Result<&'a Value, PolicyReadError> {
    if request.protocol_version != PROTOCOL_VERSION || response.protocol_version != PROTOCOL_VERSION
    {
        return Err(PolicyReadError::Protocol);
    }
    if request.operation != operation || request.request_id != response.request_id {
        return Err(PolicyReadError::InvalidReply);
    }
    match (response.status.as_str(), &response.result, &response.error) {
        (STATUS_OK, Some(value), None) => Ok(value),
        (STATUS_ERROR, None, Some(error)) => Err(match error.code.as_str() {
            crate::ERROR_UNSUPPORTED_CAPABILITY => PolicyReadError::Unsupported,
            ERROR_POLICY_UNAVAILABLE => PolicyReadError::Unavailable,
            crate::ERROR_PROTOCOL_INCOMPATIBLE => PolicyReadError::Protocol,
            _ => PolicyReadError::InvalidReply,
        }),
        _ => Err(PolicyReadError::InvalidReply),
    }
}
pub fn interpret_capabilities(
    request: &Request,
    response: &Response,
) -> Result<Capabilities, PolicyReadError> {
    decode_empty(&request.payload).map_err(|_| PolicyReadError::InvalidReply)?;
    let value = response_value(request, response, OPERATION_POLICY_CAPABILITIES)?;
    let capabilities: Capabilities =
        decode(value, MAX_POLICY_QUERY_BYTES).map_err(|_| PolicyReadError::InvalidReply)?;
    capabilities
        .is_valid()
        .then_some(capabilities)
        .ok_or(PolicyReadError::InvalidReply)
}
pub fn interpret_current(
    request: &Request,
    response: &Response,
) -> Result<EffectivePolicy, PolicyReadError> {
    decode_empty(&request.payload).map_err(|_| PolicyReadError::InvalidReply)?;
    let value = response_value(request, response, OPERATION_POLICY_CURRENT)?;
    decode_current(value).map_err(|_| PolicyReadError::InvalidReply)
}
pub fn interpret_job(
    request: &Request,
    response: &Response,
) -> Result<JobPolicyResult, PolicyReadError> {
    let query = decode_job_request(&request.payload).map_err(|_| PolicyReadError::InvalidReply)?;
    let value = response_value(request, response, OPERATION_POLICY_JOB)?;
    decode_job(value, &query.job_id).map_err(|_| PolicyReadError::InvalidReply)
}

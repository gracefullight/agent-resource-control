use super::*;
use crate::{
    policy::decode, Request, Response, ERROR_INVALID_REQUEST, PROTOCOL_VERSION, STATUS_OK,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CAPABILITY: &str = "governor.v1";
pub const EXECUTION_CAPABILITY: &str = "governor.execution.v1";
pub const OPERATION_CAPABILITIES: &str = "governor.capabilities";
pub const OPERATION_PAUSE: &str = "daemon.pause";
pub const OPERATION_RESUME: &str = "daemon.resume";
pub const OPERATION_SET_SWITCH: &str = "policy.set_switch";
pub const OPERATION_GET_SWITCH: &str = "policy.get_switch";
pub const OPERATION_JOB: &str = "governor.job";
pub const OPERATION_LAUNCH: &str = "governor.launch";
pub const OPERATION_LAUNCH_ACK: &str = "governor.launch.ack";
pub const MAX_LAUNCH_VALID_FOR_MS: u64 = 2000;
pub const SWITCH_KEY: &str = "governor.enabled";
pub const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
pub const MAX_SWITCH_BYTES: usize = 2048;
pub const MAX_EXECUTION_BYTES: usize = 16384;
pub const MAX_MUTATION_RECEIPTS: usize = 1024;
pub const ERROR_UNAVAILABLE: &str = "GOVERNOR_UNAVAILABLE";
/// Sent/queued work might have committed. Read back; never claim rollback.
pub const ERROR_WRITE_UNCERTAIN: &str = "GOVERNOR_WRITE_UNCERTAIN";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GovernorCapabilities {
    pub capability: String,
    pub execution_capability: String,
}
impl GovernorCapabilities {
    pub fn supported() -> Self {
        Self {
            capability: CAPABILITY.into(),
            execution_capability: EXECUTION_CAPABILITY.into(),
        }
    }
    pub fn is_valid(&self) -> bool {
        self == &Self::supported()
    }
}

pub fn decode_get_switch(payload: &Value) -> Result<GetGovernorSwitchRequest, &'static str> {
    let query: GetGovernorSwitchRequest = decode(payload, MAX_SWITCH_BYTES)?;
    if query
        .mutation_id
        .as_ref()
        .is_some_and(|id| super::switch::mutation_base(id).is_none())
    {
        return Err(ERROR_INVALID_REQUEST);
    }
    Ok(query)
}
pub fn decode_switch_read_result(
    payload: &Value,
    query: &GetGovernorSwitchRequest,
) -> Result<GovernorSwitchReadResult, &'static str> {
    let result: GovernorSwitchReadResult = decode(payload, MAX_SWITCH_BYTES)?;
    result
        .is_valid_for(query)
        .then_some(result)
        .ok_or(ERROR_INVALID_REQUEST)
}
pub fn decode_set_switch(
    operation: &str,
    payload: &Value,
) -> Result<SetGovernorSwitchRequest, &'static str> {
    let request: SetGovernorSwitchRequest = decode(payload, MAX_SWITCH_BYTES)?;
    let alias_matches = match operation {
        OPERATION_PAUSE => !request.enabled,
        OPERATION_RESUME => request.enabled,
        OPERATION_SET_SWITCH => true,
        _ => false,
    };
    (request.is_valid() && alias_matches)
        .then_some(request)
        .ok_or(ERROR_INVALID_REQUEST)
}
pub fn decode_switch_state(payload: &Value) -> Result<GovernorSwitchState, &'static str> {
    let state: GovernorSwitchState = decode(payload, MAX_SWITCH_BYTES)?;
    state
        .is_valid()
        .then_some(state)
        .ok_or(ERROR_INVALID_REQUEST)
}
pub fn decode_mutation_result(
    payload: &Value,
    request: &SetGovernorSwitchRequest,
) -> Result<GovernorMutationResult, &'static str> {
    let result: GovernorMutationResult = decode(payload, MAX_SWITCH_BYTES)?;
    result
        .is_valid_for(request)
        .then_some(result)
        .ok_or(ERROR_INVALID_REQUEST)
}
pub fn decode_job(payload: &Value) -> Result<GovernorJobRequest, &'static str> {
    let request: GovernorJobRequest = decode(payload, MAX_EXECUTION_BYTES)?;
    let classification_allowed = match &request {
        GovernorJobRequest::Traffic {
            request: crate::traffic::JobRequest::Enqueue { classification, .. },
        } => {
            classification.reason == crate::Reason::BunTest
                || classification.legacy() == *classification
        }
        _ => true,
    };
    (request.is_valid() && classification_allowed)
        .then_some(request)
        .ok_or(ERROR_INVALID_REQUEST)
}
pub fn decode_job_result(payload: &Value) -> Result<GovernorJobResult, &'static str> {
    let result: GovernorJobResult = decode(payload, MAX_EXECUTION_BYTES)?;
    result
        .is_valid()
        .then_some(result)
        .ok_or(ERROR_INVALID_REQUEST)
}
pub fn decode_launch(payload: &Value) -> Result<GovernorLaunchRequest, &'static str> {
    let request: GovernorLaunchRequest = decode(payload, MAX_EXECUTION_BYTES)?;
    request
        .is_valid()
        .then_some(request)
        .ok_or(ERROR_INVALID_REQUEST)
}
pub fn decode_launch_result(payload: &Value) -> Result<GovernorLaunchResult, &'static str> {
    let result: GovernorLaunchResult = decode(payload, MAX_EXECUTION_BYTES)?;
    result
        .is_valid()
        .then_some(result)
        .ok_or(ERROR_INVALID_REQUEST)
}
pub fn decode_launch_ack(
    payload: &Value,
    request: &GovernorLaunchRequest,
    result: &GovernorLaunchResult,
) -> Result<GovernorLaunchAck, &'static str> {
    let ack: GovernorLaunchAck = decode(payload, MAX_EXECUTION_BYTES)?;
    ack.matches(request, result)
        .then_some(ack)
        .ok_or(ERROR_INVALID_REQUEST)
}

pub fn capabilities_request(request_id: impl Into<String>) -> Request {
    Request::new(request_id, OPERATION_CAPABILITIES)
}
pub fn capabilities_response(request: &Request) -> Response {
    if request.protocol_version != PROTOCOL_VERSION {
        return Response::error(&request.request_id, crate::ERROR_PROTOCOL_INCOMPATIBLE);
    }
    if request.operation != OPERATION_CAPABILITIES || !request.payload.is_null() {
        return Response::error(&request.request_id, ERROR_INVALID_REQUEST);
    }
    Response::ok(
        &request.request_id,
        serde_json::json!(GovernorCapabilities::supported()),
    )
}
pub fn interpret_capabilities(
    request: &Request,
    response: &Response,
) -> Result<GovernorCapabilities, &'static str> {
    if request.protocol_version != PROTOCOL_VERSION || response.protocol_version != PROTOCOL_VERSION
    {
        return Err(crate::ERROR_PROTOCOL_INCOMPATIBLE);
    }
    if request.operation != OPERATION_CAPABILITIES
        || !request.payload.is_null()
        || request.request_id != response.request_id
    {
        return Err(ERROR_INVALID_REQUEST);
    }
    if response.status == crate::STATUS_ERROR
        && response.result.is_none()
        && response
            .error
            .as_ref()
            .is_some_and(|e| e.code == crate::ERROR_UNSUPPORTED_CAPABILITY)
    {
        return Err(crate::ERROR_UNSUPPORTED_CAPABILITY);
    }
    if response.status != STATUS_OK || response.error.is_some() {
        return Err(ERROR_INVALID_REQUEST);
    }
    let value = response.result.as_ref().ok_or(ERROR_INVALID_REQUEST)?;
    let result: GovernorCapabilities = decode(value, MAX_SWITCH_BYTES)?;
    result
        .is_valid()
        .then_some(result)
        .ok_or(ERROR_INVALID_REQUEST)
}

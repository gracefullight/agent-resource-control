//! Opt-in endpoint-derived local Build admission using the existing Governor.
use crate::{
    governor::GovernorJobRequest, traffic::JobRequest, ERROR_INVALID_REQUEST,
    ERROR_UNSUPPORTED_CAPABILITY,
};

pub const CAPABILITY: &str = "traffic.endpoint.v1";
pub const OPERATION_CAPABILITIES: &str = "traffic.endpoint.capabilities";
pub const OPERATION_JOB: &str = "traffic.endpoint.job";

impl super::Capabilities {
    pub fn endpoint_supported() -> Self {
        Self {
            capability: CAPABILITY.to_owned(),
        }
    }
}

pub fn decode_job(
    operation: &str,
    payload: &serde_json::Value,
) -> Result<GovernorJobRequest, &'static str> {
    if operation != OPERATION_JOB {
        return Err(ERROR_UNSUPPORTED_CAPABILITY);
    }
    let request: GovernorJobRequest =
        crate::policy::decode(payload, crate::governor::MAX_EXECUTION_BYTES)?;
    let allowed = match &request {
        GovernorJobRequest::Traffic {
            request: JobRequest::Enqueue { classification, .. },
        } => classification.reason == crate::Reason::LocalEndpointBuild,
        _ => true,
    };
    (allowed && request.is_valid())
        .then_some(request)
        .ok_or(ERROR_INVALID_REQUEST)
}

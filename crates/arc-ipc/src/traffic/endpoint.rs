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
    let mut request: GovernorJobRequest =
        crate::policy::decode(payload, crate::governor::MAX_EXECUTION_BYTES)?;
    if !request.is_valid() {
        return Err(ERROR_INVALID_REQUEST);
    }
    if let GovernorJobRequest::Traffic {
        request: JobRequest::Enqueue { classification, .. },
    } = &mut request
    {
        if classification.reason != crate::Reason::LocalEndpointBuild {
            return Err(ERROR_INVALID_REQUEST);
        }
        // A request carries the HOST_LOCAL projection; record the canonical form.
        *classification = crate::compute_location::canonical(*classification);
    }
    Ok(request)
}

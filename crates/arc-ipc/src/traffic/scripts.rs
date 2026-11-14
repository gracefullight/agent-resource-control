//! Bun-script capability reuses the complete Governor execution envelope.
//! Its launch/ACK protocol is unchanged and contains no classification enum.
use crate::{
    governor::GovernorJobRequest, traffic::JobRequest, ERROR_INVALID_REQUEST,
    ERROR_UNSUPPORTED_CAPABILITY,
};

pub fn decode_job(
    operation: &str,
    payload: &serde_json::Value,
) -> Result<GovernorJobRequest, &'static str> {
    // Test support before decoding any newly introduced enum vocabulary.
    if operation != super::OPERATION_BUN_SCRIPTS_JOB {
        return Err(ERROR_UNSUPPORTED_CAPABILITY);
    }
    let request: GovernorJobRequest =
        crate::policy::decode(payload, crate::governor::MAX_EXECUTION_BYTES)?;
    let classification_allowed = match &request {
        GovernorJobRequest::Traffic {
            request: JobRequest::Enqueue { classification, .. },
        } => classification.is_bun_script(),
        _ => true,
    };
    (request.is_valid() && classification_allowed)
        .then_some(request)
        .ok_or(ERROR_INVALID_REQUEST)
}

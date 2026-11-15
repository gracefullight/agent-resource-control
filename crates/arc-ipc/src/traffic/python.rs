//! Python TEST admission reuses the Governor execution and launch/ACK envelope.
use crate::{
    governor::GovernorJobRequest, traffic::JobRequest, ERROR_INVALID_REQUEST,
    ERROR_UNSUPPORTED_CAPABILITY,
};

pub fn decode_job(
    operation: &str,
    payload: &serde_json::Value,
) -> Result<GovernorJobRequest, &'static str> {
    // Reject unsupported operations before decoding any additive vocabulary.
    if operation != super::OPERATION_PYTHON_TESTS_JOB {
        return Err(ERROR_UNSUPPORTED_CAPABILITY);
    }
    let request: GovernorJobRequest =
        crate::policy::decode(payload, crate::governor::MAX_EXECUTION_BYTES)?;
    let allowed = match &request {
        GovernorJobRequest::Traffic {
            request: JobRequest::Enqueue { classification, .. },
        } => classification.is_python_test(),
        _ => true,
    };
    (allowed && request.is_valid())
        .then_some(request)
        .ok_or(ERROR_INVALID_REQUEST)
}

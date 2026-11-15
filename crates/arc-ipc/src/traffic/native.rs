//! One native-tools extension; existing Governor/diagnostic envelopes and authentication apply.
//! Constants and decoding alone never advertise or enable a daemon route.
use crate::{traffic::JobRequest, ERROR_INVALID_REQUEST, ERROR_UNSUPPORTED_CAPABILITY};

pub use super::diagnostics::{
    DiagnosticRequest as NativeDiagnosticRequest, DiagnosticResult as NativeDiagnosticResult,
};
pub use crate::governor::{
    GovernorJobRequest as NativeJobRequest, GovernorJobResult as NativeJobResult,
};

pub const CAPABILITY: &str = "traffic.native-tools.v1";
pub const OPERATION_CAPABILITIES: &str = "traffic.native-tools.capabilities";
pub const OPERATION_JOB: &str = "traffic.native-tools.job";
pub const DIAGNOSTICS_CAPABILITY: &str = "traffic.native-tools.diagnostics.v1";
pub const OPERATION_DIAGNOSTICS_CAPABILITIES: &str =
    "traffic.native-tools.diagnostics.capabilities";
pub const OPERATION_DIAGNOSTICS_JOB: &str = "traffic.native-tools.diagnostics.job";

impl super::Capabilities {
    pub fn native_supported() -> Self {
        Self {
            capability: CAPABILITY.to_owned(),
        }
    }
}

pub fn decode_job(
    operation: &str,
    payload: &serde_json::Value,
) -> Result<NativeJobRequest, &'static str> {
    if operation != OPERATION_JOB {
        return Err(ERROR_UNSUPPORTED_CAPABILITY);
    }
    let request: NativeJobRequest =
        crate::policy::decode(payload, crate::governor::MAX_EXECUTION_BYTES)?;
    let allowed = match &request {
        NativeJobRequest::Traffic {
            request: JobRequest::Enqueue { classification, .. },
        } => classification.is_native_tool(),
        _ => true,
    };
    (allowed && request.is_valid())
        .then_some(request)
        .ok_or(ERROR_INVALID_REQUEST)
}

//! Negotiated last-recorded facts; scheduling and accounting remain daemon-owned.
use super::{validate_job_id, DesktopBridge, READ_DEADLINE};
use crate::{
    dto::{DiagnosticResult, Received},
    error::{DesktopError, ErrorCode},
};
use arc_ipc::{
    jobs::JobId,
    traffic::{diagnostics, Capabilities},
};
use serde_json::{json, Value};
use std::time::Instant;

impl DesktopBridge {
    pub fn traffic_job_diagnostics(
        &self,
        job_id: &str,
    ) -> Result<Received<DiagnosticResult>, DesktopError> {
        let id = validate_job_id(job_id)?;
        self.sync_read(|bridge| bridge.read_traffic_diagnostics(id))
    }

    pub async fn traffic_job_diagnostics_async(
        &self,
        job_id: String,
    ) -> Result<Received<DiagnosticResult>, DesktopError> {
        let id = validate_job_id(&job_id)?;
        self.blocking(move |bridge| bridge.read_traffic_diagnostics(id))
            .await
    }

    fn read_traffic_diagnostics(
        &self,
        id: JobId,
    ) -> Result<Received<DiagnosticResult>, DesktopError> {
        // Probe on every read: a replacement peer cannot inherit an earlier
        // peer's capability. Both exchanges share one deadline and permit.
        let deadline = Instant::now() + READ_DEADLINE;
        let value = self.session_exchange(
            diagnostics::OPERATION_CAPABILITIES,
            Value::Null,
            deadline,
            false,
        )?;
        if !value.is_object()
            || serde_json::to_vec(&value)
                .map_or(true, |bytes| bytes.len() > diagnostics::MAX_QUERY_BYTES)
        {
            return Err(DesktopError::new(ErrorCode::InvalidReply));
        }
        let capability: Capabilities = serde_json::from_value(value)
            .map_err(|_| DesktopError::new(ErrorCode::InvalidReply))?;
        if capability.capability != diagnostics::CAPABILITY {
            return Err(DesktopError::new(ErrorCode::InvalidReply));
        }
        let value = self.session_exchange(
            diagnostics::OPERATION_JOB,
            json!({ "job_id": id }),
            deadline,
            false,
        )?;
        let result = diagnostics::decode_result(&value, &id)
            .map_err(|_| DesktopError::new(ErrorCode::InvalidReply))?;
        self.received(result)
    }
}

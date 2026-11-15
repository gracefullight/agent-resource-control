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
        // peer's capability. Only explicit unsupported permits the old read;
        // all probes and the selected query share one deadline and permit.
        let deadline = Instant::now() + READ_DEADLINE;
        use arc_ipc::traffic::native;
        let mut selected = None;
        for (probe, operation, capability) in [
            (
                native::OPERATION_DIAGNOSTICS_CAPABILITIES,
                native::OPERATION_DIAGNOSTICS_JOB,
                native::DIAGNOSTICS_CAPABILITY,
            ),
            (
                diagnostics::OPERATION_PYTHON_CAPABILITIES,
                diagnostics::OPERATION_PYTHON_JOB,
                diagnostics::PYTHON_CAPABILITY,
            ),
            (
                diagnostics::OPERATION_CAPABILITIES,
                diagnostics::OPERATION_JOB,
                diagnostics::CAPABILITY,
            ),
        ] {
            match self.probe_traffic_diagnostics(probe, capability, deadline) {
                Ok(()) => {
                    selected = Some(operation);
                    break;
                }
                Err(error) if error.code == ErrorCode::Unsupported => continue,
                Err(error) => return Err(error),
            }
        }
        let operation = selected.ok_or_else(|| DesktopError::new(ErrorCode::Unsupported))?;
        let value = self.session_exchange(operation, json!({ "job_id": id }), deadline, false)?;
        let result = diagnostics::decode_result(&value, &id)
            .map_err(|_| DesktopError::new(ErrorCode::InvalidReply))?;
        if result.classification.is_some_and(|c| {
            (operation != native::OPERATION_DIAGNOSTICS_JOB && c.is_native_tool())
                || (operation == diagnostics::OPERATION_JOB && c.is_python_test())
        }) {
            return Err(DesktopError::new(ErrorCode::InvalidReply));
        }
        self.received(result)
    }

    fn probe_traffic_diagnostics(
        &self,
        operation: &'static str,
        expected: &str,
        deadline: Instant,
    ) -> Result<(), DesktopError> {
        let value = self.session_exchange(operation, Value::Null, deadline, false)?;
        if !value.is_object()
            || serde_json::to_vec(&value)
                .map_or(true, |bytes| bytes.len() > diagnostics::MAX_QUERY_BYTES)
        {
            return Err(DesktopError::new(ErrorCode::InvalidReply));
        }
        let capability: Capabilities = serde_json::from_value(value)
            .map_err(|_| DesktopError::new(ErrorCode::InvalidReply))?;
        if capability.capability != expected {
            return Err(DesktopError::new(ErrorCode::InvalidReply));
        }
        Ok(())
    }
}

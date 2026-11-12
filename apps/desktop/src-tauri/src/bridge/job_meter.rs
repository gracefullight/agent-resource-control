//! Job-keyed, read-only accounting through the same authenticated bounded lane.
use super::{validate_job_id, DesktopBridge};
use crate::{
    dto::{JobMeterSummary, Received},
    error::{DesktopError, ErrorCode},
};
use arc_ipc::meter::{decode_summary, OPERATION_METER_SUMMARY};
use serde_json::json;

impl DesktopBridge {
    pub fn meter_summary(&self, job_id: &str) -> Result<Received<JobMeterSummary>, DesktopError> {
        let id = validate_job_id(job_id)?;
        self.sync_read(|bridge| bridge.read_job_meter(id.as_str()))
    }

    pub async fn meter_summary_async(
        &self,
        job_id: String,
    ) -> Result<Received<JobMeterSummary>, DesktopError> {
        let id = validate_job_id(&job_id)?;
        self.blocking(move |bridge| bridge.read_job_meter(id.as_str()))
            .await
    }

    fn read_job_meter(&self, job_id: &str) -> Result<Received<JobMeterSummary>, DesktopError> {
        let value = self.exchange(OPERATION_METER_SUMMARY, json!({ "job_id": job_id }))?;
        let summary = decode_summary(&value, job_id)
            .map_err(|_| DesktopError::new(ErrorCode::InvalidReply))?;
        self.received(summary)
    }
}

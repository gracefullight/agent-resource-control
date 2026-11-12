//! Capability-scoped bounded reads; no policy inference from a Meter display.
use super::{validate_job_id, DesktopBridge, READ_DEADLINE};
use crate::{
    dto::{EffectivePolicy, JobPolicyResult, Received},
    error::{DesktopError, ErrorCode},
};
use arc_ipc::policy::{self, Capabilities};
use serde_json::{json, Value};
use std::time::Instant;

impl DesktopBridge {
    pub fn policy_current(&self) -> Result<Received<EffectivePolicy>, DesktopError> {
        self.sync_read(Self::read_policy_current)
    }
    pub async fn policy_current_async(&self) -> Result<Received<EffectivePolicy>, DesktopError> {
        self.blocking(Self::read_policy_current).await
    }
    pub fn policy_job(&self, job_id: &str) -> Result<Received<JobPolicyResult>, DesktopError> {
        let id = validate_job_id(job_id)?;
        self.sync_read(|bridge| bridge.read_policy_job(id))
    }
    pub async fn policy_job_async(
        &self,
        job_id: String,
    ) -> Result<Received<JobPolicyResult>, DesktopError> {
        let id = validate_job_id(&job_id)?;
        self.blocking(move |bridge| bridge.read_policy_job(id))
            .await
    }
    fn policy_capability(&self, deadline: Instant) -> Result<(), DesktopError> {
        let value =
            self.exchange_until(policy::OPERATION_POLICY_CAPABILITIES, Value::Null, deadline)?;
        if !value.is_object()
            || serde_json::to_vec(&value)
                .map_or(true, |bytes| bytes.len() > policy::MAX_POLICY_QUERY_BYTES)
        {
            return Err(DesktopError::new(ErrorCode::InvalidReply));
        }
        let caps: Capabilities = serde_json::from_value(value)
            .map_err(|_| DesktopError::new(ErrorCode::InvalidReply))?;
        if !caps.is_valid() {
            return Err(DesktopError::new(ErrorCode::InvalidReply));
        }
        Ok(())
    }
    fn read_policy_current(&self) -> Result<Received<EffectivePolicy>, DesktopError> {
        let deadline = Instant::now() + READ_DEADLINE;
        self.policy_capability(deadline)?;
        let value = self.exchange_until(policy::OPERATION_POLICY_CURRENT, Value::Null, deadline)?;
        let current = policy::decode_current(&value)
            .map_err(|_| DesktopError::new(ErrorCode::InvalidReply))?;
        self.received(current)
    }
    fn read_policy_job(
        &self,
        id: arc_ipc::jobs::JobId,
    ) -> Result<Received<JobPolicyResult>, DesktopError> {
        let deadline = Instant::now() + READ_DEADLINE;
        self.policy_capability(deadline)?;
        let value = self.exchange_until(
            policy::OPERATION_POLICY_JOB,
            json!({ "job_id": id }),
            deadline,
        )?;
        let job = policy::decode_job(&value, &id)
            .map_err(|_| DesktopError::new(ErrorCode::InvalidReply))?;
        self.received(job)
    }
}

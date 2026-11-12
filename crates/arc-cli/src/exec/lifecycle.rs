//! Legacy lifecycle metadata exchanges remain independent of native execution.
use super::transport;
use arc_ipc::jobs::{
    CompleteRequest, CompleteResult, JobId, Outcome, ProcessIdentity, StartRequest, StartResult,
    OPERATION_JOB_COMPLETE, OPERATION_JOB_START,
};
use serde_json::json;
use std::{path::Path, time::Instant};

pub(super) fn start_until(
    socket: &Path,
    input: StartRequest,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
) -> Result<StartResult, transport::Failure> {
    use transport::Failure;
    let value = transport::call_until(
        socket,
        OPERATION_JOB_START,
        json!(input),
        deadline,
        cancelled,
    )?;
    let result: StartResult = serde_json::from_value(value).map_err(|_| Failure::InvalidReply)?;
    if result.job.job_id != *input.job_id()
        || result.job.observer != *input.observer()
        || !result.job.is_valid()
    {
        return Err(Failure::InvalidReply);
    }
    match &input {
        StartRequest::Register { classification, .. }
            if result.job.classification != *classification =>
        {
            return Err(Failure::InvalidReply)
        }
        StartRequest::Attach { child, .. } if result.job.child.as_ref() != Some(child) => {
            return Err(Failure::InvalidReply)
        }
        _ => (),
    }
    if cancelled() {
        return Err(Failure::Cancelled);
    }
    if Instant::now() >= deadline {
        return Err(Failure::Uncertain);
    }
    Ok(result)
}
pub(super) fn complete(
    socket: &Path,
    job_id: &JobId,
    observer: &ProcessIdentity,
    outcome: Outcome,
) -> Result<(), ()> {
    let request = CompleteRequest {
        job_id: job_id.clone(),
        observer: observer.clone(),
        outcome,
    };
    let value = transport::call(socket, OPERATION_JOB_COMPLETE, json!(request))?;
    let result: CompleteResult = serde_json::from_value(value).map_err(|_| ())?;
    if !result.job.is_valid()
        || result.job.job_id != request.job_id
        || result.job.observer != request.observer
        || result.job.outcome.as_ref() != Some(&request.outcome)
    {
        return Err(());
    }
    Ok(())
}

//! Lifecycle operations share the diagnostic queue, limits and cancellation.
use super::{DbClient, DbValue};
use arc_ipc::jobs::{
    CompleteRequest, CompleteResult, JobId, JobSnapshot, LifecycleRequest, StartRequest,
    StartResult, StatusRequest, StatusResult,
};
use arc_storage::jobs::{self, JobError, ReconcileEvidence, ReconcileResult, StartEvidence};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub(crate) enum Operation {
    Start(StartRequest, StartEvidence, u64),
    Complete(CompleteRequest),
    Get(JobId),
    Recent,
    Reconcile(ReconcileEvidence),
    Traffic(super::traffic::Operation),
    Meter(super::job_meter::Operation),
    Policy(super::policy::Operation),
}
pub(crate) enum Value {
    Start(Result<StartResult, JobError>),
    Complete(Result<CompleteResult, JobError>),
    Get(Result<Option<JobSnapshot>, JobError>),
    Status(Result<StatusResult, JobError>),
    Reconcile(Result<ReconcileResult, JobError>),
    Traffic(super::traffic::Value),
    Meter(super::job_meter::Value),
    Policy(Result<super::policy::Value, JobError>),
}
pub(crate) fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
#[cfg(test)]
pub(crate) fn execute(connection: &mut rusqlite::Connection, op: Operation) -> Value {
    execute_observed(connection, op, None)
}
pub(crate) fn execute_observed(
    connection: &mut rusqlite::Connection,
    op: Operation,
    reader: Option<&super::observation::Observations>,
) -> Value {
    let value = match op {
        Operation::Start(request, evidence, observed_at_ms) => {
            Value::Start(jobs::start(connection, &request, &evidence, observed_at_ms))
        }
        Operation::Complete(request) => {
            Value::Complete(jobs::complete(connection, &request, now()))
        }
        Operation::Get(id) => Value::Get(jobs::get(connection, &id)),
        Operation::Recent => Value::Status(jobs::recent(connection, now())),
        Operation::Reconcile(evidence) => {
            Value::Reconcile(jobs::apply_observation(connection, &evidence))
        }
        Operation::Traffic(operation) => Value::Traffic(super::traffic::execute_observed(
            connection, operation, reader,
        )),
        Operation::Policy(operation) => {
            Value::Policy(super::policy::execute(connection, operation, reader))
        }
        Operation::Meter(operation) => {
            Value::Meter(super::job_meter::execute(connection, operation))
        }
    };
    // Bounded opportunistic expiry shares this connection and its existing
    // cancellation/deadline. Failure never changes a committed request result.
    let _ = arc_storage::job_meter::expire(connection, now());
    let _ = arc_storage::policy::expire(connection, now());
    value
}
pub(super) async fn call(
    client: &DbClient,
    op: Operation,
    deadline: Instant,
) -> Result<Value, &'static str> {
    let value = client
        .try_job(deadline, op)
        .map_err(|_| arc_ipc::ERROR_DB_BUSY)?
        .receive_value()
        .await
        .map_err(|_| arc_ipc::ERROR_DB_BUSY)?;
    match value {
        DbValue::Lifecycle(value) => Ok(*value),
        _ => Err(arc_ipc::ERROR_DB_BUSY),
    }
}
fn wire<T: serde::Serialize>(
    value: Result<T, JobError>,
) -> Result<serde_json::Value, &'static str> {
    serde_json::to_value(value.map_err(JobError::code)?).map_err(|_| arc_ipc::ERROR_DB_BUSY)
}
pub(crate) async fn handle(
    client: DbClient,
    input: LifecycleRequest,
    deadline: Instant,
) -> Result<serde_json::Value, &'static str> {
    match input {
        LifecycleRequest::Start(request) => {
            // Native probes occur before admission; no SQLite ticket is held.
            let observed_at_ms = now();
            let child = match &request {
                StartRequest::Attach { child, .. } => Some(child),
                _ => None,
            };
            let evidence = StartEvidence::gather(&request)
                .with_meter_evidence(super::job_meter::evidence(request.observer(), child));
            match call(
                &client,
                Operation::Start(request, evidence, observed_at_ms),
                deadline,
            )
            .await?
            {
                Value::Start(value) => wire(value),
                _ => Err(arc_ipc::ERROR_DB_BUSY),
            }
        }
        LifecycleRequest::Complete(request) => {
            match call(&client, Operation::Complete(request), deadline).await? {
                Value::Complete(value) => wire(value),
                _ => Err(arc_ipc::ERROR_DB_BUSY),
            }
        }
        LifecycleRequest::Status(StatusRequest::Recent {}) => {
            match call(&client, Operation::Recent, deadline).await? {
                Value::Status(value) => wire(value),
                _ => Err(arc_ipc::ERROR_DB_BUSY),
            }
        }
        LifecycleRequest::Status(StatusRequest::ById { job_id }) => {
            // Bounded optimistic reconciliation. A concurrent writer may win;
            // reread/reprobe instead of applying stale native evidence.
            for _ in 0..3 {
                let job = match call(&client, Operation::Get(job_id.clone()), deadline).await? {
                    Value::Get(value) => value.map_err(JobError::code)?,
                    _ => return Err(arc_ipc::ERROR_DB_BUSY),
                };
                let Some(job) = job else {
                    return wire(Ok(StatusResult::NotFound {}));
                };
                if job.outcome.is_some() {
                    return wire(Ok(StatusResult::Found { job: Box::new(job) }));
                }
                let evidence = jobs::observe(&job, now().max(job.updated_at_ms));
                let result = match call(&client, Operation::Reconcile(evidence), deadline).await? {
                    Value::Reconcile(value) => value.map_err(JobError::code)?,
                    _ => return Err(arc_ipc::ERROR_DB_BUSY),
                };
                match result {
                    ReconcileResult::Applied(job) | ReconcileResult::Unchanged(job) => {
                        return wire(Ok(StatusResult::Found { job }))
                    }
                    ReconcileResult::NotFound => return wire(Ok(StatusResult::NotFound {})),
                    ReconcileResult::Stale => (),
                }
            }
            Err(arc_ipc::ERROR_DB_BUSY)
        }
    }
}

#[cfg(test)]
#[path = "jobs_tests.rs"]
mod tests;

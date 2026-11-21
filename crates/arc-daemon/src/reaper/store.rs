//! Reaper records travel through the single SQLite writer like every mutation.
use super::{jobs, DbClient};
use arc_ipc::jobs::JobId;
use arc_storage::{
    jobs::{JobError, JobResult},
    reaper::{self, Candidate, Decision, Registration},
};
use std::time::Instant;

pub(crate) enum Operation {
    Register(Box<Registration>, u64),
    EndSession(JobId, u64),
    Inventory(u64, usize),
    Decide(Box<Decision>),
}
pub(crate) enum Value {
    Registered(JobResult<()>),
    Ended(JobResult<u64>),
    Inventory(JobResult<Vec<Candidate>>),
    Decided(JobResult<bool>),
}

pub(super) fn execute(connection: &mut rusqlite::Connection, operation: Operation) -> Value {
    match operation {
        Operation::Register(registration, now) => {
            Value::Registered(reaper::register(connection, &registration, now))
        }
        Operation::EndSession(job_id, now) => {
            Value::Ended(reaper::end_session(connection, &job_id, now))
        }
        Operation::Inventory(now, limit) => {
            Value::Inventory(reaper::inventory(connection, now, limit))
        }
        Operation::Decide(decision) => Value::Decided(reaper::decide(connection, &decision)),
    }
}

async fn call(client: &DbClient, operation: Operation, deadline: Instant) -> JobResult<Value> {
    match jobs::call(client, jobs::Operation::Reaper(operation), deadline).await {
        Ok(jobs::Value::Reaper(value)) => Ok(value),
        _ => Err(JobError::Busy),
    }
}

pub(crate) async fn register(
    client: &DbClient,
    registration: Registration,
    now: u64,
    deadline: Instant,
) -> JobResult<()> {
    match call(
        client,
        Operation::Register(Box::new(registration), now),
        deadline,
    )
    .await?
    {
        Value::Registered(result) => result,
        _ => Err(JobError::StorageFailure),
    }
}

pub(crate) async fn end_session(
    client: &DbClient,
    job_id: &JobId,
    now: u64,
    deadline: Instant,
) -> JobResult<u64> {
    match call(client, Operation::EndSession(job_id.clone(), now), deadline).await? {
        Value::Ended(result) => result,
        _ => Err(JobError::StorageFailure),
    }
}

pub(crate) async fn inventory(
    client: &DbClient,
    now: u64,
    limit: usize,
    deadline: Instant,
) -> JobResult<Vec<Candidate>> {
    match call(client, Operation::Inventory(now, limit), deadline).await? {
        Value::Inventory(result) => result,
        _ => Err(JobError::StorageFailure),
    }
}

pub(crate) async fn decide(
    client: &DbClient,
    decision: Decision,
    deadline: Instant,
) -> JobResult<bool> {
    match call(client, Operation::Decide(Box::new(decision)), deadline).await? {
        Value::Decided(result) => result,
        _ => Err(JobError::StorageFailure),
    }
}

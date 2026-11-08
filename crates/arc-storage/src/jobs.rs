//! Transactional lifecycle metadata. Native evidence is gathered by the caller
//! outside the finite database worker. No operation launches or reserves work.
mod codec;
mod reconcile;
mod transitions;
use arc_ipc::jobs::{JobId, JobSnapshot, MAX_TIMESTAMP};
pub use reconcile::{apply_observation, observe, recent, ReconcileEvidence, ReconcileResult};
use rusqlite::{Connection, OptionalExtension};
use std::{error::Error, fmt};
pub use transitions::{complete, start, StartEvidence};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobError {
    InvalidRequest,
    Conflict,
    InvalidTransition,
    IdentityUnavailable,
    Busy,
    StorageFailure,
}
impl JobError {
    pub fn code(self) -> &'static str {
        match self {
            Self::InvalidRequest => "INVALID_REQUEST",
            Self::Conflict => "JOB_CONFLICT",
            Self::InvalidTransition => "INVALID_JOB_TRANSITION",
            Self::IdentityUnavailable => "PROCESS_IDENTITY_UNAVAILABLE",
            Self::Busy => "DB_BUSY",
            Self::StorageFailure => "DB_BUSY",
        }
    }
}
impl fmt::Display for JobError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}
impl Error for JobError {}
impl From<rusqlite::Error> for JobError {
    fn from(error: rusqlite::Error) -> Self {
        match error {
            rusqlite::Error::SqliteFailure(e, _)
                if matches!(
                    e.code,
                    rusqlite::ErrorCode::DatabaseBusy
                        | rusqlite::ErrorCode::DatabaseLocked
                        | rusqlite::ErrorCode::OperationInterrupted
                ) =>
            {
                Self::Busy
            }
            _ => Self::StorageFailure,
        }
    }
}
pub type JobResult<T> = Result<T, JobError>;

/// Read one last-known snapshot. BY_ID callers probe it and use apply_observation.
pub fn get(connection: &Connection, job_id: &JobId) -> JobResult<Option<JobSnapshot>> {
    let job = connection
        .query_row(codec::SELECT_ONE, [job_id.as_str()], codec::decode)
        .optional()?;
    if job.as_ref().is_some_and(|job| !JobSnapshot::is_valid(job)) {
        return Err(JobError::StorageFailure);
    }
    Ok(job)
}
fn valid_now(now: u64, job: Option<&JobSnapshot>) -> JobResult<()> {
    if now > MAX_TIMESTAMP
        || job.is_some_and(|j| now < j.updated_at_ms || j.revision >= MAX_TIMESTAMP)
    {
        Err(JobError::InvalidRequest)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests;

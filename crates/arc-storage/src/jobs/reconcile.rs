use super::{codec, get, valid_now, JobError, JobResult};
use arc_core::process_identity::{probe_live, IdentityStatus, ProcessIdentity};
use arc_ipc::jobs::{
    IdentityObservation, JobId, JobSnapshot, JobSummary, LifecycleReason as R,
    ObservationStatus as S, ObservationTarget as T, Phase, StatusResult,
};
use rusqlite::{Connection, TransactionBehavior};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconcileResult {
    Applied(Box<JobSnapshot>),
    Unchanged(Box<JobSnapshot>),
    Stale,
    NotFound,
}
/// An observation is tied to a snapshot revision and original tuple. No native
/// probes run inside a SQLite transaction or while holding a database ticket.
#[derive(Debug, Clone)]
pub struct ReconcileEvidence {
    job_id: JobId,
    revision: u64,
    identity: ProcessIdentity,
    target: T,
    status: IdentityStatus,
    observed_at_ms: u64,
}
#[cfg(test)]
pub(super) fn simulated(job: &JobSnapshot, status: IdentityStatus, now: u64) -> ReconcileEvidence {
    let mut evidence = observe(job, now);
    evidence.status = status;
    evidence
}
pub fn observe(job: &JobSnapshot, now: u64) -> ReconcileEvidence {
    let (target, identity) = job
        .child
        .as_ref()
        .map_or((T::Observer, &job.observer), |c| (T::Child, c));
    ReconcileEvidence {
        job_id: job.job_id.clone(),
        revision: job.revision,
        identity: identity.clone(),
        target,
        status: probe_live(identity),
        observed_at_ms: now,
    }
}
pub fn apply_observation(
    connection: &mut Connection,
    evidence: &ReconcileEvidence,
) -> JobResult<ReconcileResult> {
    valid_now(evidence.observed_at_ms, None)?;
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let Some(mut job) = get(&tx, &evidence.job_id)? else {
        return Ok(ReconcileResult::NotFound);
    };
    if job.outcome.is_some() {
        return Ok(ReconcileResult::Unchanged(Box::new(job)));
    }
    let identity = if evidence.target == T::Child {
        job.child.as_ref()
    } else {
        Some(&job.observer)
    };
    if job.revision != evidence.revision || identity != Some(&evidence.identity) {
        return Ok(ReconcileResult::Stale);
    }
    valid_now(evidence.observed_at_ms, Some(&job))?;
    let prior = job.clone();
    update_observation(
        &mut job,
        evidence.target,
        evidence.status,
        evidence.observed_at_ms,
    );
    if job == prior {
        return Ok(ReconcileResult::Unchanged(Box::new(job)));
    }
    job.updated_at_ms = evidence.observed_at_ms;
    job.revision += 1;
    codec::write(&tx, &job, false)?;
    tx.commit()?;
    Ok(ReconcileResult::Applied(Box::new(job)))
}
pub(super) fn update_observation(
    job: &mut JobSnapshot,
    target: T,
    status: IdentityStatus,
    now: u64,
) {
    let unclaimed = job.launch_claimed_at_ms.is_none();
    let (status, reason) = match status {
        IdentityStatus::VerifiedMatch if target == T::Child => (S::VerifiedMatch, R::ChildVerified),
        IdentityStatus::VerifiedMatch if unclaimed => (S::VerifiedMatch, R::RegisteredNotClaimed),
        IdentityStatus::VerifiedMatch => (S::NotAttached, R::ClaimedNotAttached),
        IdentityStatus::Missing if target == T::Observer && unclaimed => {
            (S::Missing, R::ObserverMissingNotClaimed)
        }
        IdentityStatus::Missing if target == T::Observer => (S::Missing, R::ObserverMissing),
        IdentityStatus::Missing => (S::Missing, R::ProcessMissing),
        IdentityStatus::Mismatch => (S::Mismatch, R::IdentityMismatch),
        IdentityStatus::Indeterminate => (S::Indeterminate, R::IdentityUnavailable),
    };
    job.reason = reason;
    job.observation = IdentityObservation {
        target,
        status,
        observed_at_ms: now,
        reason,
    };
    if target == T::Child && status == S::VerifiedMatch {
        job.phase = Phase::Running;
        job.child_verified_at_ms.get_or_insert(now);
    } else if !unclaimed && status != S::NotAttached {
        job.phase = Phase::OutcomeUnknown;
    }
}
pub(super) const RECENT_SQL: &str = "WITH candidates AS MATERIALIZED (
        SELECT job_id, registered_at_ms, updated_at_ms, reason
        FROM job_lifecycle INDEXED BY idx_jobs_lifecycle_recent
        ORDER BY registered_at_ms DESC, job_id DESC LIMIT 51)
        SELECT j.id, j.state, d.registered_at_ms, d.updated_at_ms, d.reason
        FROM candidates d JOIN jobs j ON j.id=d.job_id
        ORDER BY d.registered_at_ms DESC, d.job_id DESC";
/// Indexed, bounded last-known summaries; no per-row native probes.
pub fn recent(connection: &Connection, now: u64) -> JobResult<StatusResult> {
    valid_now(now, None)?;
    let mut stmt = connection.prepare(RECENT_SQL)?;
    let mut jobs: Vec<JobSummary> = stmt
        .query_map([], |row| {
            let phase: String = row.get(1)?;
            let reason: String = row.get(4)?;
            let decode = || -> Option<JobSummary> {
                Some(JobSummary {
                    job_id: JobId::try_from(row.get::<_, String>(0).ok()?).ok()?,
                    phase: serde_json::from_value(serde_json::Value::String(phase)).ok()?,
                    registered_at_ms: u64::try_from(row.get::<_, i64>(2).ok()?).ok()?,
                    updated_at_ms: u64::try_from(row.get::<_, i64>(3).ok()?).ok()?,
                    reason: serde_json::from_value(serde_json::Value::String(reason)).ok()?,
                })
            };
            decode().ok_or(rusqlite::Error::InvalidQuery)
        })?
        .collect::<Result<_, _>>()?;
    let truncated = jobs.len() > 50;
    jobs.truncate(50);
    if jobs.iter().any(|j| !j.is_valid()) {
        return Err(JobError::StorageFailure);
    }
    Ok(StatusResult::Recent {
        jobs,
        truncated,
        observed_at_ms: now,
        liveness_fresh: false,
    })
}

//! Existing queue-owner reconciliation on the single writer.
use super::*;

/// Stale queue-head cleanup never releases a grant. Probe outside the writer;
/// recheck the complete tuple, revision and queued state inside the transaction.
#[derive(Debug, Clone)]
pub struct QueueObservation {
    id: JobId,
    observer: ProcessIdentity,
    revision: u64,
    status: IdentityStatus,
    observed_at: u64,
}
impl QueueObservation {
    pub fn gather(job: &JobSnapshot, now: u64) -> Self {
        Self {
            id: job.job_id.clone(),
            observer: job.observer.clone(),
            revision: job.revision,
            status: probe_live(&job.observer),
            observed_at: now,
        }
    }
    #[cfg(test)]
    pub(in crate::jobs::traffic) fn simulated(
        job: &JobSnapshot,
        status: IdentityStatus,
        now: u64,
    ) -> Self {
        Self {
            id: job.job_id.clone(),
            observer: job.observer.clone(),
            revision: job.revision,
            status,
            observed_at: now,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconcileResult {
    Abandoned,
    Unchanged,
    Stale,
    NotFound,
}
pub fn reconcile_queued(
    connection: &mut Connection,
    evidence: &QueueObservation,
) -> JobResult<ReconcileResult> {
    valid_now(evidence.observed_at, None)?;
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let Some(e) = store::entry(&tx, &evidence.id)? else {
        return Ok(ReconcileResult::NotFound);
    };
    let job = get(&tx, &evidence.id)?.ok_or(E::StorageFailure)?;
    if job.observer != evidence.observer || job.revision != evidence.revision || e.state != "QUEUED"
    {
        return Ok(ReconcileResult::Stale);
    }
    if !matches!(
        evidence.status,
        IdentityStatus::Missing | IdentityStatus::Mismatch
    ) {
        return Ok(ReconcileResult::Unchanged);
    }
    let now = ordered_now(evidence.observed_at, &job)?;
    connection_state(&tx, &evidence.id, "ABANDONED")?;
    crate::policy::mark_terminal(&tx, &evidence.id, now)?;
    tx.commit()?;
    Ok(ReconcileResult::Abandoned)
}

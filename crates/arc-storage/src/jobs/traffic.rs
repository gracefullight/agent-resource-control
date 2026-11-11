//! Durable Traffic admission, lifecycle and reconciliation.
//!
//! Each mutation holds one SQLite IMMEDIATE transaction. The caller authenticates
//! the peer, gathers native evidence before entering the worker, mints a random
//! context, and is the only component that launches a process. A retry of any
//! operation is keyed by the original job ID and never launches a child.

mod admission;
mod lifecycle;
mod store;
#[cfg(test)]
mod tests;
mod uncertain;
mod witness;

pub use admission::{
    enqueue, lineage_anchors, lineage_snapshot, lineage_snapshots, poll, poll_with_proof,
    LineageProof, LineageSnapshot, OwnerEvidence,
};
pub(super) use lifecycle::reconcile_exited_child;
pub use lifecycle::{
    attach, cancel, complete, confirm_cancel_unspawned, fallback, reconcile_queued, ChildEvidence,
    QueueObservation, ReconcileResult,
};
pub use store::status;
pub use uncertain::{reconcile_uncertain, UncertainEvidence, UncertainResult};
pub use witness::{recover_unspawned, recover_witness, UnspawnedEvidence, WitnessEvidence};

use super::{get, valid_now, JobError, JobResult};
use arc_ipc::jobs::{JobId, JobSnapshot, ProcessIdentity};
use rusqlite::{Connection, OptionalExtension};

/// The oldest unreserved live queue entry, for an outside-the-writer native
/// owner probe followed by `reconcile_queued` on the single writer.
pub fn queue_head(connection: &Connection) -> JobResult<Option<JobSnapshot>> {
    let id: Option<String> = connection
        .query_row(
            "SELECT job_id FROM traffic_entries WHERE state='QUEUED' ORDER BY sequence LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()?;
    id.map(|id| {
        let id = JobId::try_from(id).map_err(|_| JobError::StorageFailure)?;
        get(connection, &id)?.ok_or(JobError::StorageFailure)
    })
    .transpose()
}

/// An attached active lineage member whose native identities can be observed
/// outside the SQLite writer. The sequence is a stable pagination cursor.
#[derive(Debug, Clone)]
pub struct RecoveryCandidate {
    pub sequence: u64,
    pub job: JobSnapshot,
    pub witness_context: Option<arc_ipc::traffic::LineageContext>,
}

/// Read a bounded page for periodic or admission-time recovery. Walk pages by
/// passing the last sequence, then restart at zero on a later pass. Reconcile
/// each snapshot with `jobs::observe`. Apply only observations for which
/// `traffic_recovery_ready()` is true, so scanning live jobs does not churn
/// lifecycle revisions. `jobs::apply_observation` rechecks identity, revision
/// and state in the write transaction. Unattached grants are included even if
/// their witness is missing, so an unknown child can enter the grace policy.
pub fn recovery_candidates(
    connection: &Connection,
    after_sequence: u64,
    limit: usize,
) -> JobResult<Vec<RecoveryCandidate>> {
    const MAX_BATCH: usize = 64;
    if after_sequence > i64::MAX as u64 || !(1..=MAX_BATCH).contains(&limit) {
        return Err(JobError::InvalidRequest);
    }
    let mut statement = connection.prepare(
        "SELECT t.sequence,t.job_id,t.inherited_context FROM traffic_entries t \
         JOIN job_lifecycle l ON l.job_id=t.job_id \
         JOIN permits p ON p.job_id=COALESCE(t.root_job_id,t.job_id) \
           AND p.state='ACTIVE' AND p.generation=t.generation \
         WHERE t.sequence>?1 AND ((t.state IN ('RUNNING','RETAINED') \
           AND l.child_pid IS NOT NULL AND l.child_verified_at_ms IS NOT NULL) \
           OR (t.state IN ('RESERVED','RETAINED') AND l.child_pid IS NULL)) \
         ORDER BY t.sequence LIMIT ?2",
    )?;
    let rows = statement.query_map((after_sequence as i64, limit as i64), |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
        ))
    })?;
    let mut candidates = Vec::new();
    for row in rows {
        let (sequence, id, context) = row?;
        let id = JobId::try_from(id).map_err(|_| JobError::StorageFailure)?;
        candidates.push(RecoveryCandidate {
            sequence: u64::try_from(sequence).map_err(|_| JobError::StorageFailure)?,
            job: get(connection, &id)?.ok_or(JobError::StorageFailure)?,
            witness_context: context
                .map(arc_ipc::traffic::LineageContext::try_from)
                .transpose()
                .map_err(|_| JobError::StorageFailure)?,
        });
    }
    Ok(candidates)
}

/// Last-known committed accounting. Grant decisions always recompute this
/// inside their own transaction rather than using this read snapshot.
pub fn active_reservations(connection: &Connection) -> JobResult<arc_traffic::Active> {
    store::active(connection)
}

pub(super) fn owned(connection: &Connection, job_id: &JobId) -> JobResult<bool> {
    Ok(connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM traffic_entries WHERE job_id=?1 UNION ALL SELECT 1 FROM traffic_tombstones WHERE job_id=?1)",
        [job_id.as_str()], |row| row.get(0),
    )?)
}

fn matching_job(
    connection: &Connection,
    job_id: &JobId,
    observer: &ProcessIdentity,
) -> JobResult<JobSnapshot> {
    let job = get(connection, job_id)?.ok_or(JobError::InvalidTransition)?;
    if job.observer != *observer {
        return Err(JobError::Conflict);
    }
    Ok(job)
}

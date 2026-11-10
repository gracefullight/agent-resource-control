//! Apply an authenticated child witness without replaying a command.
use super::{get, store, valid_now, JobResult};
use crate::jobs::{codec, reconcile};
use arc_core::process_identity::{probe_live, IdentityStatus, ProcessIdentity};
use arc_ipc::{
    jobs::{JobSnapshot, ObservationTarget, Outcome},
    traffic::LineageContext,
};
use rusqlite::{Connection, TransactionBehavior};

#[derive(Debug, Clone)]
pub struct UnspawnedEvidence {
    snapshot: JobSnapshot,
    key: LineageContext,
    owner_status: IdentityStatus,
    observed_at_ms: u64,
    spawn_failure: Option<Outcome>,
}
impl UnspawnedEvidence {
    /// The caller verifies the private witness file and key first.
    pub fn gather(snapshot: &JobSnapshot, key: &LineageContext, now: u64) -> Self {
        Self {
            snapshot: snapshot.clone(),
            key: key.clone(),
            owner_status: probe_live(&snapshot.observer),
            observed_at_ms: now,
            spawn_failure: None,
        }
    }
    pub fn gather_spawn_failure(
        snapshot: &JobSnapshot,
        key: &LineageContext,
        outcome: &Outcome,
        now: u64,
    ) -> Self {
        let mut evidence = Self::gather(snapshot, key, now);
        evidence.spawn_failure = Some(outcome.clone());
        evidence
    }
    #[cfg(test)]
    pub(super) fn simulated(
        snapshot: &JobSnapshot,
        key: &LineageContext,
        owner_status: IdentityStatus,
        now: u64,
    ) -> Self {
        Self {
            snapshot: snapshot.clone(),
            key: key.clone(),
            owner_status,
            observed_at_ms: now,
            spawn_failure: None,
        }
    }
}

/// The owner recorded that it did not start a target. The owner must also be
/// gone, and the authenticated key, revision and unattached grant are checked
/// again under the same transaction as cancellation and permit release.
pub fn recover_unspawned(
    connection: &mut Connection,
    evidence: &UnspawnedEvidence,
) -> JobResult<bool> {
    valid_now(evidence.observed_at_ms, None)?;
    if !matches!(
        evidence.owner_status,
        IdentityStatus::Missing | IdentityStatus::Mismatch
    ) {
        return Ok(false);
    }
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let Some(mut job) = get(&tx, &evidence.snapshot.job_id)? else {
        return Ok(false);
    };
    let Some(entry) = store::entry(&tx, &job.job_id)? else {
        return Ok(false);
    };
    if job.revision != evidence.snapshot.revision
        || job.observer != evidence.snapshot.observer
        || job.child.is_some()
        || job.outcome.is_some()
        || entry.inherited.as_ref() != Some(&evidence.key)
        || !matches!(entry.state.as_str(), "RESERVED" | "RETAINED")
    {
        return Ok(false);
    }
    valid_now(evidence.observed_at_ms, Some(&job))?;
    if let Some(outcome @ Outcome::SpawnFailed { .. }) = &evidence.spawn_failure {
        if !outcome.is_valid() {
            return Err(super::JobError::InvalidRequest);
        }
        super::lifecycle::finish(&tx, &mut job, &entry, outcome, evidence.observed_at_ms)?;
    } else if evidence.spawn_failure.is_some() {
        return Err(super::JobError::InvalidRequest);
    } else {
        super::lifecycle::connection_state(&tx, &job.job_id, "CANCELLED")?;
        super::lifecycle::release_if_done(&tx, &job.job_id, &entry, evidence.observed_at_ms)?;
    }
    tx.commit()?;
    Ok(true)
}

#[derive(Debug, Clone)]
pub struct WitnessEvidence {
    snapshot: JobSnapshot,
    key: LineageContext,
    child: ProcessIdentity,
    owner_status: IdentityStatus,
    child_status: IdentityStatus,
    observed_at_ms: u64,
}
impl WitnessEvidence {
    /// The caller verifies the witness file and key before constructing this.
    pub fn gather(
        snapshot: &JobSnapshot,
        key: &LineageContext,
        child: &ProcessIdentity,
        now: u64,
    ) -> Self {
        Self {
            snapshot: snapshot.clone(),
            key: key.clone(),
            child: child.clone(),
            owner_status: probe_live(&snapshot.observer),
            child_status: probe_live(child),
            observed_at_ms: now,
        }
    }

    #[cfg(test)]
    pub(super) fn simulated(
        snapshot: &JobSnapshot,
        key: &LineageContext,
        child: &ProcessIdentity,
        owner_status: IdentityStatus,
        child_status: IdentityStatus,
        observed_at_ms: u64,
    ) -> Self {
        Self {
            snapshot: snapshot.clone(),
            key: key.clone(),
            child: child.clone(),
            owner_status,
            child_status,
            observed_at_ms,
        }
    }
}

/// Compare the snapshot, key and reservation again under the write lock. A
/// missing/reused PID is definitive termination only with its recorded start
/// token; indeterminate observations retain the reservation.
pub fn recover_witness(connection: &mut Connection, evidence: &WitnessEvidence) -> JobResult<bool> {
    valid_now(evidence.observed_at_ms, None)?;
    if !matches!(
        evidence.owner_status,
        IdentityStatus::Missing | IdentityStatus::Mismatch
    ) || !evidence.child.is_valid()
        || evidence.child.boot_id != evidence.snapshot.observer.boot_id
        || evidence.child_status == IdentityStatus::Indeterminate
    {
        return Ok(false);
    }
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let Some(mut job) = get(&tx, &evidence.snapshot.job_id)? else {
        return Ok(false);
    };
    let Some(entry) = store::entry(&tx, &job.job_id)? else {
        return Ok(false);
    };
    if job.revision != evidence.snapshot.revision
        || job.observer != evidence.snapshot.observer
        || job.child.is_some()
        || job.outcome.is_some()
        || entry.inherited.as_ref() != Some(&evidence.key)
        || !matches!(entry.state.as_str(), "RESERVED" | "RETAINED")
    {
        return Ok(false);
    }
    valid_now(evidence.observed_at_ms, Some(&job))?;
    job.child = Some(evidence.child.clone());
    job.child_verified_at_ms = Some(evidence.observed_at_ms);
    reconcile::update_observation(
        &mut job,
        ObservationTarget::Child,
        evidence.child_status,
        evidence.observed_at_ms,
    );
    job.updated_at_ms = evidence.observed_at_ms;
    job.revision += 1;
    codec::write(&tx, &job, false)?;
    if entry.state == "RESERVED" {
        tx.execute(
            "UPDATE traffic_entries SET state='RUNNING' WHERE job_id=?1",
            [job.job_id.as_str()],
        )?;
    }
    super::reconcile_exited_child(
        &tx,
        &job,
        evidence.owner_status,
        evidence.child_status,
        evidence.observed_at_ms,
    )?;
    tx.commit()?;
    Ok(true)
}

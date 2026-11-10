//! Persist the first dead-owner observation and bounded unknown-child recovery.
use super::{get, store, valid_now, JobResult};
use crate::jobs::{codec, reconcile};
use arc_core::process_identity::{probe_live, IdentityStatus};
use arc_ipc::jobs::{JobSnapshot, ObservationTarget};
use rusqlite::{params, Connection, TransactionBehavior};

#[derive(Debug, Clone)]
pub struct UncertainEvidence {
    snapshot: JobSnapshot,
    owner: IdentityStatus,
    child: Option<IdentityStatus>,
    observed_at_ms: u64,
    grace_ms: u64,
}

impl UncertainEvidence {
    pub fn gather(snapshot: &JobSnapshot, now: u64, grace_ms: u64) -> Self {
        Self {
            snapshot: snapshot.clone(),
            owner: probe_live(&snapshot.observer),
            child: snapshot.child.as_ref().map(probe_live),
            observed_at_ms: now,
            grace_ms,
        }
    }

    #[cfg(test)]
    pub(super) fn simulated(
        snapshot: &JobSnapshot,
        owner: IdentityStatus,
        child: Option<IdentityStatus>,
        now: u64,
        grace_ms: u64,
    ) -> Self {
        Self {
            snapshot: snapshot.clone(),
            owner,
            child,
            observed_at_ms: now,
            grace_ms,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UncertainResult {
    Unchanged,
    Retained,
    Released,
}

/// The audit record is the durable grace clock. The writer rechecks the job
/// revision and reservation before creating an event or releasing a permit.
pub fn reconcile_uncertain(
    connection: &mut Connection,
    evidence: &UncertainEvidence,
) -> JobResult<UncertainResult> {
    valid_now(evidence.observed_at_ms, None)?;
    if !matches!(
        evidence.owner,
        IdentityStatus::Missing | IdentityStatus::Mismatch
    ) || evidence
        .child
        .is_some_and(|status| status != IdentityStatus::Indeterminate)
    {
        return Ok(UncertainResult::Unchanged);
    }
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let Some(mut job) = get(&tx, &evidence.snapshot.job_id)? else {
        return Ok(UncertainResult::Unchanged);
    };
    let Some(entry) = store::entry(&tx, &job.job_id)? else {
        return Ok(UncertainResult::Unchanged);
    };
    if job.revision != evidence.snapshot.revision
        || job.observer != evidence.snapshot.observer
        || job.child != evidence.snapshot.child
        || job.outcome.is_some()
        || !matches!(entry.state.as_str(), "RESERVED" | "RUNNING" | "RETAINED")
    {
        return Ok(UncertainResult::Unchanged);
    }
    valid_now(evidence.observed_at_ms, Some(&job))?;
    let first = format!("traffic-unknown-owner-{}", job.job_id.as_str());
    tx.execute("INSERT OR IGNORE INTO audit_events(id,severity,event_type,entity_id,payload_redacted_json,created_at) VALUES (?1,'WARN','TRAFFIC_UNKNOWN_OWNER',?2,NULL,?3)",
        params![first, job.job_id.as_str(), evidence.observed_at_ms as i64])?;
    let began: i64 = tx.query_row(
        "SELECT created_at FROM audit_events WHERE id=?1",
        [&first],
        |row| row.get(0),
    )?;
    reconcile::update_observation(
        &mut job,
        ObservationTarget::Observer,
        evidence.owner,
        evidence.observed_at_ms,
    );
    job.updated_at_ms = evidence.observed_at_ms;
    job.revision += 1;
    codec::write(&tx, &job, false)?;
    let elapsed = evidence
        .observed_at_ms
        .saturating_sub(u64::try_from(began).unwrap_or(evidence.observed_at_ms));
    let result = if elapsed >= evidence.grace_ms {
        super::lifecycle::connection_state(&tx, &job.job_id, "ABANDONED")?;
        super::lifecycle::release_if_done(&tx, &job.job_id, &entry, evidence.observed_at_ms)?;
        let expired = format!("traffic-unknown-expired-{}", job.job_id.as_str());
        tx.execute("INSERT OR IGNORE INTO audit_events(id,severity,event_type,entity_id,payload_redacted_json,created_at) VALUES (?1,'WARN','TRAFFIC_GRACE_EXPIRED',?2,NULL,?3)",
            params![expired, job.job_id.as_str(), evidence.observed_at_ms as i64])?;
        UncertainResult::Released
    } else {
        UncertainResult::Retained
    };
    tx.commit()?;
    Ok(result)
}

//! Durable ownership, lease and decision records for the bounded process Reaper.
//!
//! Native evidence is gathered by the caller. This module never signals: it
//! records what was observed and fences each claim in its own transaction.
use crate::jobs::{self, JobError, JobResult};
use arc_core::process_identity::{random_job_id, ProcessIdentity};
use arc_ipc::{
    jobs::{JobId, JobSnapshot, MAX_TIMESTAMP},
    reaper::ChildEvidence,
};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};

pub const RESOURCE_TYPE: &str = "MANAGED_PROCESS";
pub const GC_REASON: &str = "ORPHAN_PROCESS";
pub const AUDIT_EVENT: &str = "reaper.decision";
pub const LEVEL_PROVEN: &str = "PROVEN";
pub const LEVEL_UNKNOWN: &str = "UNKNOWN";
pub const STATE_TRACKED: &str = "TRACKED";
pub const STATE_TERMINATING: &str = "TERMINATING";
pub const STATE_PROTECTED: &str = "PROTECTED";
const LEASE_ACTIVE: &str = "ACTIVE";
const LEASE_ENDED: &str = "ENDED";

/// SRS 12.5 pipeline states, plus the durable claim that precedes a signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionState {
    Blocked,
    Skipped,
    RetryLater,
    Terminating,
    Reclaimed,
    PartialFailure,
}
impl DecisionState {
    pub fn code(self) -> &'static str {
        match self {
            Self::Blocked => "BLOCKED",
            Self::Skipped => "SKIPPED",
            Self::RetryLater => "RETRY_LATER",
            Self::Terminating => STATE_TERMINATING,
            Self::Reclaimed => "RECLAIMED",
            Self::PartialFailure => "PARTIAL_FAILURE",
        }
    }
}

/// A registration the daemon has already verified against native evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registration {
    pub job_id: JobId,
    pub observer: ProcessIdentity,
    pub child: ChildEvidence,
    pub session: ProcessIdentity,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resource {
    pub ownership_level: String,
    pub state: String,
    pub protected: bool,
    pub session: Option<ProcessIdentity>,
    /// Set only after an ended owner session was observed and recorded.
    pub session_ended_at_ms: Option<u64>,
    /// Unexpired ACTIVE leases, including the owner session's own lease.
    pub active_references: u64,
}

/// A running managed job. `resource` is absent for unknown ownership.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub job: JobSnapshot,
    pub resource: Option<Resource>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Decision {
    pub job_id: JobId,
    pub state: DecisionState,
    pub reason: &'static str,
    pub observed_at_ms: u64,
    /// Redacted evidence summary. No executable path, argument or environment.
    pub evidence: serde_json::Value,
    pub tracking: Tracking,
}

/// What a decision does to the tracked resource besides appending its record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tracking {
    Keep,
    /// The job finished, or can never be acted on: stop tracking it.
    Release,
    /// The preceding claim sent no signal: the resource is tracked again.
    Withdraw,
}

fn resource_id(job_id: &JobId) -> String {
    format!("process:{}", job_id.as_str())
}
fn lease_id(job_id: &JobId) -> String {
    format!("session:{}", job_id.as_str())
}
fn timestamp(now: u64) -> JobResult<i64> {
    if now > MAX_TIMESTAMP {
        return Err(JobError::InvalidRequest);
    }
    Ok(now as i64)
}
fn random_id() -> JobResult<String> {
    random_job_id().map_err(|_| JobError::StorageFailure)
}
fn file_identity(child: &ChildEvidence) -> String {
    format!("{}:{}", child.executable_device, child.executable_inode)
}

/// Bind a managed job's attached child to its owner session. The job row must
/// already name this observer and child and must still be running.
pub fn register(
    connection: &mut Connection,
    registration: &Registration,
    now: u64,
) -> JobResult<()> {
    let at = timestamp(now)?;
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let job = jobs::get(&tx, &registration.job_id)?.ok_or(JobError::InvalidRequest)?;
    if job.outcome.is_some()
        || job.observer != registration.observer
        || job.child.as_ref() != Some(&registration.child.process)
    {
        return Err(JobError::InvalidRequest);
    }
    let id = resource_id(&registration.job_id);
    let session =
        serde_json::to_string(&registration.session).map_err(|_| JobError::StorageFailure)?;
    let identity = file_identity(&registration.child);
    let existing: Option<(String, Option<String>, Option<String>)> = tx
        .query_row(
            "SELECT ownership_level,owner_session_id,canonical_file_identity FROM resource_objects WHERE id=?1",
            [&id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    match existing {
        Some((level, owner, file)) if level == LEVEL_PROVEN => {
            // A retried registration is idempotent; different evidence is not.
            return if owner.as_deref() == Some(&session) && file.as_deref() == Some(&identity) {
                Ok(())
            } else {
                Err(JobError::Conflict)
            };
        }
        // An earlier pass inventoried this job before its registration arrived.
        Some(_) => {
            tx.execute(
                "UPDATE resource_objects SET canonical_path=?2,canonical_file_identity=?3,owner_session_id=?4,ownership_level=?5,last_active_at=?6,state=?7 WHERE id=?1",
                params![id, registration.child.executable, identity, session, LEVEL_PROVEN, at, STATE_TRACKED],
            )?;
        }
        None => {
            tx.execute(
                "INSERT INTO resource_objects(id,resource_type,resource_identity,canonical_path,canonical_file_identity,owner_session_id,ownership_level,created_at,last_active_at,protected,state) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?8,0,?9)",
                params![id, RESOURCE_TYPE, registration.job_id.as_str(), registration.child.executable, identity, session, LEVEL_PROVEN, at, STATE_TRACKED],
            )?;
        }
    }
    tx.execute(
        "INSERT INTO resource_leases(id,resource_id,process_identity,acquired_at,last_heartbeat_at,state) VALUES(?1,?2,?3,?4,?4,?5)",
        params![lease_id(&registration.job_id), id, session, at, LEASE_ACTIVE],
    )?;
    tx.commit()?;
    Ok(())
}

/// Record that the owner session was observed ended. The first observation
/// time is retained; the returned value is that time.
pub fn end_session(connection: &mut Connection, job_id: &JobId, now: u64) -> JobResult<u64> {
    let at = timestamp(now)?;
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let lease = lease_id(job_id);
    tx.execute(
        "UPDATE resource_leases SET state=?2,expires_at=?3 WHERE id=?1 AND state=?4",
        params![lease, LEASE_ENDED, at, LEASE_ACTIVE],
    )?;
    let ended: Option<i64> = tx
        .query_row(
            "SELECT expires_at FROM resource_leases WHERE id=?1 AND state=?2",
            params![lease, LEASE_ENDED],
            |row| row.get(0),
        )
        .optional()?
        .flatten();
    tx.commit()?;
    ended
        .and_then(|value| u64::try_from(value).ok())
        .ok_or(JobError::InvalidTransition)
}

fn resource(tx: &Connection, job_id: &JobId, now: i64) -> JobResult<Option<Resource>> {
    let id = resource_id(job_id);
    let row: Option<(String, String, i64, Option<String>)> = tx
        .query_row(
            "SELECT ownership_level,state,protected,owner_session_id FROM resource_objects WHERE id=?1 AND resource_type=?2",
            params![id, RESOURCE_TYPE],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?;
    let Some((ownership_level, state, protected, session)) = row else {
        return Ok(None);
    };
    let session_ended_at_ms: Option<i64> = tx
        .query_row(
            "SELECT expires_at FROM resource_leases WHERE id=?1 AND state=?2",
            params![lease_id(job_id), LEASE_ENDED],
            |row| row.get(0),
        )
        .optional()?
        .flatten();
    let active_references: i64 = tx.query_row(
        "SELECT count(*) FROM resource_leases WHERE resource_id=?1 AND state=?2 AND (expires_at IS NULL OR expires_at>?3)",
        params![id, LEASE_ACTIVE, now],
        |row| row.get(0),
    )?;
    Ok(Some(Resource {
        ownership_level,
        state,
        // Any value other than an explicit zero is treated as protected.
        protected: protected != 0,
        session: session.and_then(|value| serde_json::from_str(&value).ok()),
        session_ended_at_ms: session_ended_at_ms.and_then(|value| u64::try_from(value).ok()),
        active_references: active_references as u64,
    }))
}

/// One bounded pass input: every tracked or terminating resource (oldest
/// first), then running jobs that have no ownership record at all.
pub fn inventory(connection: &Connection, now: u64, limit: usize) -> JobResult<Vec<Candidate>> {
    let at = timestamp(now)?;
    let limit = i64::try_from(limit).map_err(|_| JobError::InvalidRequest)?;
    let mut ids = Vec::new();
    for sql in [
        "SELECT resource_identity FROM resource_objects WHERE resource_type=?1 AND state IN ('TRACKED','TERMINATING') ORDER BY created_at,id LIMIT ?2",
        "SELECT l.job_id FROM job_lifecycle l WHERE l.child_pid IS NOT NULL AND l.outcome_kind IS NULL AND NOT EXISTS(SELECT 1 FROM resource_objects r WHERE r.resource_type=?1 AND r.resource_identity=l.job_id) ORDER BY l.registered_at_ms DESC,l.job_id DESC LIMIT ?2",
    ] {
        let mut statement = connection.prepare(sql)?;
        let rows = statement.query_map(params![RESOURCE_TYPE, limit], |row| row.get::<_, String>(0))?;
        for row in rows {
            ids.push(JobId::try_from(row?).map_err(|_| JobError::StorageFailure)?);
        }
    }
    let mut candidates = Vec::with_capacity(ids.len());
    for job_id in ids {
        // A resource without its lifecycle row cannot be evaluated or signalled.
        if let Some(job) = jobs::get(connection, &job_id)? {
            let resource = resource(connection, &job_id, at)?;
            candidates.push(Candidate { job, resource });
        }
    }
    Ok(candidates)
}

fn fence(tx: &Transaction<'_>, decision: &Decision, at: i64) -> JobResult<()> {
    let job = jobs::get(tx, &decision.job_id)?.ok_or(JobError::InvalidTransition)?;
    let current = resource(tx, &decision.job_id, at)?.ok_or(JobError::InvalidTransition)?;
    let allowed = match decision.state {
        // The claim that precedes a signal: ownership, protection, the lease
        // fence and the running job are re-read in the claiming transaction.
        DecisionState::Terminating => {
            current.ownership_level == LEVEL_PROVEN
                && !current.protected
                && current.active_references == 0
                && current.session_ended_at_ms.is_some()
                && matches!(current.state.as_str(), STATE_TRACKED | STATE_TERMINATING)
                && job.outcome.is_none()
        }
        // Only the wrapper's consuming wait records an outcome.
        DecisionState::Reclaimed => current.state == STATE_TERMINATING && job.outcome.is_some(),
        DecisionState::PartialFailure => current.state == STATE_TERMINATING,
        _ => true,
    };
    allowed.then_some(()).ok_or(JobError::InvalidTransition)
}

/// Append one decision with its reason and observation time. Returns false,
/// writing nothing, when it repeats the resource's most recent decision.
pub fn decide(connection: &mut Connection, decision: &Decision) -> JobResult<bool> {
    let at = timestamp(decision.observed_at_ms)?;
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let id = resource_id(&decision.job_id);
    let known: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM resource_objects WHERE id=?1)",
        [&id],
        |row| row.get(0),
    )?;
    if !known {
        if jobs::get(&tx, &decision.job_id)?.is_none() {
            return Err(JobError::InvalidRequest);
        }
        // Inventory only: unknown ownership is protected and never a candidate.
        tx.execute(
            "INSERT INTO resource_objects(id,resource_type,resource_identity,ownership_level,created_at,protected,state) VALUES(?1,?2,?3,?4,?5,0,?6)",
            params![id, RESOURCE_TYPE, decision.job_id.as_str(), LEVEL_UNKNOWN, at, STATE_PROTECTED],
        )?;
    }
    fence(&tx, decision, at)?;
    let latest: Option<(String, Option<String>)> = tx
        .query_row(
            "SELECT state,error_code FROM gc_operations WHERE resource_id=?1 ORDER BY rowid DESC LIMIT 1",
            [&id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    if latest.is_some_and(|(state, reason)| {
        state == decision.state.code() && reason.as_deref() == Some(decision.reason)
    }) {
        return Ok(false);
    }
    let level: String = tx.query_row(
        "SELECT ownership_level FROM resource_objects WHERE id=?1",
        [&id],
        |row| row.get(0),
    )?;
    let ended = (decision.state != DecisionState::Terminating).then_some(at);
    tx.execute(
        "INSERT INTO gc_operations(id,resource_id,gc_reason,ownership_evidence,state,started_at,ended_at,error_code) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
        params![random_id()?, id, GC_REASON, decision.evidence.to_string(), decision.state.code(), at, ended, decision.reason],
    )?;
    let severity = match decision.state {
        DecisionState::Blocked | DecisionState::PartialFailure => "WARN",
        _ => "INFO",
    };
    let payload = serde_json::json!({
        "job_id": decision.job_id.as_str(),
        "state": decision.state.code(),
        "reason": decision.reason,
        "observed_at_ms": decision.observed_at_ms,
        "ownership_level": level,
    });
    tx.execute(
        "INSERT INTO audit_events(id,severity,event_type,entity_id,payload_redacted_json,created_at) VALUES(?1,?2,?3,?4,?5,?6)",
        params![random_id()?, severity, AUDIT_EVENT, id, payload.to_string(), at],
    )?;
    let next = match (decision.state, decision.tracking) {
        (DecisionState::Terminating, _) => Some(STATE_TERMINATING),
        (DecisionState::Reclaimed, _) => Some("RECLAIMED"),
        (DecisionState::PartialFailure, _) => Some("UNCONFIRMED"),
        (_, Tracking::Release) => Some("RELEASED"),
        (_, Tracking::Withdraw) => Some(STATE_TRACKED),
        (_, Tracking::Keep) => None,
    };
    if let Some(next) = next {
        // Only a claim can be withdrawn; every other state is left as it is.
        let from = (decision.tracking == Tracking::Withdraw && next == STATE_TRACKED)
            .then_some(STATE_TERMINATING);
        tx.execute(
            "UPDATE resource_objects SET state=?2 WHERE id=?1 AND (?3 IS NULL OR state=?3)",
            params![id, next, from],
        )?;
    }
    tx.commit()?;
    Ok(true)
}

#[cfg(test)]
#[path = "reaper/tests.rs"]
mod tests;

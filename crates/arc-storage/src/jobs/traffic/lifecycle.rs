use super::{
    admission::OwnerEvidence, get, matching_job, ordered_now, store, valid_now, JobError as E,
    JobResult,
};
use crate::jobs::{codec, reconcile};
use arc_core::process_identity::{probe, probe_live, IdentityStatus};
use arc_ipc::{
    jobs::{
        JobId, JobSnapshot, LifecycleReason as R, ObservationTarget as T, Outcome, Phase,
        ProcessIdentity,
    },
    traffic::{JobRequest, JobResult as Reply, WaitReason},
};
use rusqlite::{params, Connection, TransactionBehavior};

/// Native identity observation made outside the single SQLite writer.
#[derive(Debug, Clone)]
pub struct ChildEvidence {
    identity: ProcessIdentity,
    identity_status: IdentityStatus,
    live_status: IdentityStatus,
    meter: Option<Box<crate::job_meter::LineageEvidence>>,
}
impl ChildEvidence {
    pub fn with_meter_evidence(mut self, evidence: crate::job_meter::LineageEvidence) -> Self {
        self.meter = Some(Box::new(evidence));
        self
    }
    pub fn gather(identity: &ProcessIdentity) -> Self {
        Self {
            meter: None,
            identity: identity.clone(),
            identity_status: probe(identity),
            live_status: probe_live(identity),
        }
    }
    #[cfg(test)]
    pub(super) fn simulated(identity: &ProcessIdentity, status: IdentityStatus) -> Self {
        Self {
            meter: None,
            identity: identity.clone(),
            identity_status: status,
            live_status: status,
        }
    }
}

/// Attach the one child spawned by the CLI. Replays can never replace its
/// pid/start/boot tuple, even if the operating system later reuses that pid.
pub fn attach(
    connection: &mut Connection,
    request: &JobRequest,
    evidence: &ChildEvidence,
    now: u64,
) -> JobResult<Reply> {
    valid_now(now, None)?;
    if !request.is_valid() {
        return Err(E::InvalidRequest);
    }
    let JobRequest::Attach {
        job_id,
        observer,
        child,
        permit_generation,
    } = request
    else {
        return Err(E::InvalidRequest);
    };
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let mut job = matching_job(&tx, job_id, observer)?;
    let e = store::entry(&tx, job_id)?.ok_or(E::InvalidTransition)?;
    check_generation(&e, Some(*permit_generation))?;
    attach_inner(&tx, &mut job, &e, child, evidence, now)?;
    let e = store::entry(&tx, job_id)?.ok_or(E::StorageFailure)?;
    let reply = store::result(&tx, job_id, &job, &e, WaitReason::Fifo)?;
    tx.commit()?;
    Ok(reply)
}

fn attach_inner(
    connection: &Connection,
    job: &mut JobSnapshot,
    e: &store::Entry,
    child: &ProcessIdentity,
    evidence: &ChildEvidence,
    now: u64,
) -> JobResult<()> {
    let now = ordered_now(now, job)?;
    if let Some(prior) = &job.child {
        return if prior == child {
            Ok(())
        } else {
            Err(E::Conflict)
        };
    }
    if !matches!(e.state.as_str(), "RESERVED" | "RETAINED") || job.outcome.is_some() {
        return Err(E::InvalidTransition);
    }
    if evidence.identity != *child {
        return Err(E::InvalidRequest);
    }
    if evidence.identity_status != IdentityStatus::VerifiedMatch {
        return Err(E::IdentityUnavailable);
    }
    valid_now(now, Some(job))?;
    job.child = Some(child.clone());
    job.child_verified_at_ms = Some(now);
    reconcile::update_observation(job, T::Child, evidence.live_status, now);
    job.updated_at_ms = now;
    job.revision += 1;
    codec::write(connection, job, false)?;
    if let Some(proof) = &evidence.meter {
        crate::job_meter::bind(connection, job, proof)?;
    }
    if e.state == "RESERVED" {
        connection.execute(
            "UPDATE traffic_entries SET state='RUNNING' WHERE job_id=?1",
            [job.job_id.as_str()],
        )?;
    }
    Ok(())
}

/// Complete and release the corresponding reservation in the same transaction.
/// A parent's permit remains active while any verified reused descendant remains.
pub fn complete(connection: &mut Connection, request: &JobRequest, now: u64) -> JobResult<Reply> {
    valid_now(now, None)?;
    if !request.is_valid() {
        return Err(E::InvalidRequest);
    }
    let JobRequest::Complete {
        job_id,
        observer,
        outcome,
        permit_generation,
    } = request
    else {
        return Err(E::InvalidRequest);
    };
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let mut job = matching_job(&tx, job_id, observer)?;
    let e = store::entry(&tx, job_id)?.ok_or(E::InvalidTransition)?;
    check_generation(&e, Some(*permit_generation))?;
    finish(&tx, &mut job, &e, outcome, now)?;
    let e = store::entry(&tx, job_id)?.ok_or(E::StorageFailure)?;
    let reply = store::result(&tx, job_id, &job, &e, WaitReason::Fifo)?;
    tx.commit()?;
    Ok(reply)
}

pub(super) fn finish(
    connection: &Connection,
    job: &mut JobSnapshot,
    e: &store::Entry,
    outcome: &Outcome,
    now: u64,
) -> JobResult<()> {
    let now = ordered_now(now, job)?;
    if let Some(prior) = &job.outcome {
        return if prior == outcome {
            Ok(())
        } else {
            Err(E::Conflict)
        };
    }
    if !(matches!(e.state.as_str(), "RESERVED" | "RUNNING" | "RETAINED")
        || recovered_unknown(job, e))
        || job.launch_claimed_at_ms.is_none()
        || matches!(outcome, Outcome::SpawnFailed { .. }) && job.child.is_some()
    {
        return Err(E::InvalidTransition);
    }
    valid_now(now, Some(job))?;
    (job.phase, job.reason) = match outcome {
        Outcome::Exited { .. } => (Phase::Completed, R::ObservedExit),
        Outcome::Signaled { .. } => (Phase::Signaled, R::ObservedSignal),
        Outcome::SpawnFailed { .. } => (Phase::SpawnFailed, R::ObservedSpawnFailure),
    };
    job.outcome = Some(outcome.clone());
    job.completed_at_ms = Some(now);
    job.updated_at_ms = now;
    job.revision += 1;
    job.observation.reason = job.reason;
    codec::write(connection, job, false)?;
    connection.execute(
        "UPDATE traffic_entries SET state='COMPLETED' WHERE job_id=?1",
        [job.job_id.as_str()],
    )?;
    release_if_done(connection, &job.job_id, e, now)
}

fn recovered_unknown(job: &JobSnapshot, entry: &store::Entry) -> bool {
    entry.state == "ABANDONED"
        && job.phase == Phase::OutcomeUnknown
        && job.child.is_some()
        && job.child_verified_at_ms.is_some()
        && job.outcome.is_none()
}

/// Cancellation with a generation asserts that the authenticated owner has
/// confirmed no spawn. An unacknowledged grant or running child stays reserved.
pub fn cancel(connection: &mut Connection, request: &JobRequest, now: u64) -> JobResult<Reply> {
    cancel_inner(
        connection,
        request,
        false,
        now,
        &arc_policy::evaluator::MissingObservation,
    )
}

/// Confirm that the authenticated owner did not spawn a child, even when the
/// grant reply and generation were permanently lost. The caller must bind the
/// confirmation to the verified socket peer and serialize it with spawning;
/// this method cannot infer no-spawn from an ordinary unknown-generation cancel.
pub fn confirm_cancel_unspawned(
    connection: &mut Connection,
    request: &JobRequest,
    owner: &OwnerEvidence,
    now: u64,
) -> JobResult<Reply> {
    let JobRequest::Cancel { observer, .. } = request else {
        return Err(E::InvalidRequest);
    };
    owner.verify(observer)?;
    cancel_inner(
        connection,
        request,
        true,
        now,
        &arc_policy::evaluator::MissingObservation,
    )
}

pub fn cancel_observed(
    connection: &mut Connection,
    request: &JobRequest,
    owner: &OwnerEvidence,
    now: u64,
    provider: &dyn arc_policy::ObservationProvider,
) -> JobResult<Reply> {
    let JobRequest::Cancel { observer, .. } = request else {
        return Err(E::InvalidRequest);
    };
    owner.verify(observer)?;
    cancel_inner(connection, request, true, now, provider)
}

fn cancel_inner(
    connection: &mut Connection,
    request: &JobRequest,
    confirmed_unspawned: bool,
    now: u64,
    provider: &dyn arc_policy::ObservationProvider,
) -> JobResult<Reply> {
    valid_now(now, None)?;
    if !request.is_valid() {
        return Err(E::InvalidRequest);
    }
    let JobRequest::Cancel {
        job_id,
        observer,
        permit_generation,
    } = request
    else {
        return Err(E::InvalidRequest);
    };
    crate::policy::transaction(connection, provider, |tx, effective| {
        let Some(e) = store::entry(tx, job_id)? else {
            if permit_generation.is_some() {
                return Err(E::Conflict);
            }
            let reply = store::put_tombstone(tx, job_id, observer, "CANCELLED", now)?;
            return Ok(reply);
        };
        let job = matching_job(tx, job_id, observer)?;
        let now = ordered_now(now, &job)?;
        check_generation(&e, *permit_generation)?;
        match e.state.as_str() {
            "QUEUED" => {
                valid_now(now, Some(&job))?;
                connection_state(tx, job_id, "CANCELLED")?;
            }
            "RESERVED" | "RETAINED"
                if (permit_generation.is_some() || confirmed_unspawned) && job.child.is_none() =>
            {
                valid_now(now, Some(&job))?;
                connection_state(tx, job_id, "CANCELLED")?;
                release_if_done(tx, job_id, &e, now)?;
            }
            "RESERVED" => {
                valid_now(now, Some(&job))?;
                connection_state(tx, job_id, "RETAINED")?;
            }
            "RUNNING" | "RETAINED" | "COMPLETED" | "CANCELLED" | "ABANDONED" => (),
            _ => return Err(E::StorageFailure),
        }
        let updated = store::entry(tx, job_id)?.ok_or(E::StorageFailure)?;
        let reply = if matches!(updated.state.as_str(), "RUNNING" | "RETAINED") {
            Reply::ReservationRetained {}
        } else {
            store::result(tx, job_id, &job, &updated, WaitReason::Fifo)?
        };
        if updated.state == "CANCELLED" {
            crate::policy::mark_terminal(tx, job_id, now)?;
            crate::policy::record(
                tx,
                job_id,
                effective,
                arc_policy::DecisionKind::Cancel,
                arc_policy::DecisionReason::Cancelled,
                updated.generation,
                now,
            )?;
        }
        Ok(reply)
    })
}

/// Record fail-open. Missing or queued jobs become owner-bound tombstones;
/// uncertain committed grants remain held until an observed outcome is reported.
pub fn fallback(
    connection: &mut Connection,
    request: &JobRequest,
    evidence: Option<&ChildEvidence>,
    now: u64,
) -> JobResult<Reply> {
    valid_now(now, None)?;
    if !request.is_valid() {
        return Err(E::InvalidRequest);
    }
    let JobRequest::Fallback {
        job_id,
        observer,
        permit_generation,
        child,
        outcome,
    } = request
    else {
        return Err(E::InvalidRequest);
    };
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let Some(e) = store::entry(&tx, job_id)? else {
        let reply = store::put_tombstone(&tx, job_id, observer, "ABANDONED", now)?;
        tx.commit()?;
        return Ok(reply);
    };
    let mut job = matching_job(&tx, job_id, observer)?;
    let now = ordered_now(now, &job)?;
    check_generation(&e, *permit_generation)?;
    if e.state == "QUEUED" {
        valid_now(now, Some(&job))?;
        connection_state(&tx, job_id, "ABANDONED")?;
        crate::policy::mark_terminal(&tx, job_id, now)?;
    } else if matches!(e.state.as_str(), "RESERVED" | "RUNNING" | "RETAINED") {
        if let Some(child) = child {
            attach_inner(
                &tx,
                &mut job,
                &e,
                child,
                evidence.ok_or(E::IdentityUnavailable)?,
                now,
            )?;
        }
        let current = store::entry(&tx, job_id)?.ok_or(E::StorageFailure)?;
        if let Some(outcome) = outcome {
            finish(&tx, &mut job, &current, outcome, now)?;
        } else {
            valid_now(now, Some(&job))?;
            connection_state(&tx, job_id, "RETAINED")?;
        }
    } else {
        if let Some(child) = child {
            if job.child.as_ref().is_some_and(|prior| prior != child)
                || matches!(job.outcome, Some(Outcome::SpawnFailed { .. }))
                || e.state == "CANCELLED"
            {
                return Err(E::Conflict);
            }
        }
        if let Some(outcome) = outcome {
            if recovered_unknown(&job, &e) {
                // A request authenticated before the owner exited may arrive
                // after recovery. Record its observed outcome without reviving
                // the old reservation or touching a later job's permit.
                finish(&tx, &mut job, &e, outcome, now)?;
            } else if job.outcome.as_ref().is_some_and(|prior| prior != outcome) {
                return Err(E::Conflict);
            }
        }
    }
    let updated = store::entry(&tx, job_id)?.ok_or(E::StorageFailure)?;
    let reply = store::result(&tx, job_id, &job, &updated, WaitReason::Fifo)?;
    tx.commit()?;
    Ok(reply)
}

fn check_generation(entry: &store::Entry, generation: Option<u64>) -> JobResult<()> {
    if generation.is_some() && entry.generation != generation {
        return Err(E::Conflict);
    }
    Ok(())
}
pub(super) fn connection_state(connection: &Connection, id: &JobId, state: &str) -> JobResult<()> {
    connection.execute(
        "UPDATE traffic_entries SET state=?2 WHERE job_id=?1",
        (id.as_str(), state),
    )?;
    Ok(())
}
pub(super) fn release_if_done(
    connection: &Connection,
    id: &JobId,
    entry: &store::Entry,
    now: u64,
) -> JobResult<()> {
    if store::entry(connection, id)?.is_some_and(|entry| {
        matches!(
            entry.state.as_str(),
            "COMPLETED" | "CANCELLED" | "ABANDONED"
        )
    }) {
        crate::policy::mark_terminal(connection, id, now)?;
    }
    let root = entry.root.as_ref().unwrap_or(id);
    let active_children: bool = connection.query_row("SELECT EXISTS(SELECT 1 FROM traffic_entries WHERE root_job_id=?1 AND state IN ('RESERVED','RUNNING','RETAINED'))", [root.as_str()], |r| r.get(0))?;
    let root_state: String = connection.query_row(
        "SELECT state FROM traffic_entries WHERE job_id=?1",
        [root.as_str()],
        |r| r.get(0),
    )?;
    if !active_children && matches!(root_state.as_str(), "COMPLETED" | "CANCELLED" | "ABANDONED") {
        connection.execute(
            "UPDATE permits SET state='RELEASED',released_at=?2 WHERE job_id=?1 AND state='ACTIVE'",
            params![root.as_str(), now as i64],
        )?;
    }
    Ok(())
}

/// Called by lifecycle reconciliation in its existing IMMEDIATE transaction.
/// An attached, verified child and its owner must both be known to have gone;
/// a missing attachment, live process or uncertain probe cannot release a grant.
/// The lifecycle row records OUTCOME_UNKNOWN, never a fabricated exit outcome.
pub(in crate::jobs) fn reconcile_exited_child(
    connection: &Connection,
    job: &JobSnapshot,
    owner_status: IdentityStatus,
    child_status: IdentityStatus,
    now: u64,
) -> JobResult<bool> {
    let gone = |status| matches!(status, IdentityStatus::Missing | IdentityStatus::Mismatch);
    if !gone(owner_status)
        || !gone(child_status)
        || job.child.is_none()
        || job.child_verified_at_ms.is_none()
        || job.outcome.is_some()
    {
        return Ok(false);
    }
    let Some(entry) = store::entry(connection, &job.job_id)? else {
        return Ok(false);
    };
    if !matches!(entry.state.as_str(), "RUNNING" | "RETAINED") {
        return Ok(false);
    }
    connection_state(connection, &job.job_id, "ABANDONED")?;
    release_if_done(connection, &job.job_id, &entry, now)?;
    Ok(true)
}

#[path = "queue_recovery.rs"]
mod queue_recovery;
pub use queue_recovery::{reconcile_queued, QueueObservation, ReconcileResult};

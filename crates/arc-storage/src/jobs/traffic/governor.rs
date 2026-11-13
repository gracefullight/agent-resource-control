//! Negotiated disposition metadata; lifecycle, FIFO and permits remain existing
//! authorities. No process probes, IPC, native spawn or independent job queue.
use super::{admission, get, lifecycle, matching_job, store, JobError as E, JobResult};
use crate::jobs::codec;
use arc_ipc::{
    governor::{GovernorJobResult as Reply, GovernorLaunchResult as Launch, GovernorSwitchState},
    jobs::{JobId, Phase, ProcessIdentity},
    traffic::{JobRequest, JobResult as Legacy, LineageContext},
};
use rusqlite::{params, Connection, OptionalExtension};

#[derive(Debug)]
pub struct Disposition {
    pub unreserved: bool,
    pub revision: u64,
    pub ready_at: Option<u64>,
    pub release_id: Option<String>,
    pub previous_generation: Option<u64>,
}
pub fn disposition(connection: &Connection, id: &JobId) -> JobResult<Option<Disposition>> {
    Ok(connection.query_row("SELECT mode,switch_revision,unreserved_at_ms,release_id,previous_generation FROM governor_jobs WHERE job_id=?1",[id.as_str()],|r|Ok(Disposition { unreserved:r.get::<_,String>(0)?=="UNRESERVED",revision:r.get::<_,i64>(1)? as u64,ready_at:r.get::<_,Option<i64>>(2)?.map(|n|n as u64),release_id:r.get(3)?,previous_generation:r.get::<_,Option<i64>>(4)?.map(|n|n as u64) })).optional()?)
}

/// Reconcile only jobs with durable proof of no prior native release. A missing
/// reserved marker proves unspawned only on the negotiated launch protocol.
pub fn prepare(connection: &Connection, id: &JobId, now: u64) -> JobResult<bool> {
    if connection.is_autocommit() {
        return Err(E::InvalidTransition);
    }
    let Some(mode) = disposition(connection, id)? else {
        return Ok(false);
    };
    let job = get(connection, id)?.ok_or(E::StorageFailure)?;
    let entry = store::entry(connection, id)?.ok_or(E::StorageFailure)?;
    if job.child.is_some()
        || job.outcome.is_some()
        || !matches!(entry.state.as_str(), "QUEUED" | "RESERVED")
    {
        return Ok(false);
    }
    let switch = crate::governor::get_switch(connection)?;
    if mode.unreserved {
        if switch.enabled && mode.release_id.is_none() {
            connection.execute("UPDATE governor_jobs SET mode='SCHEDULED',switch_revision=?2,unreserved_at_ms=NULL,previous_generation=NULL WHERE job_id=?1",params![id.as_str(),switch.revision as i64])?;
            reset_queue(connection, &job, now)?;
            return Ok(false);
        }
        return Ok(true);
    }
    if switch.enabled || entry.root.is_some() {
        return Ok(false);
    }
    let unsafe_release:bool=connection.query_row("SELECT EXISTS(SELECT 1 FROM audit_events WHERE id=?1 AND event_type='TRAFFIC_LAUNCH_FENCE') OR EXISTS(SELECT 1 FROM traffic_entries WHERE root_job_id=?2 AND state IN ('RESERVED','RUNNING','RETAINED'))",params![format!("traffic-launch-{}",id.as_str()),id.as_str()],|r|r.get(0))?;
    if unsafe_release {
        return Ok(false);
    }
    let at = now.max(job.updated_at_ms).max(entry.queued_at);
    connection.execute(
        "UPDATE permits SET state='RELEASED',released_at=?2 WHERE job_id=?1 AND state='ACTIVE'",
        params![id.as_str(), at as i64],
    )?;
    connection.execute("UPDATE governor_jobs SET mode='UNRESERVED',switch_revision=?2,unreserved_at_ms=?3,previous_generation=?4 WHERE job_id=?1 AND mode='SCHEDULED'",params![id.as_str(),switch.revision as i64,at as i64,entry.generation.map(|n|n as i64)])?;
    reset_queue(connection, &job, at)?;
    Ok(true)
}
fn reset_queue(
    connection: &Connection,
    job: &arc_ipc::jobs::JobSnapshot,
    now: u64,
) -> JobResult<()> {
    connection.execute("UPDATE traffic_entries SET state='QUEUED',granted_at_ms=NULL,generation=NULL,context=NULL,root_job_id=NULL WHERE job_id=?1",[job.job_id.as_str()])?;
    connection.execute(
        "UPDATE jobs SET enforcement='SCHEDULED',reserved_at=NULL WHERE id=?1",
        [job.job_id.as_str()],
    )?;
    if job.phase != Phase::Registered || job.launch_claimed_at_ms.is_some() {
        let mut job = job.clone();
        job.phase = Phase::Registered;
        job.launch_claimed_at_ms = None;
        job.reason = arc_ipc::jobs::LifecycleReason::RegisteredNotClaimed;
        job.observation = arc_ipc::jobs::IdentityObservation {
            target: arc_ipc::jobs::ObservationTarget::Observer,
            status: arc_ipc::jobs::ObservationStatus::VerifiedMatch,
            observed_at_ms: now.max(job.updated_at_ms),
            reason: job.reason,
        };
        job.updated_at_ms = now.max(job.updated_at_ms);
        job.revision += 1;
        codec::write(connection, &job, false)?;
    }
    Ok(())
}
pub fn reconcile_switch(
    connection: &Connection,
    _switch: &GovernorSwitchState,
    now: u64,
) -> JobResult<()> {
    let ids = connection
        .prepare("SELECT job_id FROM governor_jobs WHERE release_id IS NULL ORDER BY job_id")?
        .query_map([], |r| r.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    for id in ids {
        prepare(
            connection,
            &JobId::try_from(id).map_err(|_| E::StorageFailure)?,
            now,
        )?;
    }
    Ok(())
}
pub fn project(connection: &Connection, id: &JobId, result: Legacy) -> JobResult<Reply> {
    if let Some(mode) = disposition(connection, id)? {
        if mode.unreserved
            && matches!(
                result,
                Legacy::Queued { .. } | Legacy::Granted { .. } | Legacy::ReservationRetained { .. }
            )
        {
            return Ok(if mode.release_id.is_some() {
                Reply::Unconfirmed { job_id: id.clone() }
            } else {
                Reply::Unreserved {
                    job_id: id.clone(),
                    switch_revision: mode.revision,
                    ready_at_ms: mode.ready_at.ok_or(E::StorageFailure)?,
                }
            });
        }
    }
    Ok(Reply::Traffic { result })
}

/// A pause/resume cycle can retire a reservation before its launch request
/// reaches the writer. Only negotiated, known-unreleased roots may repoll;
/// any durable release claim or launch marker retains uncertain ownership.
pub fn reservation_invalidated(
    connection: &Connection,
    request: &arc_ipc::policy::launch::LaunchRequest,
) -> JobResult<bool> {
    if !request.is_valid() {
        return Err(E::InvalidRequest);
    }
    if connection.is_autocommit() {
        return Err(E::InvalidTransition);
    }
    let Some(mode) = disposition(connection, &request.job_id)? else {
        return Ok(false);
    };
    let job = matching_job(connection, &request.job_id, &request.observer)?;
    let entry = store::entry(connection, &request.job_id)?.ok_or(E::StorageFailure)?;
    if mode.unreserved
        || mode.release_id.is_some()
        || job.child.is_some()
        || job.outcome.is_some()
        || entry.root.is_some()
        || !matches!(entry.state.as_str(), "QUEUED" | "RESERVED")
        || entry.generation == Some(request.permit_generation)
    {
        return Ok(false);
    }
    let delivered: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM audit_events WHERE id=?1 AND event_type='TRAFFIC_LAUNCH_FENCE')",
        [format!("traffic-launch-{}", request.job_id.as_str())],
        |r| r.get(0),
    )?;
    Ok(!delivered)
}
#[allow(clippy::too_many_arguments)]
pub fn enqueue(
    connection: &mut Connection,
    request: &JobRequest,
    owner: &admission::OwnerEvidence,
    policy: &arc_traffic::Policy,
    context: &LineageContext,
    proof: Option<&admission::LineageProof>,
    now: u64,
    provider: &dyn arc_policy::ObservationProvider,
) -> JobResult<Reply> {
    let JobRequest::Enqueue { job_id, .. } = request else {
        return Err(E::InvalidRequest);
    };
    let result = admission::enqueue_governed(
        connection, request, owner, policy, context, proof, now, provider, true,
    )?;
    project(connection, job_id, result)
}
#[allow(clippy::too_many_arguments)]
pub fn poll(
    connection: &mut Connection,
    request: &JobRequest,
    owner: &admission::OwnerEvidence,
    policy: &arc_traffic::Policy,
    context: &LineageContext,
    proof: Option<&admission::LineageProof>,
    now: u64,
    provider: &dyn arc_policy::ObservationProvider,
) -> JobResult<Reply> {
    let JobRequest::Poll { job_id, .. } = request else {
        return Err(E::InvalidRequest);
    };
    let result = admission::poll_observed(
        connection, request, owner, policy, context, proof, now, provider,
    )?;
    project(connection, job_id, result)
}

/// Called under IMMEDIATE + common launch lease, retained through delivery/ACK.
pub fn claim(
    connection: &Connection,
    id: &JobId,
    observer: &ProcessIdentity,
    owner: &admission::OwnerEvidence,
    release_id: &str,
    now: u64,
) -> JobResult<Launch> {
    if connection.is_autocommit() {
        return Err(E::InvalidTransition);
    }
    owner.verify(observer)?;
    let Some(entry) = store::entry(connection, id)? else {
        return Ok(Launch::NotFound {});
    };
    matching_job(connection, id, observer)?;
    if matches!(entry.state.as_str(), "CANCELLED" | "ABANDONED") {
        return Ok(Launch::Cancelled {});
    }
    if !prepare(connection, id, now)? {
        return Ok(Launch::Retry {});
    }
    let mode = disposition(connection, id)?.ok_or(E::InvalidTransition)?;
    let mut job = matching_job(connection, id, observer)?;
    if mode.release_id.is_some() {
        return Ok(Launch::Unconfirmed {});
    }
    if job.child.is_some() || job.outcome.is_some() {
        return Err(E::InvalidTransition);
    }
    let now = super::ordered_now(now, &job)?;
    super::valid_now(now, Some(&job))?;
    connection.execute("UPDATE governor_jobs SET release_id=?2,release_claimed_at_ms=?3 WHERE job_id=?1 AND release_id IS NULL",params![id.as_str(),release_id,now as i64])?;
    job.phase = Phase::Launching;
    job.launch_claimed_at_ms = Some(now);
    job.updated_at_ms = now;
    job.revision += 1;
    job.reason = arc_ipc::jobs::LifecycleReason::ClaimedNotAttached;
    job.observation = arc_ipc::jobs::IdentityObservation {
        target: arc_ipc::jobs::ObservationTarget::None,
        status: arc_ipc::jobs::ObservationStatus::NotAttached,
        observed_at_ms: now,
        reason: job.reason,
    };
    codec::write(connection, &job, false)?;
    connection.execute(
        "UPDATE traffic_entries SET state='RESERVED' WHERE job_id=?1",
        [id.as_str()],
    )?;
    Ok(Launch::Unreserved {
        switch_revision: mode.revision,
        release_id: release_id.into(),
        valid_for_ms: arc_ipc::governor::MAX_LAUNCH_VALID_FOR_MS,
    })
}
pub fn unspawned(
    connection: &mut Connection,
    id: &JobId,
    observer: &ProcessIdentity,
    token: &str,
    owner: &admission::OwnerEvidence,
    now: u64,
) -> JobResult<()> {
    owner.verify(observer)?;
    let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let job = matching_release(&tx, id, observer, token)?;
    if job.child.is_some() || job.outcome.is_some() {
        return Err(E::InvalidTransition);
    }
    let entry = store::entry(&tx, id)?.ok_or(E::StorageFailure)?;
    tx.execute(
        "UPDATE governor_jobs SET release_id=NULL,release_claimed_at_ms=NULL WHERE job_id=?1",
        [id.as_str()],
    )?;
    // Cancellation/recovery may commit after lease expiry but before this ACK.
    // Consume its matching claim without reopening a terminal job's admission.
    if !matches!(entry.state.as_str(), "CANCELLED" | "ABANDONED") {
        reset_queue(&tx, &job, now)?;
        prepare(&tx, id, now)?;
    }
    tx.commit()?;
    Ok(())
}
fn matching_release(
    connection: &Connection,
    id: &JobId,
    observer: &ProcessIdentity,
    token: &str,
) -> JobResult<arc_ipc::jobs::JobSnapshot> {
    let job = matching_job(connection, id, observer)?;
    let mode = disposition(connection, id)?.ok_or(E::InvalidTransition)?;
    if !mode.unreserved || mode.release_id.as_deref() != Some(token) {
        return Err(E::Conflict);
    }
    Ok(job)
}
pub fn attach(
    connection: &mut Connection,
    id: &JobId,
    observer: &ProcessIdentity,
    token: &str,
    child: &ProcessIdentity,
    evidence: &lifecycle::ChildEvidence,
    now: u64,
) -> JobResult<Reply> {
    let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let mut job = matching_release(&tx, id, observer, token)?;
    let entry = store::entry(&tx, id)?.ok_or(E::InvalidTransition)?;
    lifecycle::attach_inner(&tx, &mut job, &entry, child, evidence, now)?;
    let entry = store::entry(&tx, id)?.ok_or(E::StorageFailure)?;
    let result = store::result(&tx, id, &job, &entry, arc_ipc::traffic::WaitReason::Fifo)?;
    tx.commit()?;
    Ok(Reply::Traffic { result })
}
pub fn complete(
    connection: &mut Connection,
    id: &JobId,
    observer: &ProcessIdentity,
    token: &str,
    outcome: &arc_ipc::jobs::Outcome,
    now: u64,
) -> JobResult<Reply> {
    let tx = connection.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let mut job = matching_release(&tx, id, observer, token)?;
    let entry = store::entry(&tx, id)?.ok_or(E::InvalidTransition)?;
    lifecycle::finish(&tx, &mut job, &entry, outcome, now)?;
    let entry = store::entry(&tx, id)?.ok_or(E::InvalidTransition)?;
    let result = store::result(&tx, id, &job, &entry, arc_ipc::traffic::WaitReason::Fifo)?;
    tx.commit()?;
    Ok(Reply::Traffic { result })
}

/// Private witness authentication only; never an exported lineage capability.
/// Reuse the maintained owner-bound pre-exec witness with the release nonce
/// doubled to its established 64-hex key width.
pub(super) fn matches_witness(
    connection: &Connection,
    id: &JobId,
    key: &LineageContext,
) -> JobResult<bool> {
    Ok(disposition(connection, id)?
        .and_then(|m| m.release_id)
        .is_some_and(|id| key.as_str() == format!("{id}{id}")))
}

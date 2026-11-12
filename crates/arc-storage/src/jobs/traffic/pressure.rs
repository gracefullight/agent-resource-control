//! Compatibility wrappers and transactional RESERVED launch validation.
use super::{admission, get, matching_job, store, JobError as E, JobResult};
use arc_ipc::{
    jobs::Phase,
    policy::launch::{LaunchRequest, LaunchResult},
    traffic::{JobRequest, JobResult as Reply, LineageContext},
};
use arc_policy::{DecisionKind as K, DecisionReason as D, EffectivePolicy, PolicyMode};
use arc_traffic::Policy;
use rusqlite::{params, Connection, TransactionBehavior};

pub fn enqueue(
    connection: &mut Connection,
    request: &JobRequest,
    evidence: &admission::OwnerEvidence,
    policy: &Policy,
    context: &LineageContext,
    proof: Option<&admission::LineageProof>,
    now: u64,
) -> JobResult<Reply> {
    admission::enqueue_observed(
        connection,
        request,
        evidence,
        policy,
        context,
        proof,
        now,
        &arc_policy::evaluator::MissingObservation,
    )
}
pub fn poll_with_proof(
    connection: &mut Connection,
    request: &JobRequest,
    evidence: &admission::OwnerEvidence,
    policy: &Policy,
    context: &LineageContext,
    proof: Option<&admission::LineageProof>,
    now: u64,
) -> JobResult<Reply> {
    admission::poll_observed(
        connection,
        request,
        evidence,
        policy,
        context,
        proof,
        now,
        &arc_policy::evaluator::MissingObservation,
    )
}

/// Caller owns IMMEDIATE and the publication fence through COMMIT. This is
/// metadata only: no process or native sample starts here, and no IPC wait.
pub fn launch(
    connection: &Connection,
    request: &LaunchRequest,
    evidence: &admission::OwnerEvidence,
    proof: Option<&admission::LineageProof>,
    effective: &EffectivePolicy,
    now: u64,
) -> JobResult<LaunchResult> {
    if !request.is_valid() {
        return Err(E::InvalidRequest);
    }
    if connection.is_autocommit() {
        return Err(E::InvalidTransition);
    }
    let Some(entry) = store::entry(connection, &request.job_id)? else {
        return Ok(LaunchResult::NotFound {});
    };
    let job = matching_job(connection, &request.job_id, &request.observer)?;
    evidence.verify(&job.observer)?;
    if matches!(entry.state.as_str(), "CANCELLED" | "ABANDONED") {
        return Ok(LaunchResult::Cancelled {});
    }
    if entry.state != "RESERVED"
        || entry.generation != Some(request.permit_generation)
        || job.phase != Phase::Launching
        || job.child.is_some()
        || job.outcome.is_some()
    {
        return Err(E::InvalidTransition);
    }
    let root = entry.root.as_ref().unwrap_or(&request.job_id);
    let active: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM permits WHERE job_id=?1 AND generation=?2 AND state='ACTIVE')",
        params![root.as_str(), request.permit_generation as i64],
        |r| r.get(0),
    )?;
    if !active {
        return Err(E::InvalidTransition);
    }
    if entry.root.is_some() {
        let proof = proof.ok_or(E::IdentityUnavailable)?;
        admission::validate_lineage(connection, proof, &job.observer)?;
        if !proof.matches(root, request.permit_generation) {
            return Err(E::IdentityUnavailable);
        }
        crate::policy::record(
            connection,
            &job.job_id,
            effective,
            K::Reuse,
            D::ExistingReservation,
            entry.generation,
            now,
        )?;
        return Ok(LaunchResult::Reused {
            permit_generation: request.permit_generation,
        });
    }
    if effective.mode == PolicyMode::MemoryHold {
        crate::policy::record(
            connection,
            &job.job_id,
            effective,
            K::Wait,
            D::MemoryPressure,
            entry.generation,
            now,
        )?;
        return Ok(LaunchResult::Held {
            policy: effective.clone(),
        });
    }
    // A stable, owner-bound one-shot release marker survives lost replies and
    // daemon restart. An uncertain previous release must use witness recovery,
    // never a second READY. Existing terminal handling/cancellation owns cleanup.
    let inserted=connection.execute(
        "INSERT OR IGNORE INTO audit_events(id,severity,event_type,entity_id,payload_redacted_json,created_at)
         VALUES(?1,'INFO','TRAFFIC_LAUNCH_FENCE',?2,NULL,?3)",
        params![format!("traffic-launch-{}",request.job_id.as_str()),request.job_id.as_str(),now as i64])?;
    if inserted != 1 {
        return Err(E::InvalidTransition);
    }
    // Recheck lifecycle after all eligibility checks under this transaction.
    let current = get(connection, &job.job_id)?.ok_or(E::StorageFailure)?;
    if current.revision != job.revision {
        return Err(E::Conflict);
    }
    crate::policy::record(
        connection,
        &job.job_id,
        effective,
        K::Admit,
        D::Admitted,
        entry.generation,
        now,
    )?;
    Ok(LaunchResult::Ready {
        permit_generation: request.permit_generation,
        policy: effective.clone(),
    })
}

/// Only the single writer may call this, either before delivery or after a
/// matching same-connection, owner-authenticated Unspawned ACK. Lost replies,
/// missing/malformed ACKs and Started releases retain their one-shot marker.
pub fn discard_unreleased_launch(
    connection: &mut Connection,
    request: &LaunchRequest,
) -> JobResult<()> {
    if !request.is_valid() {
        return Err(E::InvalidRequest);
    }
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let job = matching_job(&tx, &request.job_id, &request.observer)?;
    let entry = store::entry(&tx, &request.job_id)?.ok_or(E::InvalidTransition)?;
    if entry.state != "RESERVED"
        || entry.root.is_some()
        || entry.generation != Some(request.permit_generation)
        || job.phase != Phase::Launching
        || job.child.is_some()
        || job.outcome.is_some()
    {
        return Err(E::InvalidTransition);
    }
    let removed = tx.execute(
        "DELETE FROM audit_events WHERE id=?1 AND event_type='TRAFFIC_LAUNCH_FENCE' AND entity_id=?2",
        params![format!("traffic-launch-{}", request.job_id.as_str()), request.job_id.as_str()],
    )?;
    if removed != 1 {
        return Err(E::InvalidTransition);
    }
    tx.commit()?;
    Ok(())
}

/// Native owner evidence is collected before entering the single writer.
pub fn acknowledge_unspawned_launch(
    connection: &mut Connection,
    request: &LaunchRequest,
    owner: &admission::OwnerEvidence,
) -> JobResult<()> {
    owner.verify(&request.observer)?;
    discard_unreleased_launch(connection, request)
}

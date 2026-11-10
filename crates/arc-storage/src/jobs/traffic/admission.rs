use super::{get, matching_job, store, valid_now, JobResult};
use crate::jobs::{codec, JobError as E};
use arc_core::process_identity::{probe_live, IdentityStatus};
use arc_ipc::{
    jobs::{
        IdentityObservation, JobId, JobSnapshot, LifecycleReason as R, ObservationStatus as S,
        ObservationTarget as T, Phase, ProcessIdentity,
    },
    traffic::{JobRequest, JobResult as Reply, LineageContext, WaitReason},
};
use arc_traffic::{decide, Active, Decision, Policy};
use rusqlite::{params, Connection, TransactionBehavior};

/// Native liveness observation, collected outside the writer transaction.
#[derive(Debug, Clone)]
pub struct OwnerEvidence {
    identity: ProcessIdentity,
    status: IdentityStatus,
}
impl OwnerEvidence {
    pub fn gather(identity: &ProcessIdentity) -> Self {
        Self {
            identity: identity.clone(),
            status: probe_live(identity),
        }
    }
    #[cfg(test)]
    pub(super) fn simulated(identity: &ProcessIdentity, status: IdentityStatus) -> Self {
        Self {
            identity: identity.clone(),
            status,
        }
    }

    pub(super) fn verify(&self, identity: &ProcessIdentity) -> JobResult<()> {
        if self.identity != *identity {
            return Err(E::InvalidRequest);
        }
        if self.status != IdentityStatus::VerifiedMatch {
            return Err(E::IdentityUnavailable);
        }
        Ok(())
    }
}

/// An active ancestry anchor and the root that owns its reservation. The
/// `root` field is the candidate job snapshot, including for a descendant; its
/// name is retained for callers of the original single-snapshot interface.
/// No context is returned, and a snapshot alone is not proof of ancestry.
#[derive(Debug, Clone)]
pub struct LineageSnapshot {
    pub root: JobSnapshot,
    pub permit_job_id: JobId,
    pub generation: u64,
}

/// The daemon constructs this only after verifying the authenticated socket
/// peer's native ancestry to `anchor`. Storage rechecks the context, revision,
/// anchor, generation and active permit under the write transaction.
#[derive(Debug, Clone)]
pub struct LineageProof {
    root_id: JobId,
    anchor_job_id: JobId,
    revision: u64,
    generation: u64,
    anchor: ProcessIdentity,
}
impl LineageProof {
    pub fn verified(snapshot: &LineageSnapshot, anchor: &ProcessIdentity) -> Self {
        Self {
            root_id: snapshot.permit_job_id.clone(),
            anchor_job_id: snapshot.root.job_id.clone(),
            revision: snapshot.root.revision,
            generation: snapshot.generation,
            anchor: anchor.clone(),
        }
    }
}

pub fn lineage_snapshot(
    connection: &Connection,
    context: &LineageContext,
) -> JobResult<Option<LineageSnapshot>> {
    Ok(lineage_snapshots(connection, context)?.into_iter().next())
}

/// Return every active candidate in the lineage. A daemon must check native
/// ancestry against each candidate: an active sibling is not necessarily an
/// ancestor of the requesting process. The permit owner can be terminal while
/// a descendant keeps its reservation active.
pub fn lineage_snapshots(
    connection: &Connection,
    context: &LineageContext,
) -> JobResult<Vec<LineageSnapshot>> {
    let mut statement = connection.prepare(
        "SELECT candidate.job_id, root.job_id, root.generation \
         FROM traffic_entries root \
         JOIN permits p ON p.job_id=root.job_id AND p.state='ACTIVE' AND p.generation=root.generation \
         JOIN traffic_entries candidate ON (candidate.job_id=root.job_id OR candidate.root_job_id=root.job_id) \
         WHERE root.context=?1 AND root.root_job_id IS NULL \
           AND candidate.state IN ('RESERVED','RUNNING','RETAINED') \
           AND candidate.generation=root.generation \
         ORDER BY candidate.sequence",
    )?;
    let mut rows = statement.query([context.as_str()])?;
    let mut snapshots = Vec::new();
    while let Some(row) = rows.next()? {
        let id = JobId::try_from(row.get::<_, String>(0)?).map_err(|_| E::StorageFailure)?;
        let permit_job_id =
            JobId::try_from(row.get::<_, String>(1)?).map_err(|_| E::StorageFailure)?;
        let generation = u64::try_from(row.get::<_, i64>(2)?).map_err(|_| E::StorageFailure)?;
        snapshots.push(LineageSnapshot {
            root: get(connection, &id)?.ok_or(E::StorageFailure)?,
            permit_job_id,
            generation,
        });
    }
    Ok(snapshots)
}

fn validate_lineage(
    connection: &Connection,
    context: &LineageContext,
    proof: &LineageProof,
) -> JobResult<()> {
    let snapshot = lineage_snapshots(connection, context)?
        .into_iter()
        .find(|snapshot| snapshot.root.job_id == proof.anchor_job_id)
        .ok_or(E::IdentityUnavailable)?;
    if snapshot.permit_job_id != proof.root_id
        || snapshot.root.revision != proof.revision
        || snapshot.generation != proof.generation
        || (snapshot.root.observer != proof.anchor
            && snapshot.root.child.as_ref() != Some(&proof.anchor))
    {
        return Err(E::IdentityUnavailable);
    }
    Ok(())
}

/// Enqueue one eligible job and attempt its admission atomically. `new_context`
/// must be an unpredictable, freshly minted capability; it is only persisted on
/// a grant. A context supplied by the requester does not establish ancestry.
pub fn enqueue(
    connection: &mut Connection,
    request: &JobRequest,
    evidence: &OwnerEvidence,
    policy: &Policy,
    new_context: &LineageContext,
    proof: Option<&LineageProof>,
    now: u64,
) -> JobResult<Reply> {
    valid_now(now, None)?;
    if !request.is_valid() {
        return Err(E::InvalidRequest);
    }
    let JobRequest::Enqueue {
        job_id,
        observer,
        classification,
        lineage_context,
    } = request
    else {
        return Err(E::InvalidRequest);
    };
    if evidence.identity != *observer {
        return Err(E::InvalidRequest);
    }
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    if let Some(e) = store::entry(&tx, job_id)? {
        let job = matching_job(&tx, job_id, observer)?;
        if job.classification != *classification || e.inherited != *lineage_context {
            return Err(E::Conflict);
        }
        let reply = try_grant(
            &tx,
            &job,
            &e,
            Admission {
                evidence,
                policy,
                context: new_context,
                proof,
                now,
            },
        )?;
        tx.commit()?;
        return Ok(reply);
    }
    if let Some(reply) = store::tombstone(&tx, job_id, observer)? {
        return Ok(reply);
    }
    let collision: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM jobs WHERE id=?1)",
        [job_id.as_str()],
        |r| r.get(0),
    )?;
    if collision {
        return Err(E::Conflict);
    }
    match decide(*classification, Active::default(), policy, true).map_err(|_| E::InvalidRequest)? {
        Decision::Bypass(reason) => return Ok(Reply::Bypassed { reason }),
        Decision::Admit { .. } => (),
        Decision::Wait(_) => return Err(E::InvalidRequest),
    }
    let job = JobSnapshot {
        job_id: job_id.clone(),
        phase: Phase::Registered,
        classification: *classification,
        observer: observer.clone(),
        observer_verified_at_ms: now,
        child: None,
        child_verified_at_ms: None,
        observation: IdentityObservation {
            target: T::Observer,
            status: S::VerifiedMatch,
            observed_at_ms: now,
            reason: R::RegisteredNotClaimed,
        },
        reason: R::RegisteredNotClaimed,
        outcome: None,
        registered_at_ms: now,
        launch_claimed_at_ms: None,
        completed_at_ms: None,
        updated_at_ms: now,
        revision: 1,
    };
    codec::write(&tx, &job, true)?;
    tx.execute(
        "UPDATE jobs SET enforcement='SCHEDULED', queue_at=?2 WHERE id=?1",
        params![job_id.as_str(), now as i64],
    )?;
    tx.execute("INSERT INTO traffic_entries(job_id,state,queued_at_ms,inherited_context) VALUES (?1,'QUEUED',?2,?3)", params![job_id.as_str(), now as i64, lineage_context.as_ref().map(LineageContext::as_str)])?;
    let e = store::entry(&tx, job_id)?.ok_or(E::StorageFailure)?;
    let reply = try_grant(
        &tx,
        &job,
        &e,
        Admission {
            evidence,
            policy,
            context: new_context,
            proof,
            now,
        },
    )?;
    tx.commit()?;
    Ok(reply)
}

/// Poll consumes no long-lived ticket: each attempt opens and closes one short
/// transaction. A returned grant is stable after lost replies and restarts.
/// The daemon must gather fresh owner evidence before each poll; queued jobs
/// require a verified matching identity before either waiting or admission.
pub fn poll(
    connection: &mut Connection,
    request: &JobRequest,
    evidence: &OwnerEvidence,
    policy: &Policy,
    new_context: &LineageContext,
    now: u64,
) -> JobResult<Reply> {
    valid_now(now, None)?;
    if !request.is_valid() {
        return Err(E::InvalidRequest);
    }
    let JobRequest::Poll { job_id, observer } = request else {
        return Err(E::InvalidRequest);
    };
    if evidence.identity != *observer {
        return Err(E::InvalidRequest);
    }
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let Some(e) = store::entry(&tx, job_id)? else {
        return Ok(store::tombstone(&tx, job_id, observer)?.unwrap_or(Reply::NotFound {}));
    };
    let job = matching_job(&tx, job_id, observer)?;
    let reply = try_grant(
        &tx,
        &job,
        &e,
        Admission {
            evidence,
            policy,
            context: new_context,
            proof: None,
            now,
        },
    )?;
    tx.commit()?;
    Ok(reply)
}

struct Admission<'a> {
    evidence: &'a OwnerEvidence,
    policy: &'a Policy,
    context: &'a LineageContext,
    proof: Option<&'a LineageProof>,
    now: u64,
}

fn try_grant(
    connection: &Connection,
    job: &JobSnapshot,
    e: &store::Entry,
    admission: Admission<'_>,
) -> JobResult<Reply> {
    if e.state != "QUEUED" {
        return store::result(connection, &job.job_id, job, e, WaitReason::Fifo);
    }
    admission.evidence.verify(&job.observer)?;
    valid_now(admission.now, Some(job))?;
    if let Some(proof) = admission.proof {
        let inherited = e.inherited.as_ref().ok_or(E::InvalidRequest)?;
        validate_lineage(connection, inherited, proof)?;
        connection.execute("UPDATE traffic_entries SET state='RESERVED',granted_at_ms=?2,generation=?3,root_job_id=?4 WHERE job_id=?1 AND state='QUEUED'", params![job.job_id.as_str(), admission.now as i64, proof.generation as i64, proof.root_id.as_str()])?;
        let mut claimed = job.clone();
        claim(connection, &mut claimed, admission.now)?;
        let reused = store::entry(connection, &job.job_id)?.ok_or(E::StorageFailure)?;
        return store::result(connection, &job.job_id, &claimed, &reused, WaitReason::Fifo);
    }
    let head: i64 = connection.query_row(
        "SELECT sequence FROM traffic_entries WHERE state='QUEUED' ORDER BY sequence LIMIT 1",
        [],
        |r| r.get(0),
    )?;
    match decide(
        job.classification,
        store::active(connection)?,
        admission.policy,
        head == e.sequence as i64,
    )
    .map_err(|_| E::InvalidRequest)?
    {
        Decision::Wait(reason) => store::result(connection, &job.job_id, job, e, reason),
        Decision::Bypass(_) => Err(E::StorageFailure),
        Decision::Admit { class, reservation } => {
            let (_, config) = store::policy_class(class, admission.policy);
            if config.reservation != reservation {
                return Err(E::StorageFailure);
            }
            connection.execute("INSERT INTO permits(id,job_id,state,cpu_cores_est,memory_bytes_est,reserved_at,cpu_millis_est,generation) VALUES (?1,?1,'ACTIVE',?2,?3,?4,?5,?6)", params![job.job_id.as_str(), reservation.cpu_millis as f64 / 1000.0, reservation.memory_bytes as i64, admission.now as i64, reservation.cpu_millis as i64, e.sequence as i64])?;
            connection.execute("UPDATE traffic_entries SET state='RESERVED',granted_at_ms=?2,generation=sequence,context=?3 WHERE job_id=?1 AND state='QUEUED'", params![job.job_id.as_str(), admission.now as i64, admission.context.as_str()])?;
            let mut claimed = job.clone();
            claim(connection, &mut claimed, admission.now)?;
            let granted = store::entry(connection, &job.job_id)?.ok_or(E::StorageFailure)?;
            store::result(
                connection,
                &job.job_id,
                &claimed,
                &granted,
                WaitReason::Fifo,
            )
        }
    }
}

fn claim(connection: &Connection, job: &mut JobSnapshot, now: u64) -> JobResult<()> {
    valid_now(now, Some(job))?;
    job.phase = Phase::Launching;
    job.launch_claimed_at_ms = Some(now);
    job.reason = R::ClaimedNotAttached;
    job.observation = IdentityObservation {
        target: T::None,
        status: S::NotAttached,
        observed_at_ms: now,
        reason: job.reason,
    };
    job.updated_at_ms = now;
    job.revision += 1;
    codec::write(connection, job, false)?;
    connection.execute(
        "UPDATE jobs SET reserved_at=?2 WHERE id=?1",
        params![job.job_id.as_str(), now as i64],
    )?;
    Ok(())
}

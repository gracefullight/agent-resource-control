use super::{get, matching_job, ordered_now, poll_with_proof, store, valid_now, JobResult};
use crate::jobs::{codec, JobError as E};
use arc_core::process_identity::{probe_live, IdentityStatus};
use arc_ipc::{
    jobs::{
        IdentityObservation, JobSnapshot, LifecycleReason as R, ObservationStatus as S,
        ObservationTarget as T, Phase, ProcessIdentity,
    },
    traffic::{JobRequest, JobResult as Reply, LineageContext, WaitReason},
};
use arc_policy::{
    DecisionKind as K, DecisionReason as D, EffectivePolicy, ObservationProvider, PolicyMode,
};
use arc_traffic::{decide, Active, Decision, Policy};
use rusqlite::{params, Connection, OptionalExtension};

/// Native liveness observation, collected outside the writer transaction.
#[derive(Debug, Clone)]
pub struct OwnerEvidence {
    identity: ProcessIdentity,
    status: IdentityStatus,
    meter: Option<Box<crate::job_meter::LineageEvidence>>,
}
impl OwnerEvidence {
    pub fn with_meter_evidence(mut self, evidence: crate::job_meter::LineageEvidence) -> Self {
        self.meter = Some(Box::new(evidence));
        self
    }
    pub fn gather(identity: &ProcessIdentity) -> Self {
        Self {
            meter: None,
            identity: identity.clone(),
            status: probe_live(identity),
        }
    }
    #[cfg(test)]
    pub(super) fn simulated(identity: &ProcessIdentity, status: IdentityStatus) -> Self {
        Self {
            meter: None,
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

#[path = "lineage_admission.rs"]
mod lineage;
pub(super) use lineage::validate_lineage;
pub use lineage::{
    lineage_anchors, lineage_snapshot, lineage_snapshots, LineageProof, LineageSnapshot,
};

/// Enqueue one eligible job and attempt its admission atomically. `new_context`
/// must be an unpredictable, freshly minted capability; it is only persisted on
/// a grant. A context supplied by the requester does not establish ancestry.
#[allow(clippy::too_many_arguments)]
pub fn enqueue_observed(
    connection: &mut Connection,
    request: &JobRequest,
    evidence: &OwnerEvidence,
    policy: &Policy,
    new_context: &LineageContext,
    proof: Option<&LineageProof>,
    now: u64,
    provider: &dyn ObservationProvider,
) -> JobResult<Reply> {
    enqueue_governed(
        connection,
        request,
        evidence,
        policy,
        new_context,
        proof,
        now,
        provider,
        false,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn enqueue_governed(
    connection: &mut Connection,
    request: &JobRequest,
    evidence: &OwnerEvidence,
    policy: &Policy,
    new_context: &LineageContext,
    proof: Option<&LineageProof>,
    now: u64,
    provider: &dyn ObservationProvider,
    negotiated: bool,
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
    crate::policy::transaction(connection, provider, |tx, effective| {
        if let Some(e) = store::entry(tx, job_id)? {
            let job = matching_job(tx, job_id, observer)?;
            if job.classification != *classification || e.inherited != *lineage_context {
                return Err(E::Conflict);
            }
            let reply = try_grant(
                tx,
                &job,
                &e,
                Admission {
                    evidence,
                    policy,
                    context: new_context,
                    proof,
                    now,
                    effective,
                },
            )?;
            return Ok(reply);
        }
        if let Some(reply) = store::tombstone(tx, job_id, observer)? {
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
        match decide(*classification, Active::default(), policy, true)
            .map_err(|_| E::InvalidRequest)?
        {
            Decision::Bypass(reason) => return Ok(Reply::Bypassed { reason }),
            Decision::Admit { .. } => (),
            Decision::Wait(_) => return Err(E::InvalidRequest),
        }
        evidence.verify(observer)?;
        let switch = crate::governor::get_switch(tx)?;
        if !negotiated && !switch.enabled && proof.is_none() {
            return Ok(Reply::Bypassed {
                reason: arc_ipc::traffic::BypassReason::Unsupported,
            });
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
        codec::write(tx, &job, true)?;
        tx.execute(
            "UPDATE jobs SET enforcement='SCHEDULED', queue_at=?2 WHERE id=?1",
            params![job_id.as_str(), now as i64],
        )?;
        tx.execute("INSERT INTO traffic_entries(job_id,state,queued_at_ms,inherited_context) VALUES (?1,'QUEUED',?2,?3)", params![job_id.as_str(), now as i64, lineage_context.as_ref().map(LineageContext::as_str)])?;
        if negotiated {
            tx.execute(
                "INSERT INTO governor_jobs(job_id,mode,switch_revision) VALUES(?1,'SCHEDULED',?2)",
                params![job_id.as_str(), switch.revision as i64],
            )?;
        }
        let e = store::entry(tx, job_id)?.ok_or(E::StorageFailure)?;
        let reply = try_grant(
            tx,
            &job,
            &e,
            Admission {
                evidence,
                policy,
                context: new_context,
                proof,
                now,
                effective,
            },
        )?;
        if let Some(proof) = &evidence.meter {
            let job = get(tx, job_id)?.ok_or(E::StorageFailure)?;
            crate::job_meter::bind(tx, &job, proof)?;
        }
        Ok(reply)
    })
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
    poll_with_proof(
        connection,
        request,
        evidence,
        policy,
        new_context,
        None,
        now,
    )
}

/// Poll can discover the same verified ancestry as an enqueue, including for
/// protocol-1 clients that poll after sending a newly generated claim.
#[allow(clippy::too_many_arguments)]
pub fn poll_observed(
    connection: &mut Connection,
    request: &JobRequest,
    evidence: &OwnerEvidence,
    policy: &Policy,
    new_context: &LineageContext,
    proof: Option<&LineageProof>,
    now: u64,
    provider: &dyn ObservationProvider,
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
    crate::policy::transaction(connection, provider, |tx, effective| {
        let Some(e) = store::entry(tx, job_id)? else {
            return Ok(store::tombstone(tx, job_id, observer)?.unwrap_or(Reply::NotFound {}));
        };
        let job = matching_job(tx, job_id, observer)?;
        let reply = try_grant(
            tx,
            &job,
            &e,
            Admission {
                evidence,
                policy,
                context: new_context,
                proof,
                now,
                effective,
            },
        )?;
        if let Some(proof) = &evidence.meter {
            let job = get(tx, job_id)?.ok_or(E::StorageFailure)?;
            crate::job_meter::bind(tx, &job, proof)?;
        }
        Ok(reply)
    })
}

struct Admission<'a> {
    evidence: &'a OwnerEvidence,
    policy: &'a Policy,
    context: &'a LineageContext,
    proof: Option<&'a LineageProof>,
    now: u64,
    effective: &'a EffectivePolicy,
}

fn try_grant(
    connection: &Connection,
    job: &JobSnapshot,
    e: &store::Entry,
    mut admission: Admission<'_>,
) -> JobResult<Reply> {
    admission.now = ordered_now(admission.now, job)?;
    if !(e.state == "QUEUED"
        && admission.proof.is_some()
        && super::governor::disposition(connection, &job.job_id)?.is_none_or(|d| !d.unreserved))
        && super::governor::prepare(connection, &job.job_id, admission.now)?
    {
        // An eligibility reply is projected by the negotiated wrapper, never
        // mistaken for an actual permit or native launch release.
        return Ok(Reply::Queued {
            sequence: e.sequence,
            queued_at_ms: e.queued_at,
            reason: WaitReason::Fifo,
        });
    }
    let e = &store::entry(connection, &job.job_id)?.ok_or(E::StorageFailure)?;
    if e.state != "QUEUED" {
        let reply = store::result(connection, &job.job_id, job, e, WaitReason::Fifo)?;
        if matches!(e.state.as_str(), "RESERVED" | "RUNNING" | "RETAINED") {
            let (kind, reason) = if e.root.is_some() {
                (K::Reuse, D::ExistingReservation)
            } else if e.state == "RUNNING" || job.child.is_some() {
                (K::Preserve, D::Running)
            } else if admission.effective.mode == PolicyMode::MemoryHold {
                (K::Wait, D::MemoryPressure)
            } else {
                (K::Admit, D::Admitted)
            };
            crate::policy::record(
                connection,
                &job.job_id,
                admission.effective,
                kind,
                reason,
                e.generation,
                admission.now,
            )?;
        }
        return Ok(reply);
    }
    admission.evidence.verify(&job.observer)?;
    valid_now(admission.now, Some(job))?;
    if let Some(proof) = admission.proof {
        validate_lineage(connection, proof, &job.observer)?;
        connection.execute("UPDATE traffic_entries SET state='RESERVED',granted_at_ms=?2,generation=?3,root_job_id=?4 WHERE job_id=?1 AND state='QUEUED'", params![job.job_id.as_str(), admission.now as i64, proof.generation as i64, proof.root_id.as_str()])?;
        let mut claimed = job.clone();
        claim(connection, &mut claimed, admission.now)?;
        let reused = store::entry(connection, &job.job_id)?.ok_or(E::StorageFailure)?;
        crate::policy::record(
            connection,
            &job.job_id,
            admission.effective,
            K::Reuse,
            D::ExistingReservation,
            Some(proof.generation),
            admission.now,
        )?;
        return store::result(connection, &job.job_id, &claimed, &reused, WaitReason::Fifo);
    }
    if admission.effective.mode == PolicyMode::MemoryHold {
        crate::policy::record(
            connection,
            &job.job_id,
            admission.effective,
            K::Wait,
            D::MemoryPressure,
            None,
            admission.now,
        )?;
        return store::result(connection, &job.job_id, job, e, WaitReason::MemoryBudget);
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
        Decision::Wait(reason) => {
            let decision = match reason {
                WaitReason::Fifo => D::Fifo,
                WaitReason::ClassWindow => D::ClassWindow,
                WaitReason::CpuBudget => D::CpuBudget,
                WaitReason::MemoryBudget => D::MemoryBudget,
            };
            crate::policy::record(
                connection,
                &job.job_id,
                admission.effective,
                K::Wait,
                decision,
                None,
                admission.now,
            )?;
            store::result(connection, &job.job_id, job, e, reason)
        }
        Decision::Bypass(_) => Err(E::StorageFailure),
        Decision::Admit { class, reservation } => {
            let (_, config) = store::policy_class(class, admission.policy);
            if config.reservation != reservation {
                return Err(E::StorageFailure);
            }
            let previous: Option<i64> = connection
                .query_row(
                    "SELECT generation FROM permits WHERE job_id=?1",
                    [job.job_id.as_str()],
                    |r| r.get(0),
                )
                .optional()?;
            let generation = previous
                .map_or(Some(e.sequence), |n| u64::try_from(n).ok()?.checked_add(1))
                .filter(|n| *n <= arc_ipc::governor::MAX_SAFE_INTEGER)
                .ok_or(E::StorageFailure)?;
            let stored=connection.execute("INSERT INTO permits(id,job_id,state,cpu_cores_est,memory_bytes_est,reserved_at,cpu_millis_est,generation) VALUES (?1,?1,'ACTIVE',?2,?3,?4,?5,?6) ON CONFLICT(job_id) DO UPDATE SET state='ACTIVE',cpu_cores_est=excluded.cpu_cores_est,memory_bytes_est=excluded.memory_bytes_est,reserved_at=excluded.reserved_at,released_at=NULL,cpu_millis_est=excluded.cpu_millis_est,generation=excluded.generation WHERE permits.state='RELEASED'", params![job.job_id.as_str(), reservation.cpu_millis as f64 / 1000.0, reservation.memory_bytes as i64, admission.now as i64, reservation.cpu_millis as i64, generation as i64])?;
            if stored != 1 {
                return Err(E::Conflict);
            }
            connection.execute("UPDATE traffic_entries SET state='RESERVED',granted_at_ms=?2,generation=?4,context=?3 WHERE job_id=?1 AND state='QUEUED'", params![job.job_id.as_str(), admission.now as i64, admission.context.as_str(), generation as i64])?;
            let mut claimed = job.clone();
            claim(connection, &mut claimed, admission.now)?;
            let granted = store::entry(connection, &job.job_id)?.ok_or(E::StorageFailure)?;
            crate::policy::record(
                connection,
                &job.job_id,
                admission.effective,
                K::Admit,
                D::Admitted,
                Some(generation),
                admission.now,
            )?;
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

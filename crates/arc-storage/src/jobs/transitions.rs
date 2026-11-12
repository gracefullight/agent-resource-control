use super::{codec, get, valid_now, JobError, JobResult};
use arc_core::process_identity::{probe, probe_live, IdentityStatus, ProcessIdentity};
use arc_ipc::jobs::{
    CompleteDisposition, CompleteRequest, CompleteResult, IdentityObservation, JobSnapshot,
    LifecycleReason as R, ObservationStatus as S, ObservationTarget as T, Outcome, Phase,
    StartDisposition as D, StartRequest, StartResult,
};
use rusqlite::{Connection, TransactionBehavior};

/// Bound native evidence: gather before admission to the database worker.
/// Private fields prevent a wire assertion from becoming verification evidence.
#[derive(Debug, Clone)]
pub struct StartEvidence {
    observer: ProcessIdentity,
    observer_status: IdentityStatus,
    child: Option<(ProcessIdentity, IdentityStatus)>,
    child_identity_status: Option<IdentityStatus>,
    meter: Option<Box<crate::job_meter::LineageEvidence>>,
}
impl StartEvidence {
    pub fn with_meter_evidence(mut self, evidence: crate::job_meter::LineageEvidence) -> Self {
        self.meter = Some(Box::new(evidence));
        self
    }
    #[cfg(test)]
    pub(super) fn simulated(request: &StartRequest, status: IdentityStatus) -> Self {
        let mut evidence = Self::gather(request);
        if let Some((_, child_status)) = &mut evidence.child {
            *child_status = status;
            evidence.child_identity_status = Some(status);
        } else {
            evidence.observer_status = status;
        }
        evidence
    }
    pub fn gather(request: &StartRequest) -> Self {
        Self {
            meter: None,
            observer: request.observer().clone(),
            observer_status: probe_live(request.observer()),
            child_identity_status: match request {
                StartRequest::Attach { child, .. } => Some(probe(child)),
                _ => None,
            },
            child: match request {
                StartRequest::Attach { child, .. } => Some((child.clone(), probe_live(child))),
                _ => None,
            },
        }
    }
}
/// An error after a caller deadline may follow COMMIT. Resolve with the same key;
/// no disposition grants a restarted observer permission to launch work.
pub fn start(
    connection: &mut Connection,
    request: &StartRequest,
    evidence: &StartEvidence,
    now: u64,
) -> JobResult<StartResult> {
    valid_now(now, None)?;
    if !request.is_valid() || evidence.observer != *request.observer() {
        return Err(JobError::InvalidRequest);
    }
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    if super::traffic::owned(&tx, request.job_id())? {
        return Err(JobError::InvalidTransition);
    }
    let existing = get(&tx, request.job_id())?;
    let (job, disposition) = match existing {
        None => {
            let StartRequest::Register {
                job_id,
                observer,
                classification,
            } = request
            else {
                return Err(JobError::InvalidTransition);
            };
            let collision: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM jobs WHERE id=?1)",
                [job_id.as_str()],
                |row| row.get(0),
            )?;
            if collision {
                return Err(JobError::Conflict);
            }
            if evidence.observer_status != IdentityStatus::VerifiedMatch {
                return Err(JobError::IdentityUnavailable);
            }
            (
                JobSnapshot {
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
                },
                D::Created,
            )
        }
        Some(mut job) => {
            if job.observer != *request.observer() {
                return Err(JobError::Conflict);
            }
            match request {
                StartRequest::Register { classification, .. } => {
                    if job.classification != *classification {
                        return Err(JobError::Conflict);
                    }
                    return Ok(StartResult {
                        disposition: D::Unchanged,
                        job,
                    });
                }
                StartRequest::Claim { .. } => {
                    if job.launch_claimed_at_ms.is_some() {
                        return Ok(StartResult {
                            disposition: D::Unchanged,
                            job,
                        });
                    }
                    if job.phase != Phase::Registered {
                        return Err(JobError::InvalidTransition);
                    }
                    if evidence.observer_status != IdentityStatus::VerifiedMatch {
                        return Err(JobError::IdentityUnavailable);
                    }
                    valid_now(now, Some(&job))?;
                    job.phase = Phase::Launching;
                    job.launch_claimed_at_ms = Some(now);
                    job.reason = R::ClaimedNotAttached;
                    job.observation = IdentityObservation {
                        target: T::None,
                        status: S::NotAttached,
                        observed_at_ms: now,
                        reason: job.reason,
                    };
                }
                StartRequest::Attach { child, .. } => {
                    if let Some(prior) = &job.child {
                        if prior != child {
                            return Err(JobError::Conflict);
                        }
                        return Ok(StartResult {
                            disposition: D::Unchanged,
                            job,
                        });
                    }
                    if job.outcome.is_some() || job.launch_claimed_at_ms.is_none() {
                        return Err(JobError::InvalidTransition);
                    }
                    let Some((identity, status)) = &evidence.child else {
                        return Err(JobError::InvalidRequest);
                    };
                    if identity != child {
                        return Err(JobError::InvalidRequest);
                    }
                    valid_now(now, Some(&job))?;
                    job.child = Some(child.clone());
                    if evidence.child_identity_status == Some(IdentityStatus::VerifiedMatch) {
                        job.child_verified_at_ms = Some(now);
                    }
                    super::reconcile::update_observation(&mut job, T::Child, *status, now);
                }
            }
            job.updated_at_ms = now;
            job.revision += 1;
            (job, D::Applied)
        }
    };
    codec::write(&tx, &job, disposition == D::Created)?;
    if let Some(proof) = &evidence.meter {
        crate::job_meter::bind(&tx, &job, proof)?;
    }
    tx.commit()?;
    Ok(StartResult { disposition, job })
}

pub fn complete(
    connection: &mut Connection,
    request: &CompleteRequest,
    now: u64,
) -> JobResult<CompleteResult> {
    valid_now(now, None)?;
    if !request.is_valid() {
        return Err(JobError::InvalidRequest);
    }
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    if super::traffic::owned(&tx, &request.job_id)? {
        return Err(JobError::InvalidTransition);
    }
    let mut job = get(&tx, &request.job_id)?.ok_or(JobError::InvalidTransition)?;
    if job.observer != request.observer {
        return Err(JobError::Conflict);
    }
    if let Some(outcome) = &job.outcome {
        if outcome != &request.outcome {
            return Err(JobError::Conflict);
        }
        return Ok(CompleteResult {
            disposition: CompleteDisposition::Unchanged,
            job,
        });
    }
    if job.launch_claimed_at_ms.is_none()
        || matches!(request.outcome, Outcome::SpawnFailed { .. }) && job.child.is_some()
    {
        return Err(JobError::InvalidTransition);
    }
    valid_now(now, Some(&job))?;
    (job.phase, job.reason) = match request.outcome {
        Outcome::Exited { .. } => (Phase::Completed, R::ObservedExit),
        Outcome::Signaled { .. } => (Phase::Signaled, R::ObservedSignal),
        Outcome::SpawnFailed { .. } => (Phase::SpawnFailed, R::ObservedSpawnFailure),
    };
    job.outcome = Some(request.outcome.clone());
    job.completed_at_ms = Some(now);
    job.updated_at_ms = now;
    job.revision += 1;
    // Parent observation of an outcome is distinct from native child identity.
    job.observation.reason = job.reason;
    // Keep the native identity observation time; completion is a separate event.
    codec::write(&tx, &job, false)?;
    tx.commit()?;
    Ok(CompleteResult {
        disposition: CompleteDisposition::Applied,
        job,
    })
}

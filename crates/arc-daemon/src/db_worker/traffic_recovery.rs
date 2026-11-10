//! Bounded recovery probes, kept outside the SQLite worker's write lock.
use super::{call, jobs, witness, DbClient, JobError, Operation, Value};
use arc_core::process_identity::{probe_live, IdentityStatus};
use arc_storage::jobs::{self as storage, traffic};
use std::{path::Path, time::Instant};

/// Before publishing the worker, reconcile all active pages.
pub(crate) fn recover_at_start(connection: &mut rusqlite::Connection) -> Result<(), JobError> {
    let socket = arc_core::socket_path().ok();
    recover_at_start_inner(connection, socket.as_deref())
}

#[cfg(test)]
pub(super) fn recover_at_start_on(
    connection: &mut rusqlite::Connection,
    socket: &Path,
) -> Result<(), JobError> {
    recover_at_start_inner(connection, Some(socket))
}

fn recover_at_start_inner(
    connection: &mut rusqlite::Connection,
    socket: Option<&Path>,
) -> Result<(), JobError> {
    if let Some(head) = traffic::queue_head(connection)? {
        let evidence = traffic::QueueObservation::gather(&head, jobs::now());
        traffic::reconcile_queued(connection, &evidence)?;
    }
    let mut after = 0;
    loop {
        let candidates = traffic::recovery_candidates(connection, after, 64)?;
        let count = candidates.len();
        for candidate in candidates {
            after = candidate.sequence;
            if candidate.job.child.is_none() {
                if let Some(receipt) = socket.and_then(|path| witness::read_at(path, &candidate)) {
                    let applied = match receipt.proof() {
                        witness::Proof::Child(child)
                            if probe_live(child) == IdentityStatus::Indeterminate =>
                        {
                            false
                        }
                        witness::Proof::Child(child) => traffic::recover_witness(
                            connection,
                            &traffic::WitnessEvidence::gather(
                                &candidate.job,
                                receipt.key(),
                                child,
                                jobs::now(),
                            ),
                        )?,
                        witness::Proof::Unspawned => traffic::recover_unspawned(
                            connection,
                            &traffic::UnspawnedEvidence::gather(
                                &candidate.job,
                                receipt.key(),
                                jobs::now(),
                            ),
                        )?,
                        witness::Proof::SpawnFailed(outcome) => traffic::recover_unspawned(
                            connection,
                            &traffic::UnspawnedEvidence::gather_spawn_failure(
                                &candidate.job,
                                receipt.key(),
                                outcome,
                                jobs::now(),
                            ),
                        )?,
                    };
                    if applied {
                        receipt.discard();
                        continue;
                    }
                }
                traffic::reconcile_uncertain(
                    connection,
                    &traffic::UncertainEvidence::gather(&candidate.job, jobs::now(), grace_ms()),
                )?;
                continue;
            }
            let evidence = storage::observe(&candidate.job, jobs::now());
            if evidence.traffic_recovery_ready() {
                storage::apply_observation(connection, &evidence)?;
            } else {
                traffic::reconcile_uncertain(
                    connection,
                    &traffic::UncertainEvidence::gather(&candidate.job, jobs::now(), grace_ms()),
                )?;
            }
        }
        if count < 64 {
            break;
        }
    }
    Ok(())
}

fn grace_ms() -> u64 {
    std::env::var("ARC_TRAFFIC_RECOVERY_GRACE_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0 && *value <= arc_ipc::jobs::MAX_TIMESTAMP)
        .unwrap_or(120_000)
}

pub(crate) async fn recover(client: &DbClient, deadline: Instant) -> Result<(), &'static str> {
    let head = match call(client, Operation::Head, deadline).await? {
        Value::Head(value) => value.map_err(JobError::code)?,
        _ => return Err(arc_ipc::ERROR_DB_BUSY),
    };
    if let Some(job) = head {
        let evidence = traffic::QueueObservation::gather(&job, jobs::now());
        match call(client, Operation::ReconcileQueue(evidence), deadline).await? {
            Value::Reply(value) => {
                value.map_err(JobError::code)?;
            }
            _ => return Err(arc_ipc::ERROR_DB_BUSY),
        }
    }
    let mut after = 0;
    loop {
        let candidates = match call(client, Operation::Candidates(after), deadline).await? {
            Value::Candidates(value) => value.map_err(JobError::code)?,
            _ => return Err(arc_ipc::ERROR_DB_BUSY),
        };
        let count = candidates.len();
        for candidate in candidates {
            after = candidate.sequence;
            if candidate.job.child.is_none() {
                if let Some(receipt) = witness::read(&candidate) {
                    let operation = match receipt.proof() {
                        witness::Proof::Child(child)
                            if probe_live(child) == IdentityStatus::Indeterminate =>
                        {
                            None
                        }
                        witness::Proof::Child(child) => Some(Operation::RecoverWitness(Box::new(
                            traffic::WitnessEvidence::gather(
                                &candidate.job,
                                receipt.key(),
                                child,
                                jobs::now(),
                            ),
                        ))),
                        witness::Proof::Unspawned => Some(Operation::RecoverUnspawned(Box::new(
                            traffic::UnspawnedEvidence::gather(
                                &candidate.job,
                                receipt.key(),
                                jobs::now(),
                            ),
                        ))),
                        witness::Proof::SpawnFailed(outcome) => Some(Operation::RecoverUnspawned(
                            Box::new(traffic::UnspawnedEvidence::gather_spawn_failure(
                                &candidate.job,
                                receipt.key(),
                                outcome,
                                jobs::now(),
                            )),
                        )),
                    };
                    if let Some(operation) = operation {
                        match call(client, operation, deadline).await? {
                            Value::Witness(value) => {
                                if value.map_err(JobError::code)? {
                                    receipt.discard();
                                    continue;
                                }
                            }
                            _ => return Err(arc_ipc::ERROR_DB_BUSY),
                        }
                    }
                }
                reconcile_unknown(client, &candidate.job, deadline).await?;
                continue;
            }
            let evidence = storage::observe(&candidate.job, jobs::now());
            if evidence.traffic_recovery_ready() {
                match jobs::call(client, jobs::Operation::Reconcile(evidence), deadline).await? {
                    jobs::Value::Reconcile(value) => {
                        value.map_err(JobError::code)?;
                    }
                    _ => return Err(arc_ipc::ERROR_DB_BUSY),
                }
            } else {
                reconcile_unknown(client, &candidate.job, deadline).await?;
            }
        }
        if count < 64 {
            break;
        }
    }
    Ok(())
}

async fn reconcile_unknown(
    client: &DbClient,
    job: &arc_ipc::jobs::JobSnapshot,
    deadline: Instant,
) -> Result<(), &'static str> {
    let evidence = traffic::UncertainEvidence::gather(job, jobs::now(), grace_ms());
    match call(
        client,
        Operation::RecoverUncertain(Box::new(evidence)),
        deadline,
    )
    .await?
    {
        Value::Uncertain(value) => {
            value.map_err(JobError::code)?;
            Ok(())
        }
        _ => Err(arc_ipc::ERROR_DB_BUSY),
    }
}

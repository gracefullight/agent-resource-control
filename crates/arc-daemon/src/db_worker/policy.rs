//! Bounded policy reads and launch metadata on the single writer.
use super::{jobs, traffic, DbClient};
use arc_ipc::{
    jobs::JobId,
    policy::launch::{LaunchRequest, LaunchResult},
};
use arc_policy::ObservationProvider;
use arc_storage::jobs::{traffic as repository, JobError as E};
use rusqlite::{Connection, TransactionBehavior};
use std::time::Instant;

pub(crate) use crate::connection::policy_dispatch::{Input, Value};
pub(crate) enum Operation {
    Current,
    Job(JobId),
    Diagnostics(JobId),
    Unspawned {
        request: LaunchRequest,
        owner: repository::OwnerEvidence,
    },
    Launch {
        request: LaunchRequest,
        owner: repository::OwnerEvidence,
        proof: Option<Box<repository::LineageProof>>,
    },
}
pub(crate) fn execute(
    connection: &mut Connection,
    operation: Operation,
    reader: Option<&super::observation::Observations>,
) -> Result<Value, E> {
    execute_with_clock(connection, operation, reader, Instant::now)
}
fn execute_with_clock(
    connection: &mut Connection,
    operation: Operation,
    reader: Option<&super::observation::Observations>,
    mut clock: impl FnMut() -> Instant,
) -> Result<Value, E> {
    let provider: &dyn ObservationProvider = reader
        .map(|r| r as &dyn ObservationProvider)
        .unwrap_or(&arc_policy::evaluator::MissingObservation);
    match operation {
        Operation::Unspawned { request, owner } => {
            repository::acknowledge_unspawned_launch(connection, &request, &owner)?;
            Ok(Value::Acknowledged)
        }
        Operation::Current => {
            arc_storage::policy::transaction(connection, provider, |tx, effective| {
                arc_storage::policy::record_effective(tx, effective, jobs::now())?;
                Ok(Value::Current(effective.clone()))
            })
        }
        Operation::Job(id) => {
            arc_storage::policy::transaction(connection, provider, |tx, effective| {
                arc_storage::policy::record_effective(tx, effective, jobs::now())?;
                arc_storage::policy::job(tx, &id, effective.clone(), jobs::now())
                    .map(|value| Value::Job(Box::new(value)))
            })
        }
        Operation::Diagnostics(id) => repository::diagnostics(
            connection,
            &id,
            &arc_traffic::Policy::default(),
            jobs::now(),
        )
        .map(|value| Value::Diagnostics(Box::new(value))),
        Operation::Launch {
            request,
            owner,
            proof,
        } => {
            let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let control_end =
                Instant::now() + std::time::Duration::from_millis(arc_policy::MAX_LAUNCH_FENCE_MS);
            let control = reader
                .map(|r| r.control_fence(control_end))
                .unwrap_or_else(|| Some((control_end, Box::new(()) as Box<dyn Send>)))
                .ok_or(E::Busy)?;
            // Resolve after IMMEDIATE, never before a possible writer backlog.
            let active = reader.map(|source| source.reader());
            let snapshot = active.as_ref().and_then(|source| source.as_ref());
            let (result, fence) = if let Some(reader) = snapshot {
                reader.with_launch_deadline(|input, deadline| {
                    let effective = arc_policy::evaluator::evaluate(input);
                    let result = (|| {
                        let result = repository::launch(
                            &tx,
                            &request,
                            &owner,
                            proof.as_deref(),
                            &effective,
                            jobs::now(),
                        )?;
                        if matches!(result, LaunchResult::Ready { .. })
                            && deadline.is_none_or(|end| clock() >= end)
                        {
                            return Err(E::Busy);
                        }
                        tx.commit()?;
                        Ok(result)
                    })();
                    let ready = matches!(result, Ok(LaunchResult::Ready { .. }));
                    (result, ready)
                })
            } else {
                // A daemon with no sampler explicitly uses L0; no native
                // publication exists to fence. Keep the same absolute IPC bound.
                let mut effective = None;
                arc_policy::evaluator::MissingObservation.with_observation(&mut |input| {
                    effective = Some(arc_policy::evaluator::evaluate(input))
                });
                let end = Instant::now()
                    + std::time::Duration::from_millis(arc_policy::MAX_LAUNCH_FENCE_MS);
                let fence = match reader {
                    Some(source) => source.fallback_fence(end),
                    None => Some((end, Box::new(()) as Box<dyn Send>)),
                };
                let result = repository::launch(
                    &tx,
                    &request,
                    &owner,
                    proof.as_deref(),
                    &effective.ok_or(E::StorageFailure)?,
                    jobs::now(),
                )?;
                if matches!(result, LaunchResult::Ready { .. })
                    && (fence.is_none() || clock() >= end)
                {
                    return Err(E::Busy);
                }
                tx.commit()?;
                let fence = matches!(result, LaunchResult::Ready { .. })
                    .then_some(fence)
                    .flatten();
                (Ok(result), fence)
            };
            let result = result?;
            let fence = fence.map(|(end, fence)| {
                (
                    end.min(control_end),
                    Box::new((control, fence)) as Box<dyn Send>,
                )
            });
            if matches!(result, LaunchResult::Ready { .. })
                && fence.as_ref().is_none_or(|(end, _)| clock() >= *end)
            {
                // No release left this writer. If COMMIT consumed the final
                // freshness budget, retract only this known-undelivered marker.
                // Cleanup failure is uncertainty, never a retryable rejection.
                drop(fence);
                repository::discard_unreleased_launch(connection, &request)
                    .map_err(|_| E::InvalidTransition)?;
                return Err(E::Busy);
            }
            Ok(Value::Launch { result, fence })
        }
    }
}
pub(crate) async fn handle(
    client: DbClient,
    input: Input,
    peer: u32,
    deadline: Instant,
) -> Result<Value, &'static str> {
    if let Input::Governor(input) = input {
        return super::governor::handle(client, input, peer, deadline)
            .await
            .map(Value::Governor);
    }
    let unspawned = matches!(&input, Input::Unspawned(_));
    let operation = match input {
        Input::Governor(_) => unreachable!(),
        Input::Current => Operation::Current,
        Input::Job(id) => Operation::Job(id),
        Input::Diagnostics(id) => Operation::Diagnostics(id),
        Input::Launch(request) | Input::Unspawned(request) => {
            use arc_core::process_identity::{probe_live, IdentityStatus};
            if request.observer.pid != peer
                || probe_live(&request.observer) != IdentityStatus::VerifiedMatch
            {
                return Err(arc_ipc::ERROR_INVALID_REQUEST);
            }
            if unspawned {
                let owner = repository::OwnerEvidence::gather(&request.observer);
                return match jobs::call(
                    &client,
                    jobs::Operation::Policy(Operation::Unspawned { request, owner }),
                    deadline,
                )
                .await?
                {
                    jobs::Value::Policy(value) => value.map_err(E::code),
                    _ => Err(arc_ipc::ERROR_DB_BUSY),
                };
            }
            let proof = traffic::lineage_proof(
                &client,
                &arc_ipc::traffic::JobRequest::Poll {
                    job_id: request.job_id.clone(),
                    observer: request.observer.clone(),
                },
                deadline,
            )
            .await?;
            let owner = repository::OwnerEvidence::gather(&request.observer);
            Operation::Launch {
                request,
                owner,
                proof: proof.map(Box::new),
            }
        }
    };
    match jobs::call(&client, jobs::Operation::Policy(operation), deadline).await? {
        jobs::Value::Policy(value) => value.map_err(E::code),
        _ => Err(arc_ipc::ERROR_DB_BUSY),
    }
}

#[cfg(all(test, target_os = "macos"))]
#[path = "policy_tests.rs"]
pub(crate) mod tests;

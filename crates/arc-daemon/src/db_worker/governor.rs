//! Existing bounded writer adapter for Governor state and negotiated execution.
use super::{jobs, traffic, DbClient};
pub(crate) use crate::connection::governor_dispatch::{Input, Value};
use arc_ipc::{governor::*, traffic::JobRequest};
use arc_policy::ObservationProvider;
use arc_storage::{
    governor as state,
    jobs::{
        traffic::{self as repository, governor as execution},
        JobError as E,
    },
};
use rusqlite::{Connection, TransactionBehavior};
use std::time::{Duration, Instant};

#[path = "governor/execution.rs"]
mod transaction;
pub(crate) use transaction::{execute, Operation};

async fn call(client: &DbClient, op: Operation, end: Instant) -> Result<Value, &'static str> {
    match jobs::call(client, jobs::Operation::Governor(op), end).await? {
        jobs::Value::Governor(r) => r.map_err(E::code),
        _ => Err(arc_ipc::ERROR_DB_BUSY),
    }
}
/// [`call`], also returning the footprint reading its own commit made due.
async fn watched(
    client: &DbClient,
    op: Operation,
    job: &arc_ipc::jobs::JobId,
    bound: super::footprint::Bound,
    end: Instant,
) -> Result<(Value, Option<arc_storage::footprint::Point>), &'static str> {
    let (value, point) =
        super::footprint::watched(client, jobs::Operation::Governor(op), job, bound, end).await?;
    match value {
        jobs::Value::Governor(r) => Ok((r.map_err(E::code)?, point)),
        _ => Err(arc_ipc::ERROR_DB_BUSY),
    }
}
fn authenticate(observer: &arc_ipc::jobs::ProcessIdentity, peer: u32) -> Result<(), &'static str> {
    if observer.pid != peer
        || arc_core::process_identity::probe_live(observer)
            != arc_core::process_identity::IdentityStatus::VerifiedMatch
    {
        return Err(arc_ipc::ERROR_INVALID_REQUEST);
    }
    Ok(())
}
pub(crate) async fn handle(
    client: DbClient,
    input: Input,
    peer: u32,
    end: Instant,
) -> Result<Value, &'static str> {
    #[cfg(test)]
    return handle_inner(client, input, peer, end, |_| {}).await;
    #[cfg(not(test))]
    handle_inner(client, input, peer, end).await
}

async fn handle_inner(
    client: DbClient,
    input: Input,
    peer: u32,
    end: Instant,
    #[cfg(test)] mut after_proof: impl FnMut(bool),
) -> Result<Value, &'static str> {
    let op = match input {
        Input::Get(q) => Operation::Get(q),
        Input::Set(q) => Operation::Set(q),
        Input::Job(GovernorJobRequest::Traffic { request }) => {
            if matches!(
                request,
                JobRequest::Enqueue { .. } | JobRequest::Poll { .. }
            ) {
                let Some((job_id, observer)) = super::footprint::traffic_job(&request) else {
                    unreachable!()
                };
                authenticate(observer, peer)?;
                let _footprint_guard = super::footprint::guard(job_id, end).await?;
                traffic::recover_for_governor(&client, end).await?;
                // Match the legacy route's bounded stale-proof recovery.
                // Native ancestry/owner evidence is refreshed outside SQLite;
                // all attempts retain the original request ID and deadline.
                for _ in 0..3 {
                    let proof = traffic::lineage_proof(&client, &request, end)
                        .await?
                        .and_then(|p| p.observe_before_write(observer))
                        .map(Box::new);
                    let had_proof = proof.is_some();
                    #[cfg(test)]
                    after_proof(had_proof);
                    let owner = repository::OwnerEvidence::gather(observer)
                        .with_meter_evidence(super::job_meter::evidence(observer, None));
                    let context = traffic::context().map_err(|_| arc_ipc::ERROR_DB_BUSY)?;
                    let op = if matches!(request, JobRequest::Enqueue { .. }) {
                        Operation::Enqueue {
                            request: request.clone(),
                            owner,
                            context,
                            proof,
                        }
                    } else {
                        Operation::Poll {
                            request: request.clone(),
                            owner,
                            context,
                            proof,
                        }
                    };
                    match watched(&client, op, job_id, Default::default(), end).await {
                        Err(code) if had_proof && code == E::IdentityUnavailable.code() => {
                            continue;
                        }
                        result => {
                            let (value, point) = result?;
                            super::footprint::after_commit(&client, job_id, observer, point, end)
                                .await;
                            return Ok(value);
                        }
                    }
                }
                return Err(arc_ipc::ERROR_DB_BUSY);
            } else {
                let status = matches!(
                    request,
                    JobRequest::Status { .. } | JobRequest::StatusV2 { .. }
                );
                let value = traffic::handle(client, request, peer, end).await?;
                let reply = if status {
                    GovernorJobResult::Status {
                        result: serde_json::from_value(value)
                            .map_err(|_| arc_ipc::ERROR_DB_BUSY)?,
                    }
                } else {
                    GovernorJobResult::Traffic {
                        result: serde_json::from_value(value)
                            .map_err(|_| arc_ipc::ERROR_DB_BUSY)?,
                    }
                };
                return Ok(Value::Job(reply));
            }
        }
        Input::Job(request @ GovernorJobRequest::Attach { .. }) => {
            let GovernorJobRequest::Attach {
                job_id,
                observer,
                child,
                ..
            } = &request
            else {
                unreachable!()
            };
            authenticate(observer, peer)?;
            let _footprint_guard = super::footprint::guard(job_id, end).await?;
            if !traffic::is_descendant(child, observer)
                && !super::job_meter::owned_child(child, observer)
            {
                return Err(arc_ipc::ERROR_INVALID_REQUEST);
            }
            let evidence = repository::ChildEvidence::gather(child)
                .with_meter_evidence(super::job_meter::evidence(observer, Some(child)));
            return call(&client, Operation::Attach(request, evidence), end).await;
        }
        Input::Job(request @ GovernorJobRequest::Complete { .. }) => {
            let GovernorJobRequest::Complete {
                job_id, observer, ..
            } = &request
            else {
                unreachable!()
            };
            authenticate(observer, peer)?;
            let (job_id, observer) = (job_id.clone(), observer.clone());
            let _footprint_guard = super::footprint::guard(&job_id, end).await?;
            // The end reading precedes the completion that ends the release.
            let bound = super::footprint::before_end(&job_id, &observer, end).await?;
            let (value, _) =
                watched(&client, Operation::Complete(request), &job_id, bound, end).await?;
            return Ok(value);
        }
        Input::Launch(request) => {
            authenticate(request.observer(), peer)?;
            let (job_id, observer) = (request.job_id().clone(), request.observer().clone());
            let _footprint_guard = super::footprint::guard(&job_id, end).await?;
            let proof = traffic::lineage_proof(
                &client,
                &JobRequest::Poll {
                    job_id: request.job_id().clone(),
                    observer: request.observer().clone(),
                },
                end,
            )
            .await?
            .and_then(|p| p.observe_before_write(request.observer()))
            .map(Box::new);
            let owner = repository::OwnerEvidence::gather(request.observer());
            let operation = Operation::Launch {
                request,
                owner,
                proof,
                release_id: nonce()?,
                end: end.min(
                    client.shared.observation.now()
                        + Duration::from_millis(MAX_LAUNCH_VALID_FOR_MS),
                ),
            };
            let (value, point) =
                watched(&client, operation, &job_id, Default::default(), end).await?;
            // The walk ends within the launch lease the reply holds.
            if let Value::Launch {
                result: GovernorLaunchResult::Unreserved { .. },
                fence: Some((until, _)),
            } = &value
            {
                let walk_end = end.min(*until);
                super::footprint::after_commit(&client, &job_id, &observer, point, walk_end).await;
            }
            return Ok(value);
        }
        Input::Unspawned { request, result } => {
            authenticate(request.observer(), peer)?;
            let _footprint_guard = super::footprint::guard(request.job_id(), end).await?;
            match result {
                GovernorLaunchResult::Unreserved { release_id, .. } => {
                    return call(
                        &client,
                        Operation::Unspawned {
                            id: request.job_id().clone(),
                            observer: request.observer().clone(),
                            token: release_id,
                            owner: repository::OwnerEvidence::gather(request.observer()),
                        },
                        end,
                    )
                    .await;
                }
                GovernorLaunchResult::Reserved { .. } => {
                    let GovernorLaunchRequest::Reserved { request } = request else {
                        return Err(arc_ipc::ERROR_INVALID_REQUEST);
                    };
                    let owner = repository::OwnerEvidence::gather(&request.observer);
                    match jobs::call(
                        &client,
                        jobs::Operation::Policy(super::policy::Operation::Unspawned {
                            request,
                            owner,
                        }),
                        end,
                    )
                    .await?
                    {
                        jobs::Value::Policy(r) => {
                            r.map_err(E::code)?;
                        }
                        _ => return Err(arc_ipc::ERROR_DB_BUSY),
                    }
                    return Ok(Value::Acknowledged);
                }
                _ => return Err(arc_ipc::ERROR_INVALID_REQUEST),
            }
        }
    };
    call(&client, op, end).await
}
fn nonce() -> Result<String, &'static str> {
    use std::io::Read;
    let mut bytes = [0; 16];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut f| f.read_exact(&mut bytes))
        .map_err(|_| arc_ipc::ERROR_DB_BUSY)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

#[cfg(all(test, target_os = "macos"))]
#[path = "governor_tests.rs"]
mod tests;

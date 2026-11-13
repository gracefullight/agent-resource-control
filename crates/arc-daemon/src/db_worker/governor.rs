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
                let observer = match &request {
                    JobRequest::Enqueue { observer, .. } | JobRequest::Poll { observer, .. } => {
                        observer
                    }
                    _ => unreachable!(),
                };
                authenticate(observer, peer)?;
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
                    match call(&client, op, end).await {
                        Err(code) if had_proof && code == E::IdentityUnavailable.code() => {
                            continue;
                        }
                        result => return result,
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
                observer, child, ..
            } = &request
            else {
                unreachable!()
            };
            authenticate(observer, peer)?;
            if !traffic::is_descendant(child, observer)
                && !super::job_meter::owned_child(child, observer)
            {
                return Err(arc_ipc::ERROR_INVALID_REQUEST);
            }
            let evidence = repository::ChildEvidence::gather(child)
                .with_meter_evidence(super::job_meter::evidence(observer, Some(child)));
            Operation::Attach(request, evidence)
        }
        Input::Job(request @ GovernorJobRequest::Complete { .. }) => {
            let GovernorJobRequest::Complete { observer, .. } = &request else {
                unreachable!()
            };
            authenticate(observer, peer)?;
            Operation::Complete(request)
        }
        Input::Launch(request) => {
            authenticate(request.observer(), peer)?;
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
            Operation::Launch {
                request,
                owner,
                proof,
                release_id: nonce()?,
                end: end.min(Instant::now() + Duration::from_millis(MAX_LAUNCH_VALID_FOR_MS)),
            }
        }
        Input::Unspawned { request, result } => {
            authenticate(request.observer(), peer)?;
            match result {
                GovernorLaunchResult::Unreserved { release_id, .. } => Operation::Unspawned {
                    id: request.job_id().clone(),
                    observer: request.observer().clone(),
                    token: release_id,
                    owner: repository::OwnerEvidence::gather(request.observer()),
                },
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

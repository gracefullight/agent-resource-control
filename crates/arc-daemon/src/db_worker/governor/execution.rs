//! Transactional Governor operations on the existing writer.
use super::*;

pub(crate) enum Operation {
    Get(GetGovernorSwitchRequest),
    Set(SetGovernorSwitchRequest),
    Enqueue {
        request: JobRequest,
        owner: repository::OwnerEvidence,
        context: arc_ipc::traffic::LineageContext,
        proof: Option<Box<repository::LineageProof>>,
    },
    Poll {
        request: JobRequest,
        owner: repository::OwnerEvidence,
        context: arc_ipc::traffic::LineageContext,
        proof: Option<Box<repository::LineageProof>>,
    },
    Attach(GovernorJobRequest, repository::ChildEvidence),
    Complete(GovernorJobRequest),
    Launch {
        request: GovernorLaunchRequest,
        owner: repository::OwnerEvidence,
        proof: Option<Box<repository::LineageProof>>,
        release_id: String,
        end: Instant,
    },
    Unspawned {
        id: arc_ipc::jobs::JobId,
        observer: arc_ipc::jobs::ProcessIdentity,
        token: String,
        owner: repository::OwnerEvidence,
    },
}
pub(crate) fn execute(
    connection: &mut Connection,
    operation: Operation,
    reader: Option<&super::super::observation::Observations>,
) -> Result<Value, E> {
    let provider: &dyn ObservationProvider = reader
        .map(|r| r as &dyn ObservationProvider)
        .unwrap_or(&arc_policy::evaluator::MissingObservation);
    match operation {
        Operation::Get(query) => {
            let tx = connection.transaction()?;
            let result = state::read_switch(&tx, &query)?;
            tx.commit()?;
            Ok(Value::Get(result))
        }
        Operation::Set(request) => {
            let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let _control = provider.try_control().ok_or(E::Busy)?;
            let result = state::set_switch(&tx, &request, jobs::now())?;
            tx.commit()?;
            Ok(Value::Set(result))
        }
        Operation::Enqueue {
            request,
            owner,
            context,
            proof,
        } => execution::enqueue(
            connection,
            &request,
            &owner,
            &arc_traffic::Policy::default(),
            &context,
            proof.as_deref(),
            jobs::now(),
            provider,
        )
        .map(Value::Job),
        Operation::Poll {
            request,
            owner,
            context,
            proof,
        } => execution::poll(
            connection,
            &request,
            &owner,
            &arc_traffic::Policy::default(),
            &context,
            proof.as_deref(),
            jobs::now(),
            provider,
        )
        .map(Value::Job),
        Operation::Attach(
            GovernorJobRequest::Attach {
                job_id,
                observer,
                release_id,
                child,
            },
            evidence,
        ) => execution::attach(
            connection,
            &job_id,
            &observer,
            &release_id,
            &child,
            &evidence,
            jobs::now(),
        )
        .map(Value::Job),
        Operation::Complete(GovernorJobRequest::Complete {
            job_id,
            observer,
            release_id,
            outcome,
        }) => execution::complete(
            connection,
            &job_id,
            &observer,
            &release_id,
            &outcome,
            jobs::now(),
        )
        .map(Value::Job),
        Operation::Unspawned {
            id,
            observer,
            token,
            owner,
        } => {
            execution::unspawned(connection, &id, &observer, &token, &owner, jobs::now())?;
            Ok(Value::Acknowledged)
        }
        Operation::Launch {
            request,
            owner,
            proof,
            release_id,
            end,
        } => {
            let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let control = reader
                .map(|r| r.control_fence(end))
                .unwrap_or_else(|| Some((end, Box::new(()) as Box<dyn Send>)))
                .ok_or(E::Busy)?;
            if Instant::now() >= end {
                return Err(E::Busy);
            }
            let Some(job) = arc_storage::jobs::get(&tx, request.job_id())? else {
                return Ok(Value::Launch {
                    result: GovernorLaunchResult::NotFound {},
                    fence: None,
                });
            };
            if job.observer != *request.observer() {
                return Err(E::Conflict);
            }
            let status = repository::status(&tx, request.job_id())?;
            if matches!(
                status,
                arc_ipc::traffic::StatusResult::Cancelled {}
                    | arc_ipc::traffic::StatusResult::Abandoned {}
            ) {
                return Ok(Value::Launch {
                    result: GovernorLaunchResult::Cancelled {},
                    fence: None,
                });
            }
            if job.child.is_some() || job.outcome.is_some() {
                return Ok(Value::Launch {
                    result: GovernorLaunchResult::Unconfirmed {},
                    fence: None,
                });
            }
            if execution::prepare(&tx, request.job_id(), jobs::now())? {
                if let GovernorLaunchRequest::Reserved { request: reserved } = &request {
                    if execution::disposition(&tx, request.job_id())?
                        .and_then(|d| d.previous_generation)
                        != Some(reserved.permit_generation)
                    {
                        return Err(E::Conflict);
                    }
                }
                let mut result = execution::claim(
                    &tx,
                    request.job_id(),
                    request.observer(),
                    &owner,
                    &release_id,
                    jobs::now(),
                )?;
                tx.commit()?;
                if let GovernorLaunchResult::Unreserved { valid_for_ms, .. } = &mut result {
                    let remaining =
                        end.saturating_duration_since(Instant::now()).as_millis() as u64;
                    // Nothing left this writer on an expired release. Cleanup
                    // uses a new short transaction after dropping both leases.
                    if remaining == 0 {
                        drop(control);
                        execution::unspawned(
                            connection,
                            request.job_id(),
                            request.observer(),
                            &release_id,
                            &owner,
                            jobs::now(),
                        )?;
                        return Err(E::Busy);
                    }
                    *valid_for_ms = remaining.min(MAX_LAUNCH_VALID_FOR_MS);
                }
                let fence =
                    matches!(result, GovernorLaunchResult::Unreserved { .. }).then_some(control);
                return Ok(Value::Launch { result, fence });
            }
            let GovernorLaunchRequest::Reserved { request } = &request else {
                tx.commit()?;
                return Ok(Value::Launch {
                    result: GovernorLaunchResult::Retry {},
                    fence: None,
                });
            };
            let delivered:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM audit_events WHERE id=?1 AND event_type='TRAFFIC_LAUNCH_FENCE')",[format!("traffic-launch-{}",request.job_id.as_str())],|r|r.get(0))?;
            if delivered {
                return Ok(Value::Launch {
                    result: GovernorLaunchResult::Unconfirmed {},
                    fence: None,
                });
            }
            if execution::reservation_invalidated(&tx, request)? {
                tx.commit()?;
                return Ok(Value::Launch {
                    result: GovernorLaunchResult::Retry {},
                    fence: None,
                });
            }
            // Same transaction and common lease as mode selection. Publication
            // lease is acquired afterwards and survives COMMIT through ACK.
            let active = reader.map(|r| r.reader());
            let source = active.as_ref().and_then(|r| r.as_ref());
            let (result, fence) = if let Some(source) = source {
                source.with_launch_deadline(|input, deadline| {
                    let result = (|| {
                        let effective = arc_policy::evaluator::evaluate(input);
                        let result = repository::launch(
                            &tx,
                            request,
                            &owner,
                            proof.as_deref(),
                            &effective,
                            jobs::now(),
                        )?;
                        if matches!(result, arc_ipc::policy::launch::LaunchResult::Ready { .. })
                            && deadline.is_none_or(|d| Instant::now() >= d.min(end))
                        {
                            return Err(E::Busy);
                        }
                        tx.commit()?;
                        Ok(result)
                    })();
                    let ready = matches!(
                        result,
                        Ok(arc_ipc::policy::launch::LaunchResult::Ready { .. })
                    );
                    (result, ready)
                })
            } else {
                let fence = reader
                    .map(|r| r.fallback_fence(end))
                    .unwrap_or_else(|| Some((end, Box::new(()) as Box<dyn Send>)));
                let mut result = Err(E::StorageFailure);
                arc_policy::evaluator::MissingObservation.with_observation(&mut |input| {
                    result = repository::launch(
                        &tx,
                        request,
                        &owner,
                        proof.as_deref(),
                        &arc_policy::evaluator::evaluate(input),
                        jobs::now(),
                    );
                });
                let result = result?;
                if matches!(result, arc_ipc::policy::launch::LaunchResult::Ready { .. })
                    && (fence.is_none() || Instant::now() >= end)
                {
                    return Err(E::Busy);
                }
                tx.commit()?;
                let ready = matches!(result, arc_ipc::policy::launch::LaunchResult::Ready { .. });
                (Ok(result), if ready { fence } else { None })
            };
            let result = result?;
            let fence =
                fence.map(|(at, fence)| (at.min(end), Box::new((control, fence)) as Box<dyn Send>));
            if matches!(result, arc_ipc::policy::launch::LaunchResult::Ready { .. })
                && fence.as_ref().is_none_or(|(at, _)| Instant::now() >= *at)
            {
                drop(fence);
                repository::discard_unreleased_launch(connection, request)?;
                return Err(E::Busy);
            }
            Ok(Value::Launch {
                result: GovernorLaunchResult::Reserved {
                    result: Box::new(result),
                },
                fence,
            })
        }
        _ => Err(E::InvalidRequest),
    }
}

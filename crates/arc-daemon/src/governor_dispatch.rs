//! Additive protocol-1 operations, on existing owner-authenticated admission.
use super::policy_dispatch::{self, Fence, Handler};
use crate::{limits::RuntimeLimits, runtime_metrics::RuntimeMetrics, transport};
use arc_ipc::{governor::*, Request, Response, PROTOCOL_VERSION};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::net::UnixStream;

pub(crate) enum Input {
    Get(GetGovernorSwitchRequest),
    Set(SetGovernorSwitchRequest),
    Job(GovernorJobRequest),
    Launch(GovernorLaunchRequest),
    Unspawned {
        request: GovernorLaunchRequest,
        result: GovernorLaunchResult,
    },
}
pub(crate) enum Value {
    Get(GovernorSwitchReadResult),
    Set(GovernorMutationResult),
    Job(GovernorJobResult),
    Launch {
        result: GovernorLaunchResult,
        fence: Option<Fence>,
    },
    Acknowledged,
}
pub(crate) async fn dispatch(
    stream: &UnixStream,
    request: &Request,
    limits: &RuntimeLimits,
    handler: Option<&Handler>,
    metrics: &Arc<RuntimeMetrics>,
) -> bool {
    if request.protocol_version != PROTOCOL_VERSION {
        return false;
    }
    if !matches!(
        request.operation.as_str(),
        OPERATION_CAPABILITIES
            | OPERATION_GET_SWITCH
            | OPERATION_SET_SWITCH
            | OPERATION_PAUSE
            | OPERATION_RESUME
            | OPERATION_JOB
            | OPERATION_LAUNCH
            | OPERATION_LAUNCH_ACK
    ) {
        return false;
    }
    let end = Instant::now()
        + if request.operation == OPERATION_LAUNCH {
            Duration::from_millis(MAX_LAUNCH_VALID_FOR_MS)
        } else {
            limits.db_response_timeout
        };
    let send = |r: Response| async move { policy_dispatch::write(stream, &r, end, metrics).await };
    let Some(handler) = handler else {
        send(Response::error(
            &request.request_id,
            arc_ipc::ERROR_UNSUPPORTED_CAPABILITY,
        ))
        .await;
        return true;
    };
    if request.operation == OPERATION_CAPABILITIES {
        send(capabilities_response(request)).await;
        return true;
    }
    let input = match request.operation.as_str() {
        OPERATION_GET_SWITCH => decode_get_switch(&request.payload).map(Input::Get),
        OPERATION_SET_SWITCH | OPERATION_PAUSE | OPERATION_RESUME => {
            decode_set_switch(&request.operation, &request.payload).map(Input::Set)
        }
        OPERATION_JOB => decode_job(&request.payload).map(Input::Job),
        OPERATION_LAUNCH => decode_launch(&request.payload).map(Input::Launch),
        _ => Err(arc_ipc::ERROR_INVALID_REQUEST),
    };
    let launch_request = match &input {
        Ok(Input::Launch(q)) => Some(q.clone()),
        _ => None,
    };
    let mutation = matches!(&input, Ok(Input::Set(_)));
    #[cfg(test)]
    let bun_enqueue = matches!(
        &input,
        Ok(Input::Job(GovernorJobRequest::Traffic {
            request: arc_ipc::traffic::JobRequest::Enqueue { classification, .. }
        })) if classification.reason == arc_ipc::Reason::BunTest
    );
    let work_end = end
        - limits
            .write_timeout
            .min(end.saturating_duration_since(Instant::now()) / 2);
    let result = match (input, crate::platform::peer_pid(stream)) {
        (Err(e), _) => Err(e),
        (_, Err(_)) => Err(arc_ipc::ERROR_INVALID_REQUEST),
        (Ok(input), Ok(peer)) => tokio::time::timeout_at(
            tokio::time::Instant::from_std(work_end),
            handler(policy_dispatch::Input::Governor(input), peer, work_end),
        )
        .await
        .unwrap_or(Err(arc_ipc::ERROR_DB_BUSY)),
    };
    match result {
        Err(code) => {
            let code = if code == arc_ipc::ERROR_DB_BUSY {
                if mutation {
                    ERROR_WRITE_UNCERTAIN
                } else if request.operation == OPERATION_GET_SWITCH {
                    ERROR_UNAVAILABLE
                } else {
                    code
                }
            } else {
                code
            };
            send(Response::error(&request.request_id, code)).await;
        }
        Ok(policy_dispatch::Value::Governor(value)) => match value {
            Value::Get(v) => {
                send(Response::ok(&request.request_id, serde_json::json!(v))).await;
            }
            Value::Set(v) => {
                send(Response::ok(&request.request_id, serde_json::json!(v))).await;
            }
            Value::Job(v) => {
                #[cfg(test)]
                if bun_enqueue
                    && matches!(
                        &v,
                        GovernorJobResult::Traffic {
                            result: arc_ipc::traffic::JobResult::Granted { .. }
                        }
                    )
                    && crate::platform::peer_pid(stream).is_ok_and(super::drop_bun_grant_for_test)
                {
                    return true;
                }
                send(Response::ok(&request.request_id, serde_json::json!(v))).await;
            }
            Value::Acknowledged => {
                send(Response::ok(&request.request_id, serde_json::json!({}))).await;
            }
            Value::Launch {
                mut result,
                mut fence,
            } => {
                let launch_end = fence.as_ref().map_or(end, |(at, _)| (*at).min(end));
                if let GovernorLaunchResult::Unreserved { valid_for_ms, .. } = &mut result {
                    let left = launch_end
                        .saturating_duration_since(Instant::now())
                        .as_millis() as u64;
                    if left == 0 {
                        drop(fence);
                        send(Response::error(&request.request_id, arc_ipc::ERROR_DB_BUSY)).await;
                        return true;
                    }
                    *valid_for_ms = left.min(*valid_for_ms);
                }
                let sent = policy_dispatch::write(
                    stream,
                    &Response::ok(&request.request_id, serde_json::json!(result)),
                    launch_end,
                    metrics,
                )
                .await;
                if sent && fence.is_some() {
                    if let Some(original) = launch_request {
                        let ack_end = end
                            + Duration::from_millis(arc_ipc::policy::launch::LAUNCH_ACK_GRACE_MS);
                        let ack = {
                            let pending = read_ack(stream, &original, &result, ack_end, metrics);
                            tokio::pin!(pending);
                            tokio::select! {result=&mut pending=>result,_=tokio::time::sleep_until(tokio::time::Instant::from_std(launch_end))=>{drop(fence.take());pending.await}}
                        };
                        // Drop launch and publication leases before writer cleanup.
                        drop(fence.take());
                        if let Ok((id, ack)) = ack {
                            let unspawned = matches!(
                                ack,
                                GovernorLaunchAck::Reserved {
                                    ack: arc_ipc::policy::launch::LaunchAck::Unspawned { .. }
                                } | GovernorLaunchAck::Unreserved {
                                    ack: GovernorUnreservedAck::Unspawned { .. }
                                }
                            );
                            let result = if unspawned {
                                match crate::platform::peer_pid(stream) {
                                    Ok(peer) => tokio::time::timeout_at(
                                        tokio::time::Instant::from_std(ack_end),
                                        handler(
                                            policy_dispatch::Input::Governor(Input::Unspawned {
                                                request: original,
                                                result,
                                            }),
                                            peer,
                                            ack_end,
                                        ),
                                    )
                                    .await
                                    .unwrap_or(Err(arc_ipc::ERROR_DB_BUSY))
                                    .map(|_| ()),
                                    Err(_) => Err(arc_ipc::ERROR_INVALID_REQUEST),
                                }
                            } else {
                                Ok(())
                            };
                            let response = match result {
                                Ok(()) => Response::ok(id, serde_json::json!({})),
                                Err(e) => Response::error(id, e),
                            };
                            policy_dispatch::write(stream, &response, ack_end, metrics).await;
                        }
                    }
                }
                drop(fence);
            }
        },
        _ => {
            send(Response::error(&request.request_id, arc_ipc::ERROR_DB_BUSY)).await;
        }
    }
    true
}
async fn read_ack(
    stream: &UnixStream,
    expected: &GovernorLaunchRequest,
    result: &GovernorLaunchResult,
    end: Instant,
    metrics: &Arc<RuntimeMetrics>,
) -> Result<(String, GovernorLaunchAck), ()> {
    let end = tokio::time::Instant::from_std(end);
    let size = transport::read_header(stream, end).await.map_err(|_| ())?;
    if size > MAX_EXECUTION_BYTES as u32 + 1024 {
        return Err(());
    }
    let _body = metrics.enter(
        crate::runtime_metrics::Gauge::RequestBodyBytes,
        size as usize,
    );
    let body = transport::read_body(stream, size, end)
        .await
        .map_err(|_| ())?;
    let request = Request::decode(&body).map_err(|_| ())?;
    if request.protocol_version != PROTOCOL_VERSION || request.operation != OPERATION_LAUNCH_ACK {
        return Err(());
    }
    let ack = decode_launch_ack(&request.payload, expected, result).map_err(|_| ())?;
    Ok((request.request_id, ack))
}

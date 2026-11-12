//! Owner-authenticated additive policy dispatch and same-connection launch ACK.
use crate::{
    limits::RuntimeLimits,
    runtime_metrics::{Counter, Gauge, RuntimeMetrics},
    transport,
};
use arc_ipc::{
    policy::{self, launch},
    Request, Response, PROTOCOL_VERSION,
};
use std::{
    future::Future,
    pin::Pin,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::net::UnixStream;

pub(crate) enum Input {
    Current,
    Job(arc_ipc::jobs::JobId),
    Launch(launch::LaunchRequest),
    Unspawned(launch::LaunchRequest),
}
pub(crate) type Fence = (Instant, Box<dyn Send>);
pub(crate) enum Value {
    Current(policy::EffectivePolicy),
    Job(Box<policy::JobPolicyResult>),
    Acknowledged,
    Launch {
        result: launch::LaunchResult,
        fence: Option<Fence>,
    },
}
pub(crate) type Handler = Arc<
    dyn Fn(Input, u32, Instant) -> Pin<Box<dyn Future<Output = Result<Value, &'static str>> + Send>>
        + Send
        + Sync,
>;

/// Return true when the operation was handled. Request body/connection/task
/// budgets are supplied by the existing transport admission, not another queue.
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
    let recognized = matches!(
        request.operation.as_str(),
        policy::OPERATION_POLICY_CAPABILITIES
            | policy::OPERATION_POLICY_CURRENT
            | policy::OPERATION_POLICY_JOB
            | launch::OPERATION_TRAFFIC_LAUNCH
            | launch::OPERATION_TRAFFIC_LAUNCH_ACK
    );
    if !recognized {
        return false;
    }
    let deadline = Instant::now()
        + if request.operation == launch::OPERATION_TRAFFIC_LAUNCH {
            Duration::from_millis(arc_policy::MAX_LAUNCH_FENCE_MS)
        } else {
            limits
                .db_response_timeout
                .min(Duration::from_millis(policy::POLICY_READ_MAX_BUDGET_MS))
        };
    let send =
        |response: Response| async move { write(stream, &response, deadline, metrics).await };
    let Some(handler) = handler else {
        send(Response::error(
            &request.request_id,
            arc_ipc::ERROR_UNSUPPORTED_CAPABILITY,
        ))
        .await;
        return true;
    };
    if request.operation == policy::OPERATION_POLICY_CAPABILITIES {
        send(if policy::decode_empty(&request.payload).is_ok() {
            policy::capabilities_response(request)
        } else {
            Response::error(&request.request_id, arc_ipc::ERROR_INVALID_REQUEST)
        })
        .await;
        return true;
    }
    let mut launch_request = None;
    let input = match request.operation.as_str() {
        policy::OPERATION_POLICY_CURRENT => {
            policy::decode_empty(&request.payload).map(|_| Input::Current)
        }
        policy::OPERATION_POLICY_JOB => {
            policy::decode_job_request(&request.payload).map(|q| Input::Job(q.job_id))
        }
        launch::OPERATION_TRAFFIC_LAUNCH => {
            launch::decode_launch_request(&request.payload).map(|q| {
                launch_request = Some(q.clone());
                Input::Launch(q)
            })
        }
        _ => Err(arc_ipc::ERROR_INVALID_REQUEST),
    };
    // Reserve response time within the same absolute budget. A timed-out
    // writer must still be able to report unavailable before that budget ends.
    let work_deadline = deadline
        - limits
            .write_timeout
            .min(deadline.saturating_duration_since(Instant::now()) / 2);
    let result = match (input, crate::platform::peer_pid(stream)) {
        (Err(code), _) => Err(code),
        (_, Err(_)) => Err(arc_ipc::ERROR_INVALID_REQUEST),
        (Ok(input), Ok(peer)) => tokio::time::timeout_at(
            tokio::time::Instant::from_std(work_deadline),
            handler(input, peer, work_deadline),
        )
        .await
        .unwrap_or(Err(arc_ipc::ERROR_DB_BUSY)),
    };
    match result {
        Err(code) => {
            let code = if launch_request.is_none() && code == arc_ipc::ERROR_DB_BUSY {
                policy::ERROR_POLICY_UNAVAILABLE
            } else {
                code
            };
            send(Response::error(&request.request_id, code)).await;
        }
        Ok(Value::Current(value)) => {
            send(policy::current_response(request, &value)).await;
        }
        Ok(Value::Job(value)) => {
            send(policy::job_response(request, &value)).await;
        }
        Ok(Value::Acknowledged) => {
            send(Response::ok(&request.request_id, serde_json::json!({}))).await;
        }
        Ok(Value::Launch { result, mut fence }) => {
            let end = fence
                .as_ref()
                .map(|(end, _)| *end)
                .unwrap_or(deadline)
                .min(deadline);
            let response = Response::ok(&request.request_id, serde_json::json!(result));
            if write(stream, &response, end, metrics).await && fence.is_some() {
                if let Some(launch_request) = launch_request {
                    let ack_end = deadline + Duration::from_millis(launch::LAUNCH_ACK_GRACE_MS);
                    // The cleanup grace never extends the publication lease.
                    // Retain partial frame state while the lease expires.
                    let ack = {
                        let pending = read_ack(stream, &launch_request, ack_end, metrics);
                        tokio::pin!(pending);
                        tokio::select! {
                        result = &mut pending => result,
                        _ = tokio::time::sleep_until(tokio::time::Instant::from_std(end)) => {
                            drop(fence.take());
                            pending.await
                        }
                        }
                    };
                    drop(fence.take());
                    if let Ok((id, ack)) = ack {
                        let result = if matches!(ack, launch::LaunchAck::Unspawned { .. }) {
                            match crate::platform::peer_pid(stream) {
                                Ok(peer) => tokio::time::timeout_at(
                                    tokio::time::Instant::from_std(ack_end),
                                    handler(Input::Unspawned(launch_request), peer, ack_end),
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
                            Err(code) => Response::error(id, code),
                        };
                        write(stream, &response, ack_end, metrics).await;
                    }
                }
            }
            drop(fence);
        }
    }
    true
}
async fn read_ack(
    stream: &UnixStream,
    expected: &launch::LaunchRequest,
    end: Instant,
    metrics: &Arc<RuntimeMetrics>,
) -> Result<(String, launch::LaunchAck), ()> {
    let end = tokio::time::Instant::from_std(end);
    let size = transport::read_header(stream, end).await.map_err(|_| ())?;
    if size > launch::MAX_LAUNCH_PAYLOAD_BYTES as u32 + 1024 {
        return Err(());
    }
    let _body = metrics.enter(Gauge::RequestBodyBytes, size as usize);
    let body = transport::read_body(stream, size, end)
        .await
        .map_err(|_| ())?;
    let request = Request::decode(&body).map_err(|_| ())?;
    if request.protocol_version != PROTOCOL_VERSION
        || request.operation != launch::OPERATION_TRAFFIC_LAUNCH_ACK
    {
        return Err(());
    }
    let ack = launch::decode_launch_ack(&request.payload, expected).map_err(|_| ())?;
    if let launch::LaunchAck::Started { child, .. } = &ack {
        if child.boot_id != expected.observer.boot_id {
            return Err(());
        }
    }
    Ok((request.request_id, ack))
}

async fn write(
    stream: &UnixStream,
    response: &Response,
    end: Instant,
    metrics: &Arc<RuntimeMetrics>,
) -> bool {
    let _bytes = metrics.enter(Gauge::ResponseBytes, arc_ipc::MAX_FRAME_BYTES as usize);
    match transport::write_response(stream, response, tokio::time::Instant::from_std(end)).await {
        Ok(()) => {
            metrics.increment(Counter::FramesCompleted);
            true
        }
        Err(error) => {
            if error.kind() == std::io::ErrorKind::TimedOut {
                metrics.increment(Counter::WriteTimeouts);
            }
            false
        }
    }
}

#[cfg(test)]
#[path = "policy_dispatch_tests.rs"]
mod tests;

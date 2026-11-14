//! Negotiated read on the same authenticated connection and bounded writer.
use super::{write, Handler, Input, Value};
use crate::{limits::RuntimeLimits, runtime_metrics::RuntimeMetrics};
use arc_ipc::{traffic::diagnostics as wire, Request, Response, PROTOCOL_VERSION};
use std::{sync::Arc, time::Instant};
use tokio::net::UnixStream;

pub(super) async fn dispatch(
    stream: &UnixStream,
    request: &Request,
    limits: &RuntimeLimits,
    handler: Option<&Handler>,
    metrics: &Arc<RuntimeMetrics>,
) -> bool {
    if request.protocol_version != PROTOCOL_VERSION
        || !matches!(
            request.operation.as_str(),
            wire::OPERATION_CAPABILITIES | wire::OPERATION_JOB
        )
    {
        return false;
    }
    let deadline = Instant::now() + limits.db_response_timeout;
    let send =
        |response: Response| async move { write(stream, &response, deadline, metrics).await };
    // An unavailable handler rejects the operation before any new vocabulary.
    let Some(handler) = handler else {
        send(Response::error(
            &request.request_id,
            arc_ipc::ERROR_UNSUPPORTED_CAPABILITY,
        ))
        .await;
        return true;
    };
    if request.operation == wire::OPERATION_CAPABILITIES {
        send(
            if arc_ipc::traffic::decode_probe(&request.payload).is_ok() {
                Response::ok(
                    &request.request_id,
                    serde_json::json!({"capability": wire::CAPABILITY}),
                )
            } else {
                Response::error(&request.request_id, arc_ipc::ERROR_INVALID_REQUEST)
            },
        )
        .await;
        return true;
    }
    let work_end = deadline
        - limits
            .write_timeout
            .min(deadline.saturating_duration_since(Instant::now()) / 2);
    let query = wire::decode_request(&request.payload);
    let result = match (query.as_ref(), crate::platform::peer_pid(stream)) {
        (Err(code), _) => Err(*code),
        (_, Err(_)) => Err(arc_ipc::ERROR_INVALID_REQUEST),
        (Ok(query), Ok(peer)) => tokio::time::timeout_at(
            tokio::time::Instant::from_std(work_end),
            handler(Input::Diagnostics(query.job_id.clone()), peer, work_end),
        )
        .await
        .unwrap_or(Err(arc_ipc::ERROR_DB_BUSY)),
    };
    let response = match result {
        Ok(Value::Diagnostics(value)) => {
            let payload = serde_json::json!(value);
            if wire::decode_result(&payload, &query.expect("validated query").job_id).is_ok() {
                Response::ok(&request.request_id, payload)
            } else {
                Response::error(&request.request_id, arc_ipc::ERROR_DB_BUSY)
            }
        }
        Err(code) => Response::error(&request.request_id, code),
        _ => Response::error(&request.request_id, arc_ipc::ERROR_DB_BUSY),
    };
    send(response).await;
    true
}

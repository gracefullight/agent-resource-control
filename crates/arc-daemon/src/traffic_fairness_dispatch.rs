//! Project fairness negotiation (AT-TRF-08) on the authenticated connection
//! and the bounded writer of the diagnostics read.
//!
//! Advertising the capability lets a client send the optional project key
//! with its enqueue. The decision read is the only reply that names an
//! additive reason; every other reader keeps the projected vocabulary.
use super::{write, Handler, Input, Value};
use crate::{limits::RuntimeLimits, runtime_metrics::RuntimeMetrics};
use arc_ipc::{traffic::fairness as wire, Request, Response, PROTOCOL_VERSION};
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
            wire::OPERATION_CAPABILITIES | wire::OPERATION_DECISION
        )
    {
        return false;
    }
    let deadline = Instant::now() + limits.db_response_timeout;
    let send =
        |response: Response| async move { write(stream, &response, deadline, metrics).await };
    let error = |code| Response::error(&request.request_id, code);
    // A daemon without the writer rejects both before any new vocabulary.
    let Some(handler) = handler else {
        send(error(arc_ipc::ERROR_UNSUPPORTED_CAPABILITY)).await;
        return true;
    };
    if request.operation == wire::OPERATION_CAPABILITIES {
        send(
            if arc_ipc::traffic::decode_probe(&request.payload).is_ok() {
                Response::ok(
                    &request.request_id,
                    serde_json::json!(arc_ipc::traffic::Capabilities::fairness_supported()),
                )
            } else {
                error(arc_ipc::ERROR_INVALID_REQUEST)
            },
        )
        .await;
        return true;
    }
    let work_end = deadline
        - limits
            .write_timeout
            .min(deadline.saturating_duration_since(Instant::now()) / 2);
    let response = match (
        wire::decode_request(&request.payload),
        crate::platform::peer_pid(stream),
    ) {
        (Err(code), _) => error(code),
        (_, Err(_)) => error(arc_ipc::ERROR_INVALID_REQUEST),
        (Ok(query), Ok(peer)) => {
            let read = tokio::time::timeout_at(
                tokio::time::Instant::from_std(work_end),
                handler(Input::Decision(query.job_id.clone()), peer, work_end),
            )
            .await
            .unwrap_or(Err(arc_ipc::ERROR_DB_BUSY));
            match read {
                Ok(Value::Decision(reason)) => Response::ok(
                    &request.request_id,
                    serde_json::json!(wire::DecisionResult {
                        schema_version: 1,
                        job_id: query.job_id,
                        reason,
                    }),
                ),
                Err(code) => error(code),
                Ok(_) => error(arc_ipc::ERROR_DB_BUSY),
            }
        }
    };
    send(response).await;
    true
}

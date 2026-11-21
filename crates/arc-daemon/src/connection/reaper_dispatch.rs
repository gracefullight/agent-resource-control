//! Additive, owner-authenticated registration of a wrapper's Reaper capability.
//!
//! Only the live observer may register, on its own direct connection. A relay,
//! a proxy or a daemon without a running Reaper leaves the job unregistered,
//! and an unregistered job is unknown ownership: never a candidate.
use crate::{limits::RuntimeLimits, runtime_metrics::RuntimeMetrics};
use arc_ipc::{reaper, Request, Response, PROTOCOL_VERSION};
use std::{future::Future, pin::Pin, sync::Arc, time::Instant};
use tokio::net::UnixStream;

pub(crate) type Handler = Arc<
    dyn Fn(
            reaper::RegisterRequest,
            Instant,
        ) -> Pin<Box<dyn Future<Output = Result<(), &'static str>> + Send>>
        + Send
        + Sync,
>;

/// Return true when the operation was handled.
pub(crate) async fn dispatch(
    stream: &UnixStream,
    request: &Request,
    limits: &RuntimeLimits,
    handler: &Option<Handler>,
    metrics: &Arc<RuntimeMetrics>,
) -> bool {
    if request.protocol_version != PROTOCOL_VERSION
        || request.operation != reaper::OPERATION_REGISTER
    {
        return false;
    }
    let deadline = Instant::now() + limits.db_response_timeout;
    let result = match (
        handler,
        reaper::decode_register(&request.payload),
        crate::platform::peer_pid(stream),
    ) {
        (None, ..) => Err(arc_ipc::ERROR_UNSUPPORTED_CAPABILITY),
        (_, Err(code), _) => Err(code),
        (Some(handler), Ok(input), Ok(peer)) if peer == input.observer.pid => {
            tokio::time::timeout_at(
                tokio::time::Instant::from_std(deadline),
                handler(input, deadline),
            )
            .await
            .unwrap_or(Err(arc_ipc::ERROR_DB_BUSY))
        }
        _ => Err(arc_ipc::ERROR_INVALID_REQUEST),
    };
    let response = match result {
        Ok(()) => Response::ok(&request.request_id, serde_json::json!({})),
        Err(code) => Response::error(&request.request_id, code),
    };
    let end = deadline + limits.write_timeout;
    super::policy_dispatch::write(stream, &response, end, metrics).await;
    true
}

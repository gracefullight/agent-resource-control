//! Additive job Meter dispatch; host snapshot and protocol-1 completions retain
//! their original decoders. Payload validation precedes database admission.
use crate::{
    limits::RuntimeLimits,
    runtime_metrics::{Counter, RuntimeMetrics},
};
use arc_ipc::{meter::JobMeterRequest, Response};
use std::{future::Future, pin::Pin, sync::Arc};
use tokio::{
    net::UnixStream,
    time::{timeout_at, Instant},
};

pub(crate) type Handler = Arc<
    dyn Fn(
            JobMeterRequest,
            u32,
            std::time::Instant,
        ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, &'static str>> + Send>>
        + Send
        + Sync,
>;

pub(crate) async fn response(
    handler: &Handler,
    stream: &UnixStream,
    id: &str,
    input: Result<JobMeterRequest, &'static str>,
    limits: &RuntimeLimits,
    metrics: &Arc<RuntimeMetrics>,
) -> Response {
    let input = match input {
        Ok(input) => input,
        Err(code) => return Response::error(id, code),
    };
    let peer = match crate::platform::peer_pid(stream) {
        Ok(peer) => peer,
        Err(_) => return Response::error(id, arc_ipc::ERROR_INVALID_REQUEST),
    };
    let max_ms = match &input {
        JobMeterRequest::Report(_) => arc_ipc::meter::METER_REPORT_MAX_BUDGET_MS,
        JobMeterRequest::Summary(_) => arc_ipc::meter::METER_SUMMARY_MAX_BUDGET_MS,
    };
    let deadline = std::time::Instant::now()
        + limits
            .db_response_timeout
            .min(std::time::Duration::from_millis(max_ms));
    match timeout_at(Instant::from_std(deadline), handler(input, peer, deadline)).await {
        Ok(Ok(value)) => Response::ok(id, value),
        Ok(Err(code)) => Response::error(id, code),
        Err(_) => {
            metrics.increment(Counter::DbTimeouts);
            Response::error(id, arc_ipc::ERROR_DB_BUSY)
        }
    }
}

//! Lane C dispatch of traffic.decision_history, read by `arcctl job diagnostics`.
//!
//! Called from `policy_dispatch::dispatch` on the authenticated connection;
//! the read runs on the single SQLite writer as `Input::DecisionHistory`
//! through `db_worker::policy`. Only a client that saw the daemon's standing
//! advertisement sends it; the reply is the only one naming the raw reason
//! `start_rate`, the start-rate settings and the per-class estimates.
use super::{write, Handler, Input, RuntimeLimits, Value};
use crate::runtime_metrics::RuntimeMetrics;
use arc_ipc::{traffic::decision_history as wire, Request, Response, PROTOCOL_VERSION};
use std::{
    sync::{Arc, OnceLock},
    time::Instant,
};
use tokio::net::UnixStream;

pub(crate) type Query = arc_ipc::traffic::decision_history::HistoryRequest;

/// The frozen Traffic configuration, installed by
/// `db_worker::traffic::configuration::current` the first time it resolves.
static POLICY: OnceLock<arc_traffic::Policy> = OnceLock::new();

pub(crate) fn install(policy: &arc_traffic::Policy) {
    let _ = POLICY.set(*policy);
}

pub(super) async fn dispatch(
    stream: &UnixStream,
    request: &Request,
    limits: &RuntimeLimits,
    handler: Option<&Handler>,
    metrics: &Arc<RuntimeMetrics>,
) -> bool {
    if request.protocol_version != PROTOCOL_VERSION
        || request.operation != wire::OPERATION_DECISION_HISTORY
    {
        return false;
    }
    let deadline = Instant::now() + limits.db_response_timeout;
    let error = |code| Response::error(&request.request_id, code);
    let response = match (handler, wire::decode_request(&request.payload)) {
        (None, _) => error(arc_ipc::ERROR_UNSUPPORTED_CAPABILITY),
        (_, Err(code)) => error(code),
        (Some(handler), Ok(query)) => match crate::platform::peer_pid(stream) {
            Err(_) => error(arc_ipc::ERROR_INVALID_REQUEST),
            Ok(peer) => {
                let work_end = deadline
                    - limits
                        .write_timeout
                        .min(deadline.saturating_duration_since(Instant::now()) / 2);
                let read = tokio::time::timeout_at(
                    tokio::time::Instant::from_std(work_end),
                    handler(Input::DecisionHistory(query), peer, work_end),
                )
                .await
                .unwrap_or(Err(arc_ipc::ERROR_DB_BUSY));
                match read {
                    Ok(Value::Read(value)) => Response::ok(&request.request_id, value),
                    Err(code) => error(code),
                    Ok(_) => error(arc_ipc::ERROR_DB_BUSY),
                }
            }
        },
    };
    write(stream, &response, deadline, metrics).await;
    true
}

/// The bounded read on the writer; the reply value is sent unchanged.
pub(crate) fn read(
    connection: &mut rusqlite::Connection,
    query: Query,
    snapshot: Option<&arc_meter::SnapshotReader>,
    now_ms: u64,
) -> Result<serde_json::Value, arc_storage::jobs::JobError> {
    let _ = snapshot;
    let policy = POLICY
        .get()
        .ok_or(arc_storage::jobs::JobError::StorageFailure)?;
    let history = arc_storage::jobs::traffic::decision_history::read(
        connection,
        &query.job_id,
        policy,
        now_ms,
    )?;
    let value =
        serde_json::to_value(&history).map_err(|_| arc_storage::jobs::JobError::StorageFailure)?;
    (value.to_string().len() <= wire::MAX_HISTORY_BYTES)
        .then_some(value)
        .ok_or(arc_storage::jobs::JobError::StorageFailure)
}

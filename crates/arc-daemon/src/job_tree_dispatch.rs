//! Lane A dispatch of meter.job_tree, read by `arcctl meter summary --job --json`.
//!
//! Called from `policy_dispatch::dispatch` on the authenticated connection;
//! the read runs on the single SQLite writer as `Input::JobTree`
//! through `db_worker::policy`. Only this operation names the tree report;
//! the v1 `meter.summary` reply is unchanged.
use super::{write, Handler, Input, RuntimeLimits, Value};
use crate::runtime_metrics::RuntimeMetrics;
use arc_ipc::{
    meter::job_tree::{self as wire, JobTreeReply, ProcessTreeReport},
    Request, Response, PROTOCOL_VERSION,
};
use rusqlite::OptionalExtension;
use std::{sync::Arc, time::Instant};
use tokio::net::UnixStream;

pub(crate) type Query = arc_ipc::meter::job_tree::JobTreeRequest;

pub(super) async fn dispatch(
    stream: &UnixStream,
    request: &Request,
    limits: &RuntimeLimits,
    handler: Option<&Handler>,
    metrics: &Arc<RuntimeMetrics>,
) -> bool {
    let quality = request.operation == arc_ipc::meter::job_tree_quality::OPERATION;
    if request.protocol_version != PROTOCOL_VERSION
        || !(request.operation == wire::OPERATION_JOB_TREE || quality)
    {
        return false;
    }
    let deadline = Instant::now() + limits.db_response_timeout;
    let error = |code| Response::error(&request.request_id, code);
    let Some(handler) = handler else {
        write(
            stream,
            &error(arc_ipc::ERROR_UNSUPPORTED_CAPABILITY),
            deadline,
            metrics,
        )
        .await;
        return true;
    };
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
            let job_id = query.job_id.clone();
            let read = tokio::time::timeout_at(
                tokio::time::Instant::from_std(work_end),
                handler(Input::JobTree(query), peer, work_end),
            )
            .await
            .unwrap_or(Err(arc_ipc::ERROR_DB_BUSY));
            match read {
                Ok(Value::Read(value)) if quality => {
                    match wire::decode_reply(&value, &job_id).ok().and_then(|reply| {
                        serde_json::to_value(arc_ipc::meter::job_tree_quality::Reply::from_report(
                            job_id,
                            reply.process_tree.as_ref(),
                        ))
                        .ok()
                    }) {
                        Some(value) => Response::ok(&request.request_id, value),
                        None => error(arc_ipc::ERROR_DB_BUSY),
                    }
                }
                Ok(Value::Read(value)) => Response::ok(&request.request_id, value),
                Err(code) => error(code),
                Ok(_) => error(arc_ipc::ERROR_DB_BUSY),
            }
        }
    };
    write(stream, &response, deadline, metrics).await;
    true
}

/// The bounded read on the writer; the reply value is sent unchanged. An
/// expired summary or a job never sampled has no tree report (`null`).
pub(crate) fn read(
    connection: &mut rusqlite::Connection,
    query: Query,
    snapshot: Option<&arc_meter::SnapshotReader>,
    now_ms: u64,
) -> Result<serde_json::Value, arc_storage::jobs::JobError> {
    use arc_storage::jobs::JobError;
    let _ = (snapshot, now_ms);
    let stored: Option<String> = connection
        .query_row(
            "SELECT process_tree_json FROM job_meter_summaries
             WHERE job_id=?1 AND expired_at_ms IS NULL",
            [query.job_id.as_str()],
            |row| row.get(0),
        )
        .optional()?
        .flatten();
    let process_tree = match stored {
        Some(json) => Some(
            serde_json::from_str::<ProcessTreeReport>(&json)
                .ok()
                .filter(|report| report.job_id == query.job_id.as_str())
                .ok_or(JobError::StorageFailure)?,
        ),
        None => None,
    };
    serde_json::to_value(JobTreeReply {
        schema_version: wire::SCHEMA_VERSION,
        job_id: query.job_id,
        process_tree,
    })
    .map_err(|_| JobError::StorageFailure)
}

#[cfg(test)]
#[path = "job_tree_dispatch/tests.rs"]
mod tests;

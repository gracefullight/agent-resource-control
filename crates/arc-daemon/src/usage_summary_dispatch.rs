//! Lane B dispatch of `meter.usage_summary` and of `meter.snapshot` with
//! `include_traffic` (MTR-004, MTR-008, MTR-018, MET-013, MET-015, MET-017).
//!
//! Called from `policy_dispatch::dispatch` on the authenticated connection;
//! the read runs on the single SQLite writer as `Input::UsageSummary`
//! through `db_worker::policy`. A `meter.snapshot` request that does not ask
//! for Traffic (`null`, protocol 1, or `include_traffic:false`) is left to
//! the v1 Meter handler, so its reply bytes are unchanged. Only a request
//! with `include_traffic:true`, sent by a client that saw the daemon's
//! advertisement, gets the v1 snapshot fields plus `traffic`.
use super::{write, Handler, Input, RuntimeLimits, Value};
use crate::runtime_metrics::RuntimeMetrics;
use arc_ipc::{
    meter::{self as wire, usage},
    Request, Response, PROTOCOL_VERSION,
};
use std::{sync::Arc, time::Instant};
use tokio::net::UnixStream;

/// One lane B read on the writer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Query {
    Summary(usage::UsageQuery),
    /// The cached Meter snapshot with `traffic.queue.depth` from committed state.
    SnapshotTraffic,
}

/// Which lane B read `request` names, if any; `Err` carries its error code.
fn classify(request: &Request) -> Option<Result<Query, &'static str>> {
    if request.protocol_version != PROTOCOL_VERSION {
        return None;
    }
    match request.operation.as_str() {
        usage::OPERATION_USAGE_SUMMARY => {
            Some(usage::decode_query(&request.payload).map(Query::Summary))
        }
        wire::OPERATION_METER_SNAPSHOT => match usage::decode_snapshot_request(&request.payload) {
            Ok(usage::SnapshotRequest {
                include_traffic: true,
            }) => Some(Ok(Query::SnapshotTraffic)),
            // Protocol-1 and invalid payloads keep the v1 handler's reply.
            _ => None,
        },
        _ => None,
    }
}

pub(super) async fn dispatch(
    stream: &UnixStream,
    request: &Request,
    limits: &RuntimeLimits,
    handler: Option<&Handler>,
    metrics: &Arc<RuntimeMetrics>,
) -> bool {
    let Some(query) = classify(request) else {
        return false;
    };
    let snapshot = matches!(query, Ok(Query::SnapshotTraffic));
    let Some(handler) = handler else {
        if snapshot {
            // Without the writer no queue depth exists; the v1 reply stands.
            return false;
        }
        let response = Response::error(&request.request_id, arc_ipc::ERROR_UNSUPPORTED_CAPABILITY);
        let deadline = Instant::now() + limits.db_response_timeout;
        write(stream, &response, deadline, metrics).await;
        return true;
    };
    let deadline = Instant::now() + limits.db_response_timeout;
    let work_end = deadline
        - limits
            .write_timeout
            .min(deadline.saturating_duration_since(Instant::now()) / 2);
    let error = |code| Response::error(&request.request_id, code);
    let response = match (query, crate::platform::peer_pid(stream)) {
        (Err(code), _) => error(code),
        (_, Err(_)) => error(arc_ipc::ERROR_INVALID_REQUEST),
        (Ok(query), Ok(peer)) => {
            let read = tokio::time::timeout_at(
                tokio::time::Instant::from_std(work_end),
                handler(Input::UsageSummary(query), peer, work_end),
            )
            .await
            .unwrap_or(Err(arc_ipc::ERROR_DB_BUSY));
            match read {
                // `read` answers Null when no Meter snapshot is cached.
                Ok(Value::Read(serde_json::Value::Null)) if snapshot => {
                    error(wire::ERROR_METER_UNAVAILABLE)
                }
                Ok(Value::Read(value)) => Response::ok(&request.request_id, value),
                Err(_) if snapshot => error(wire::ERROR_METER_UNAVAILABLE),
                Err(code) => error(code),
                Ok(_) => error(arc_ipc::ERROR_DB_BUSY),
            }
        }
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
    use arc_storage::{jobs::JobError, usage_summary};
    match query {
        Query::Summary(query) => {
            usage_summary::summarize(connection, &query, now_ms).and_then(|summary| {
                serde_json::to_value(summary).map_err(|_| JobError::StorageFailure)
            })
        }
        Query::SnapshotTraffic => {
            let Some(meter) = snapshot.and_then(arc_meter::SnapshotReader::snapshot) else {
                return Ok(serde_json::Value::Null);
            };
            let traffic = usage_summary::queue_depth(connection, now_ms)?;
            let mut value = serde_json::to_value(&meter).map_err(|_| JobError::StorageFailure)?;
            if wire::decode_snapshot(&value).is_err() {
                return Ok(serde_json::Value::Null);
            }
            let object = value.as_object_mut().ok_or(JobError::StorageFailure)?;
            object.insert(
                usage::SNAPSHOT_TRAFFIC_KEY.to_owned(),
                serde_json::to_value(traffic).map_err(|_| JobError::StorageFailure)?,
            );
            Ok(value)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request(operation: &str, payload: serde_json::Value) -> Request {
        let mut request = Request::new("usage", operation);
        request.payload = payload;
        request
    }

    #[test]
    fn only_a_traffic_snapshot_or_a_usage_summary_leaves_the_v1_handlers() {
        let snapshot = wire::OPERATION_METER_SNAPSHOT;
        for payload in [
            serde_json::Value::Null,
            json!({"include_traffic": false}),
            json!({}),
            json!({"include_traffic": true, "pid": 1}),
        ] {
            assert_eq!(
                classify(&request(snapshot, payload.clone())),
                None,
                "{payload}"
            );
        }
        assert_eq!(
            classify(&request(snapshot, json!({"include_traffic": true}))),
            Some(Ok(Query::SnapshotTraffic))
        );
        let mut old = request(snapshot, json!({"include_traffic": true}));
        old.protocol_version += 1;
        assert_eq!(classify(&old), None);
        let summary = usage::OPERATION_USAGE_SUMMARY;
        let valid = json!({"by":"vendor","from_ms":1,"to_ms":2,"bucket":"hour"});
        assert!(matches!(
            classify(&request(summary, valid)),
            Some(Ok(Query::Summary(_)))
        ));
        for invalid in [
            serde_json::Value::Null,
            json!({"by":"vendor","from_ms":2,"to_ms":2,"bucket":"hour"}),
            json!({"by":"session","from_ms":1,"to_ms":2,"bucket":"hour"}),
        ] {
            assert_eq!(
                classify(&request(summary, invalid)),
                Some(Err(arc_ipc::ERROR_INVALID_REQUEST))
            );
        }
        assert_eq!(classify(&request("meter.summary", json!({}))), None);
    }
}

//! Additive, read-only protocol-1 Meter contract over the existing authenticated
//! transport. These schema helpers do not install a daemon handler.
use crate::{Request, Response, ERROR_INVALID_REQUEST, PROTOCOL_VERSION, STATUS_ERROR, STATUS_OK};
pub use arc_core::meter::{MeterSnapshot, MAX_SNAPSHOT_BYTES};
use serde::Deserialize;
use serde_json::Value;

pub const OPERATION_METER_SNAPSHOT: &str = "meter.snapshot";
pub const ERROR_METER_UNAVAILABLE: &str = "METER_UNAVAILABLE";

pub fn snapshot_request(request_id: impl Into<String>) -> Request {
    Request::new(request_id, OPERATION_METER_SNAPSHOT)
}

/// No path, PID, rate, history or attribution query is accepted from a client.
pub fn decode_request(payload: &Value) -> Result<(), &'static str> {
    payload.is_null().then_some(()).ok_or(ERROR_INVALID_REQUEST)
}

fn object_tree(value: &Value) -> bool {
    match value {
        Value::Array(_) => false,
        Value::Object(map) => map.values().all(object_tree),
        _ => true,
    }
}

/// A bounded object tree, not serde's positional struct arrays. Diagnostics from
/// decoding untrusted peer content are never returned to a product surface.
pub fn decode_snapshot(value: &Value) -> Result<MeterSnapshot, &'static str> {
    if !value.is_object()
        || !object_tree(value)
        || serde_json::to_vec(value).map_or(true, |bytes| bytes.len() > MAX_SNAPSHOT_BYTES)
    {
        return Err("invalid meter snapshot");
    }
    let snapshot = MeterSnapshot::deserialize(value).map_err(|_| "invalid meter snapshot")?;
    snapshot
        .is_valid()
        .then_some(snapshot)
        .ok_or("invalid meter snapshot")
}

/// The daemon uses this only after reading its cached latest snapshot. It never
/// performs native measurement on a requesting connection's execution path.
pub fn snapshot_response(request: &Request, snapshot: &MeterSnapshot) -> Response {
    if request.protocol_version != PROTOCOL_VERSION {
        return Response::error(&request.request_id, crate::ERROR_PROTOCOL_INCOMPATIBLE);
    }
    if request.operation != OPERATION_METER_SNAPSHOT || decode_request(&request.payload).is_err() {
        return Response::error(&request.request_id, ERROR_INVALID_REQUEST);
    }
    if !snapshot.is_valid() {
        return Response::error(&request.request_id, ERROR_METER_UNAVAILABLE);
    }
    let result = serde_json::to_value(snapshot);
    match result {
        Ok(value) if decode_snapshot(&value).is_ok() => Response::ok(&request.request_id, value),
        _ => Response::error(&request.request_id, ERROR_METER_UNAVAILABLE),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotError {
    Unsupported,
    Unavailable,
    Protocol,
    InvalidReply,
}

/// Matching responses only. An old daemon's explicit unsupported reply remains
/// distinct from connection loss, malformed readings and a genuine numeric zero.
pub fn interpret_snapshot(
    request: &Request,
    response: &Response,
) -> Result<MeterSnapshot, SnapshotError> {
    if request.protocol_version != PROTOCOL_VERSION || response.protocol_version != PROTOCOL_VERSION
    {
        return Err(SnapshotError::Protocol);
    }
    if request.operation != OPERATION_METER_SNAPSHOT
        || decode_request(&request.payload).is_err()
        || request.request_id != response.request_id
    {
        return Err(SnapshotError::InvalidReply);
    }
    match (response.status.as_str(), &response.result, &response.error) {
        (STATUS_OK, Some(value), None) => {
            decode_snapshot(value).map_err(|_| SnapshotError::InvalidReply)
        }
        (STATUS_ERROR, None, Some(error)) if error.code == crate::ERROR_UNSUPPORTED_CAPABILITY => {
            Err(SnapshotError::Unsupported)
        }
        (STATUS_ERROR, None, Some(error)) if error.code == ERROR_METER_UNAVAILABLE => {
            Err(SnapshotError::Unavailable)
        }
        _ => Err(SnapshotError::InvalidReply),
    }
}

/// Typed read with the caller's existing absolute deadline and cancellation.
/// No new retry policy, thread, authentication path or daemon launch is added.
#[cfg(unix)]
pub fn snapshot_until(
    socket: &std::path::Path,
    request_id: impl Into<String>,
    deadline: std::time::Instant,
    cancelled: &dyn Fn() -> bool,
) -> Result<MeterSnapshot, crate::client::Failure> {
    let request = snapshot_request(request_id);
    let value = crate::client::exchange_until(
        socket,
        &request,
        deadline,
        cancelled,
        &mut false,
        crate::client::ExchangeOptions {
            continuity: None,
            retry_busy: false,
        },
    )?;
    decode_snapshot(&value).map_err(|_| crate::client::Failure::InvalidReply)
}

#[cfg(test)]
mod tests;

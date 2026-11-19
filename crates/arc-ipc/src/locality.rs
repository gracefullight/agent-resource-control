//! Additive endpoint-aware classification; raw inputs are never persisted.
use crate::{Request, Response, ERROR_INVALID_REQUEST};
use arc_detector::{ClassifyRequest, EndpointContext};
use serde::{Deserialize, Serialize};
use std::ffi::OsString;

/// The operation itself is the capability: old peers return UNSUPPORTED.
pub const OPERATION_CLASSIFY: &str = "job.classify.locality.v1";
const MAX_PAYLOAD_BYTES: usize = 196_608;

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum Command {
    Shell { command: String, shell_kind: String },
    Argv { argv: Vec<String> },
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClassifyLocalityRequest {
    pub command: Command,
    pub context: EndpointContext,
}

pub fn now_ms() -> Option<u64> {
    let value = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_millis();
    u64::try_from(value).ok()
}

pub(crate) fn classify_response(request: &Request) -> Response {
    classify_response_at(request, now_ms().unwrap_or(0))
}

/// A supplied clock makes age-boundary and malformed-request checks deterministic.
pub fn classify_response_at(request: &Request, now_ms: u64) -> Response {
    if request.protocol_version != crate::PROTOCOL_VERSION {
        return Response::error(&request.request_id, crate::ERROR_PROTOCOL_INCOMPATIBLE);
    }
    if request.operation != OPERATION_CLASSIFY {
        return Response::error(&request.request_id, crate::ERROR_UNSUPPORTED_CAPABILITY);
    }
    if !request.payload.is_object()
        || serde_json::to_vec(&request.payload).map_or(true, |v| v.len() > MAX_PAYLOAD_BYTES)
    {
        return Response::error(&request.request_id, ERROR_INVALID_REQUEST);
    }
    let Ok(input) = ClassifyLocalityRequest::deserialize(&request.payload) else {
        return Response::error(&request.request_id, ERROR_INVALID_REQUEST);
    };
    let result = match input.command {
        Command::Shell {
            command,
            shell_kind,
        } => arc_detector::classify_with_context(
            &ClassifyRequest {
                command,
                shell_kind,
            },
            &input.context,
            now_ms,
        ),
        Command::Argv { argv } => arc_detector::classify_argv_with_context(
            &argv.into_iter().map(OsString::from).collect::<Vec<_>>(),
            &input.context,
            now_ms,
        ),
    };
    Response::ok(&request.request_id, serde_json::json!(result))
}

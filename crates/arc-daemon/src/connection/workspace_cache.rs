//! Per-daemon bounded cache. Clients revalidate identities and membership.
use arc_ipc::{
    workspace_cache::{
        cache_types::{MAX_ENTRIES, MAX_ENTRY_BYTES},
        CacheRequest,
    },
    Request, Response,
};
use std::{
    collections::VecDeque,
    sync::{Mutex, OnceLock},
};

static ENTRIES: OnceLock<Mutex<VecDeque<(String, String)>>> = OnceLock::new();

pub(super) fn handle(request: &Request) -> Option<Response> {
    if request.operation != arc_ipc::workspace_cache::OPERATION {
        return None;
    }
    if request.protocol_version != arc_ipc::PROTOCOL_VERSION {
        return Some(Response::error(
            &request.request_id,
            arc_ipc::ERROR_PROTOCOL_INCOMPATIBLE,
        ));
    }
    if serde_json::to_vec(&request.payload).map_or(true, |v| v.len() > 2 * MAX_ENTRY_BYTES + 1024) {
        return Some(Response::error(
            &request.request_id,
            arc_ipc::ERROR_INVALID_REQUEST,
        ));
    }
    let Ok(input) = serde_json::from_value::<CacheRequest>(request.payload.clone()) else {
        return Some(Response::error(
            &request.request_id,
            arc_ipc::ERROR_INVALID_REQUEST,
        ));
    };
    if !input.valid() {
        return Some(Response::error(
            &request.request_id,
            arc_ipc::ERROR_INVALID_REQUEST,
        ));
    }
    let Ok(mut entries) = ENTRIES.get_or_init(|| Mutex::new(VecDeque::new())).lock() else {
        return Some(Response::error(&request.request_id, arc_ipc::ERROR_DB_BUSY));
    };
    let value = match input {
        CacheRequest::Probe => {
            serde_json::json!({"capability": arc_ipc::workspace_cache::OPERATION})
        }
        CacheRequest::Read { key } => {
            let entry = entries
                .iter()
                .position(|(name, _)| name == &key)
                .and_then(|index| entries.remove(index));
            let value = entry.as_ref().map(|(_, entry)| entry.clone());
            if let Some(entry) = entry {
                entries.push_back(entry);
            }
            serde_json::json!({"entry": value})
        }
        CacheRequest::Write { key, entry } => {
            if let Some(index) = entries.iter().position(|(name, _)| name == &key) {
                entries.remove(index);
            }
            while entries.len() >= MAX_ENTRIES {
                entries.pop_front();
            }
            entries.push_back((key, entry));
            serde_json::json!({"stored": true})
        }
    };
    Some(Response::ok(&request.request_id, value))
}

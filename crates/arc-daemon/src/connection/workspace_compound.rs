//! Transient negotiated compounds keyed by their execution job. The bounded
//! map is diagnostic metadata; protocol-1 lifecycle and job bytes are intact.
use arc_ipc::{
    workspace_compound::{Compound, CompoundRequest, OPERATION},
    Request, Response,
};
use std::{
    collections::VecDeque,
    sync::{Mutex, OnceLock},
};

static JOBS: OnceLock<Mutex<VecDeque<(String, Compound)>>> = OnceLock::new();
const MAX_JOBS: usize = 64;
const MAX_PAYLOAD: usize = 196_608;

pub(super) fn handle(request: &Request) -> Option<Response> {
    if request.operation != OPERATION {
        return None;
    }
    if request.protocol_version != arc_ipc::PROTOCOL_VERSION {
        return Some(Response::error(
            &request.request_id,
            arc_ipc::ERROR_PROTOCOL_INCOMPATIBLE,
        ));
    }
    if serde_json::to_vec(&request.payload).map_or(true, |bytes| bytes.len() > MAX_PAYLOAD) {
        return Some(Response::error(
            &request.request_id,
            arc_ipc::ERROR_INVALID_REQUEST,
        ));
    }
    let Ok(input) = serde_json::from_value::<CompoundRequest>(request.payload.clone()) else {
        return Some(Response::error(
            &request.request_id,
            arc_ipc::ERROR_INVALID_REQUEST,
        ));
    };
    let Ok(mut jobs) = JOBS.get_or_init(|| Mutex::new(VecDeque::new())).lock() else {
        return Some(Response::error(&request.request_id, arc_ipc::ERROR_DB_BUSY));
    };
    let value = match input {
        CompoundRequest::Probe => serde_json::json!({"capability": OPERATION}),
        CompoundRequest::Analyze { members, job_id } => {
            let Some(compound) = Compound::local(members) else {
                return Some(Response::error(
                    &request.request_id,
                    arc_ipc::ERROR_INVALID_REQUEST,
                ));
            };
            if let Some(id) = job_id {
                let id = id.as_str().to_owned();
                if let Some(index) = jobs.iter().position(|(key, _)| key == &id) {
                    jobs.remove(index);
                }
                while jobs.len() >= MAX_JOBS {
                    jobs.pop_front();
                }
                jobs.push_back((id, compound.clone()));
            }
            serde_json::json!({"compound": compound})
        }
        CompoundRequest::Read { job_id } => {
            serde_json::json!({"compound": jobs.iter().find(|(id, _)| id == job_id.as_str()).map(|(_, value)| value)})
        }
    };
    Some(Response::ok(&request.request_id, value))
}

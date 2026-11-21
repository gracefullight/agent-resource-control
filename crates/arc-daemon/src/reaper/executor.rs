//! ReaperExecutor: ask the supervising wrapper to signal its own child.
//!
//! This daemon never calls kill on a job child. A lost endpoint, a dead or
//! unauthenticated wrapper, or any unreadable reply results in no signal
//! request being counted as delivered.
use super::registry::{endpoint_peer, Capability};
use arc_ipc::{
    jobs::JobId,
    reaper::{ActionOutcome, ActionRequest, ActionResult, ChildEvidence, Refusal, Signal},
};
use std::time::Instant;

pub(super) enum Reply {
    Signaled,
    Refused(Refusal),
    /// Nothing reached the wrapper.
    Unreachable(&'static str),
    /// The request may have been delivered; no reply confirmed its outcome.
    Unconfirmed,
}

pub(super) async fn request(
    job_id: &JobId,
    capability: &Capability,
    child: ChildEvidence,
    signal: Signal,
    deadline: Instant,
) -> Reply {
    let stream = match endpoint_peer(&capability.endpoint, &capability.observer, deadline).await {
        Ok(stream) => stream,
        Err(reason) => return Reply::Unreachable(reason),
    };
    let Ok(nonce) = arc_core::process_identity::random_job_id() else {
        return Reply::Unreachable("ENDPOINT_UNAVAILABLE");
    };
    let action = ActionRequest {
        kind: arc_ipc::reaper::ACTION_KIND.into(),
        nonce: nonce.clone(),
        token: capability.token.clone(),
        job_id: job_id.clone(),
        child,
        signal,
    };
    let Ok(message) = serde_json::to_value(action) else {
        return Reply::Unreachable("ENDPOINT_UNAVAILABLE");
    };
    let end = tokio::time::Instant::from_std(deadline);
    let reply = match crate::transport::exchange_value(&stream, &message, end).await {
        Ok(reply) => reply,
        Err(false) => return Reply::Unreachable("ENDPOINT_UNAVAILABLE"),
        Err(true) => return Reply::Unconfirmed,
    };
    // Injected fault `action-reply`: the request was delivered and answered,
    // and this daemon behaves as if it never read the answer.
    if super::fixture::fault("action-reply").is_some() {
        return Reply::Unconfirmed;
    }
    match serde_json::from_value::<ActionResult>(reply) {
        Ok(result) if result.kind == arc_ipc::reaper::RESULT_KIND && result.nonce == nonce => {
            match result.outcome {
                ActionOutcome::Signaled {} => Reply::Signaled,
                ActionOutcome::Refused { reason } => Reply::Refused(reason),
            }
        }
        _ => Reply::Unconfirmed,
    }
}

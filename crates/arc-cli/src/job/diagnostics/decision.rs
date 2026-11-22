//! The last recorded decision of one job, including a negotiated fairness
//! reason (AT-TRF-08).
//!
//! The v1 diagnostics reply carries a fairness reason as its projection, the
//! existing pair `admit`/`admitted`. Only when that projected decision could
//! hide one does this module ask for the raw reason, as a second request on
//! the same authenticated connection. An older daemon, or any failure of that
//! optional read, leaves the projection: a valid recorded fact, never an
//! invented absence. Output of every other job is unchanged except for the
//! one rendered line.
use arc_ipc::{
    client::Session,
    jobs::JobId,
    traffic::{
        diagnostics::DiagnosticResult,
        fairness::{
            decode_result, DecisionRequest, FairnessReason, NegotiatedReason, OPERATION_DECISION,
        },
    },
    Request,
};
use serde_json::{json, Value};

/// The fairness reason recorded for this job's last decision, if any.
pub(super) fn fair_turn(
    session: &mut Session,
    job: &JobId,
    result: &DiagnosticResult,
) -> Option<FairnessReason> {
    let decision = result.admission.as_ref()?.last_decision.as_ref()?;
    let projected = (decision.kind, decision.reason);
    if !FairnessReason::ALL
        .iter()
        .any(|reason| reason.projection() == projected)
    {
        return None;
    }
    let mut request = Request::new(crate::request_id(), OPERATION_DECISION);
    request.payload = json!(DecisionRequest {
        job_id: job.clone()
    });
    let value = session.exchange(&request, &|| false).ok()?;
    match decode_result(&value, job).ok()?.reason? {
        // A reason that does not project to the served decision is not its
        // explanation; keep the decision the diagnostics reply carried.
        NegotiatedReason::Fairness(reason) if reason.projection() == projected => Some(reason),
        _ => None,
    }
}

/// `Last recorded decision: <reason>`, or nothing when none is recorded.
pub(super) fn line(result: &DiagnosticResult, fair: Option<FairnessReason>) -> String {
    let recorded = result
        .admission
        .as_ref()
        .and_then(|admission| admission.last_decision.as_ref());
    match (fair, recorded) {
        (Some(reason), _) => format!("Last recorded decision: {}\n", tag(&reason)),
        (None, Some(decision)) => format!("Last recorded decision: {}\n", tag(&decision.reason)),
        (None, None) => String::new(),
    }
}

/// The diagnostics object; a fair turn adds `last_recorded_reason` beside it.
pub(super) fn document(result: &DiagnosticResult, fair: Option<FairnessReason>) -> Value {
    let mut document = json!(result);
    if let Some(reason) = fair {
        document["last_recorded_reason"] = json!(reason);
    }
    document
}

fn tag<T: serde::Serialize>(value: &T) -> String {
    json!(value).as_str().unwrap_or_default().to_owned()
}

#[cfg(test)]
#[path = "decision_tests.rs"]
mod tests;

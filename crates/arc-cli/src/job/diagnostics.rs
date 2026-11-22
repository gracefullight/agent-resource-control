//! Optional read-only detail. Never turns a failed read into absence/liveness.
use crate::exec::transport::Failure;
use arc_ipc::{
    client::Session,
    jobs::JobId,
    traffic::{diagnostics::*, fairness::FairnessReason, Capabilities},
    Request,
};
use serde_json::json;
use std::{
    path::Path,
    process::ExitCode,
    time::{Duration, Instant},
};

pub(crate) fn run(id: String, json_output: bool) -> ExitCode {
    let Ok(job) = JobId::try_from(id) else {
        return super::invalid();
    };
    let result = arc_core::socket_path()
        .map_err(|_| Failure::Unavailable)
        .and_then(|socket| inspect(&socket, &job));
    match result {
        Ok((result, fair, history)) => {
            let history = history.as_ref();
            if json_output {
                let document = decision::document(&result, fair);
                println!("{}", history::document(document, history));
            } else {
                print!(
                    "{}{}{}",
                    render(&result),
                    decision::line(&result, fair),
                    history::lines(history)
                );
            }
            ExitCode::SUCCESS
        }
        Err(Failure::Unsupported) => super::error(
            "DIAGNOSTICS_UNSUPPORTED",
            "Job diagnostics are unsupported by this daemon.",
        ),
        Err(_) => super::error(
            "DIAGNOSTICS_UNAVAILABLE",
            "Classification and admission facts are unavailable.",
        ),
    }
}
#[cfg(test)]
fn read(socket: &Path, job: &JobId) -> Result<DiagnosticResult, Failure> {
    inspect(socket, job).map(|(result, _, _)| result)
}
type Inspected = (
    DiagnosticResult,
    Option<FairnessReason>,
    Option<arc_ipc::traffic::decision_history::DecisionHistory>,
);
fn inspect(socket: &Path, job: &JobId) -> Result<Inspected, Failure> {
    let deadline = Instant::now() + Duration::from_secs(2);
    let probe = |operation| {
        Session::connect_until(socket, deadline, &|| false, None)?
            .exchange(&Request::new(crate::request_id(), operation), &|| false)
    };
    use arc_ipc::traffic::native;
    let mut selected = None;
    for (cap_operation, operation, capability) in [
        (
            native::OPERATION_DIAGNOSTICS_CAPABILITIES,
            native::OPERATION_DIAGNOSTICS_JOB,
            native::DIAGNOSTICS_CAPABILITY,
        ),
        (
            OPERATION_PYTHON_CAPABILITIES,
            OPERATION_PYTHON_JOB,
            PYTHON_CAPABILITY,
        ),
        (OPERATION_CAPABILITIES, OPERATION_JOB, CAPABILITY),
    ] {
        let value = match probe(cap_operation) {
            Ok(value) => value,
            Err(Failure::Unsupported) => continue,
            Err(error) => return Err(error),
        };
        if !value.is_object() {
            return Err(Failure::InvalidReply);
        }
        let caps: Capabilities =
            serde_json::from_value(value).map_err(|_| Failure::InvalidReply)?;
        if caps.capability != capability {
            return Err(Failure::InvalidReply);
        }
        selected = Some(operation);
        break;
    }
    let operation = selected.ok_or(Failure::Unsupported)?;
    let mut query = Request::new(crate::request_id(), operation);
    query.payload = json!(DiagnosticRequest {
        job_id: job.clone()
    });
    let continuity = arc_ipc::client::Continuity::default();
    let mut session = Session::connect_until(socket, deadline, &|| false, Some(&continuity))?;
    let value = session.exchange(&query, &|| false)?;
    let result = decode_result(&value, job).map_err(|_| Failure::InvalidReply)?;
    if result.classification.is_some_and(|c| {
        (operation != native::OPERATION_DIAGNOSTICS_JOB && c.is_native_tool())
            || (operation == OPERATION_JOB && c.is_python_test())
    }) {
        return Err(Failure::InvalidReply);
    }
    let fair = decision::fair_turn(&mut session, job, &result);
    let daemon = continuity.identity().map(|identity| identity.pid);
    let history = history::read(&mut session, socket, daemon, job);
    Ok((result, fair, history))
}

#[path = "diagnostics/decision.rs"]
mod decision;
#[path = "diagnostics/history.rs"]
mod history;

#[cfg(test)]
#[path = "diagnostics/tests.rs"]
mod tests;
fn render(result: &DiagnosticResult) -> String {
    let mut output = format!(
        "Job: {}\nLast known: true\nRecorded (Unix ms): {}\nLiveness refreshed: false\n",
        result.job_id.as_str(),
        result.recorded_at_ms
    );
    if let Some(class) = result.classification {
        output.push_str(&format!(
            "Classification: {}\nLocality: {}\nClassification reason: {}\n",
            json!(class.classification).as_str().unwrap(),
            json!(class.locality).as_str().unwrap(),
            json!(class.reason).as_str().unwrap()
        ));
    } else {
        output.push_str("Classification: not recorded\n");
    }
    if let Some(admission) = &result.admission {
        output.push_str(&format!(
            "Admission: {}\nPermit active (last recorded): {}\n",
            json!(admission.state).as_str().unwrap(),
            admission.permit_active
        ));
        if let Some(reason) = admission.wait_reason {
            output.push_str(&format!(
                "Last recorded queue cause: {}\n",
                json!(reason).as_str().unwrap()
            ));
        }
        if let Some(resources) = admission.reservation {
            output.push_str(&format!(
                "Reserved CPU (millis): {}\nReserved memory (bytes): {}\n",
                resources.cpu_millis, resources.memory_bytes
            ));
        }
        if let Some(outcome) = &admission.outcome {
            output.push_str(&format!("Recorded outcome: {}\n", json!(outcome)));
        }
    } else {
        output.push_str("Admission: not recorded\n");
    }
    output
}

//! Optional read-only detail. Never turns a failed read into absence/liveness.
use crate::exec::transport::Failure;
use arc_ipc::{
    client::Session,
    jobs::JobId,
    traffic::{diagnostics::*, Capabilities},
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
        .and_then(|socket| read(&socket, &job));
    match result {
        Ok(result) => {
            if json_output {
                println!("{}", json!(result));
            } else {
                print!("{}", render(&result));
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
fn read(socket: &Path, job: &JobId) -> Result<DiagnosticResult, Failure> {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut probe = Session::connect_until(socket, deadline, &|| false, None)?;
    let value = probe.exchange(
        &Request::new(crate::request_id(), OPERATION_CAPABILITIES),
        &|| false,
    )?;
    let caps: Capabilities = serde_json::from_value(value).map_err(|_| Failure::InvalidReply)?;
    if caps.capability != CAPABILITY {
        return Err(Failure::InvalidReply);
    }
    drop(probe);
    let mut query = Request::new(crate::request_id(), OPERATION_JOB);
    query.payload = json!(DiagnosticRequest {
        job_id: job.clone()
    });
    let mut session = Session::connect_until(socket, deadline, &|| false, None)?;
    let value = session.exchange(&query, &|| false)?;
    decode_result(&value, job).map_err(|_| Failure::InvalidReply)
}
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

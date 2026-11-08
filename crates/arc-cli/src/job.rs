//! Read-only lifecycle discovery; timestamps are Unix milliseconds.
use arc_ipc::jobs::{
    JobId, ObservationStatus, ObservationTarget, Outcome, Phase, StatusRequest, StatusResult,
    OPERATION_JOB_STATUS,
};
use serde_json::json;
use std::process::ExitCode;
pub(crate) fn invalid() -> ExitCode {
    eprintln!("INVALID_REQUEST\nUse arcctl job status [JOB_ID]. No work was launched.");
    ExitCode::from(2)
}
pub(crate) fn run(id: Option<String>) -> ExitCode {
    let query = match id {
        Some(id) => match JobId::try_from(id) {
            Ok(job_id) => StatusRequest::ById { job_id },
            Err(_) => {
                return error(
                    "JOB_NOT_FOUND",
                    "No lifecycle record was found for this ID.",
                )
            }
        },
        None => StatusRequest::Recent {},
    };
    let expected_id = match &query {
        StatusRequest::ById { job_id } => Some(job_id.clone()),
        _ => None,
    };
    let result = arc_core::socket_path()
        .map_err(|_| ())
        .and_then(|socket| {
            super::exec::transport::call(&socket, OPERATION_JOB_STATUS, json!(query))
        })
        .and_then(|value| serde_json::from_value::<StatusResult>(value).map_err(|_| ()));
    match result {
        Ok(result)
            if result.is_valid()
                && match (&query, &result) {
                    (StatusRequest::ById { .. }, StatusResult::Found { job }) => {
                        expected_id.as_ref() == Some(&job.job_id)
                    }
                    (StatusRequest::ById { .. }, StatusResult::NotFound {})
                    | (StatusRequest::Recent {}, StatusResult::Recent { .. }) => true,
                    _ => false,
                } =>
        {
            match result {
                StatusResult::NotFound {} => error(
                    "JOB_NOT_FOUND",
                    "No lifecycle record was found for this ID.",
                ),
                StatusResult::Recent {
                    jobs, truncated, ..
                } => {
                    if jobs.is_empty() {
                        println!("No lifecycle records.");
                    }
                    for job in jobs {
                        println!(
                            "Job: {}\nState: {}\nRegistered (Unix ms): {}\nUpdated (Unix ms): {}\nReason: {}\nLast known: true",
                            job.job_id.as_str(),
                            label(job.phase),
                            job.registered_at_ms,
                            job.updated_at_ms,
                            json!(job.reason).as_str().unwrap_or("IDENTITY_UNAVAILABLE")
                        );
                    }
                    if truncated {
                        println!("Truncated: showing the 50 most recent records.");
                    }
                    println!("Scheduling: not implemented");
                    ExitCode::SUCCESS
                }
                StatusResult::Found { job } => {
                    println!("Job: {}\nState: {}\nIdentity: {}\nRegistered (Unix ms): {}\nObserved (Unix ms): {}",
                    job.job_id.as_str(), label(job.phase),
                    if job.observation.status == ObservationStatus::VerifiedMatch && job.observation.target==ObservationTarget::Child { "verified at observation" } else { "unavailable" },
                    job.registered_at_ms, job.observation.observed_at_ms);
                    println!(
                        "Reason: {}",
                        json!(job.reason).as_str().unwrap_or("IDENTITY_UNAVAILABLE")
                    );
                    match job.outcome {
                        Some(Outcome::Exited { code }) => println!("Exit code: {code}"),
                        Some(Outcome::Signaled {
                            signal,
                            core_dumped,
                        }) => println!("Signal: {signal}\nCore dumped: {core_dumped}"),
                        Some(Outcome::SpawnFailed { .. }) => println!("Reason: SPAWN_FAILED"),
                        None => (),
                    }
                    if let Some(time) = job.completed_at_ms {
                        println!("Completed (Unix ms): {time}");
                    }
                    println!("Scheduling: not implemented");
                    ExitCode::SUCCESS
                }
            }
        }
        _ => error(
            "STATUS_UNAVAILABLE",
            "Job status is unavailable. No work was launched by this status request.",
        ),
    }
}
fn error(code: &str, text: &str) -> ExitCode {
    eprintln!("{code}\n{text}");
    ExitCode::FAILURE
}
fn label(phase: Phase) -> &'static str {
    match phase {
        Phase::Registered | Phase::Launching => "starting",
        Phase::Running => "running",
        Phase::Completed => "completed",
        Phase::Signaled => "terminated by signal",
        Phase::SpawnFailed => "spawn failed",
        Phase::OutcomeUnknown => "outcome unknown",
    }
}

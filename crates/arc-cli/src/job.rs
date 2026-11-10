//! Read-only lifecycle discovery; timestamps are Unix milliseconds.
use arc_ipc::jobs::{
    JobId, ObservationStatus, ObservationTarget, Outcome, Phase, StatusRequest, StatusResult,
    OPERATION_JOB_STATUS,
};
use serde_json::json;
use std::{
    process::ExitCode,
    time::{Duration, Instant},
};
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
    let selected_traffic = expected_id.as_ref().map(traffic_status);
    if let (Some(id), Some(TrafficQuery::Found(result))) =
        (expected_id.as_ref(), selected_traffic.as_ref())
    {
        return render_traffic(id, result.clone());
    }
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
                    let deadline = Instant::now() + Duration::from_millis(250);
                    for job in jobs {
                        let state = match traffic_status_until(&job.job_id, deadline) {
                            TrafficQuery::Unsupported | TrafficQuery::NotFound => {
                                if job.phase == Phase::Registered {
                                    "registered"
                                } else {
                                    label(job.phase)
                                }
                            }
                            TrafficQuery::Found(status) => traffic_label(&status),
                            TrafficQuery::Unavailable => "unknown",
                        };
                        println!(
                            "Job: {}\nState: {}\nRegistered (Unix ms): {}\nUpdated (Unix ms): {}\nReason: {}\nLast known: true",
                            job.job_id.as_str(),
                            state,
                            job.registered_at_ms,
                            job.updated_at_ms,
                            json!(job.reason).as_str().unwrap_or("IDENTITY_UNAVAILABLE")
                        );
                    }
                    if truncated {
                        println!("Truncated: showing the 50 most recent records.");
                    }
                    println!("Scheduling: see job status <JOB_ID> for details");
                    ExitCode::SUCCESS
                }
                StatusResult::Found { job } => {
                    println!("Job: {}\nState: {}\nIdentity: {}\nRegistered (Unix ms): {}\nObserved (Unix ms): {}",
                    job.job_id.as_str(), if matches!(selected_traffic, Some(TrafficQuery::Unavailable)) { "unknown" } else { label(job.phase) },
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
                    if matches!(selected_traffic, Some(TrafficQuery::Unavailable)) {
                        println!("Scheduling: status unavailable");
                    } else {
                        println!("Scheduling: unavailable for this record");
                    }
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

enum TrafficQuery {
    Found(arc_ipc::traffic::StatusResult),
    NotFound,
    Unsupported,
    Unavailable,
}

fn traffic_status(id: &JobId) -> TrafficQuery {
    traffic_status_until(id, Instant::now() + Duration::from_secs(2))
}

fn traffic_status_until(id: &JobId, deadline: Instant) -> TrafficQuery {
    let Ok(socket) = arc_core::socket_path() else {
        return TrafficQuery::Unavailable;
    };
    let request = arc_ipc::traffic::JobRequest::StatusV2 { job_id: id.clone() };
    let value = match super::exec::transport::call_until(
        &socket,
        arc_ipc::traffic::OPERATION_TRAFFIC_JOB,
        json!(request),
        deadline,
        &|| false,
    ) {
        Ok(value) => value,
        Err(super::exec::transport::Failure::Unsupported) => return TrafficQuery::Unsupported,
        Err(_) => return TrafficQuery::Unavailable,
    };
    let Ok(result) = serde_json::from_value::<arc_ipc::traffic::StatusResult>(value) else {
        return TrafficQuery::Unavailable;
    };
    if !result.is_valid() {
        return TrafficQuery::Unavailable;
    }
    if matches!(result, arc_ipc::traffic::StatusResult::NotFound {}) {
        TrafficQuery::NotFound
    } else {
        TrafficQuery::Found(result)
    }
}

fn traffic_label(status: &arc_ipc::traffic::StatusResult) -> &'static str {
    use arc_ipc::traffic::StatusResult;
    match status {
        StatusResult::Queued { .. } => "queued",
        StatusResult::Reserved { .. } => "starting",
        StatusResult::Running { .. } => "running",
        StatusResult::Completed { outcome, .. } => match outcome {
            Outcome::Exited { .. } => "completed",
            Outcome::Signaled { .. } => "terminated by signal",
            Outcome::SpawnFailed { .. } => "spawn failed",
        },
        StatusResult::Cancelled {} => "cancelled",
        StatusResult::Abandoned {} | StatusResult::Unconfirmed {} | StatusResult::NotFound {} => {
            "outcome unknown"
        }
    }
}

fn render_traffic(id: &JobId, status: arc_ipc::traffic::StatusResult) -> ExitCode {
    use arc_ipc::traffic::StatusResult;
    println!("Job: {}", id.as_str());
    match status {
        StatusResult::Queued { .. } => println!("State: queued"),
        StatusResult::Reserved { .. } => println!("State: starting"),
        StatusResult::Running { queue_wait_ms } => {
            println!("State: running\nQueue wait (ms): {queue_wait_ms}")
        }
        StatusResult::Completed {
            queue_wait_ms,
            outcome,
        } => {
            match outcome {
                Outcome::Exited { code } => println!("State: completed\nExit code: {code}"),
                Outcome::Signaled {
                    signal,
                    core_dumped,
                } => println!(
                    "State: terminated by signal\nSignal: {signal}\nCore dumped: {core_dumped}"
                ),
                Outcome::SpawnFailed { .. } => {
                    println!("State: spawn failed\nReason: SPAWN_FAILED")
                }
            }
            println!("Queue wait (ms): {queue_wait_ms}");
        }
        StatusResult::Cancelled {} => println!("State: cancelled"),
        StatusResult::Abandoned {} => println!("State: outcome unknown"),
        StatusResult::Unconfirmed {} => println!("State: outcome unknown\nReservation: retained"),
        StatusResult::NotFound {} => {
            return error(
                "JOB_NOT_FOUND",
                "No lifecycle record was found for this ID.",
            )
        }
    }
    println!("Last known: true");
    ExitCode::SUCCESS
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

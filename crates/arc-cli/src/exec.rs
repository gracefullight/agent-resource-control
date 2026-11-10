//! One CLI-owned native child, independent of every metadata retry.
mod descriptors;
mod preparation;
mod signals;
mod terminal;
pub(crate) mod transport;
use arc_core::process_identity::random_job_id;
use arc_ipc::jobs::{
    CompleteRequest, CompleteResult, JobId, Outcome, ProcessIdentity, SpawnReason, StartRequest,
    StartResult, OPERATION_JOB_COMPLETE, OPERATION_JOB_START,
};
use serde_json::json;
use std::{
    ffi::OsString,
    io,
    os::unix::process::ExitStatusExt,
    path::Path,
    process::{Command, ExitCode},
    time::{Duration, Instant},
};

pub(crate) fn invalid() -> ExitCode {
    eprintln!("INVALID_REQUEST\nUse arcctl exec -- <PROGRAM> [ARG...]. No child was started.");
    ExitCode::from(2)
}
fn start_until(
    socket: &Path,
    input: StartRequest,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
) -> Result<StartResult, transport::Failure> {
    use transport::Failure;
    let value = transport::call_until(
        socket,
        OPERATION_JOB_START,
        json!(input),
        deadline,
        cancelled,
    )?;
    let result: StartResult = serde_json::from_value(value).map_err(|_| Failure::InvalidReply)?;
    if result.job.job_id != *input.job_id()
        || result.job.observer != *input.observer()
        || !result.job.is_valid()
    {
        return Err(Failure::InvalidReply);
    }
    match &input {
        StartRequest::Register { classification, .. }
            if result.job.classification != *classification =>
        {
            return Err(Failure::InvalidReply)
        }
        StartRequest::Attach { child, .. } if result.job.child.as_ref() != Some(child) => {
            return Err(Failure::InvalidReply)
        }
        _ => (),
    }
    if cancelled() {
        return Err(Failure::Cancelled);
    }
    if Instant::now() >= deadline {
        return Err(Failure::Uncertain);
    }
    Ok(result)
}
pub(crate) fn run(argv: Vec<OsString>) -> ExitCode {
    if argv.is_empty()
        || argv[0].is_empty()
        || argv.iter().any(|s| s.as_encoded_bytes().contains(&0))
    {
        return invalid();
    }
    let deadline = Instant::now() + Duration::from_millis(1800);
    let Ok(socket) = arc_core::socket_path() else {
        return preparation::stopped(transport::Failure::Local);
    };
    let shared_job = terminal::shares_job();
    let Ok(signals) = signals::Signals::install(shared_job) else {
        return preparation::stopped(transport::Failure::Local);
    };
    let Ok(observer) = ProcessIdentity::capture(std::process::id()) else {
        return preparation::stopped(transport::Failure::Local);
    };
    let Ok(job_id) = random_job_id().and_then(|s| JobId::try_from(s).map_err(io::Error::other))
    else {
        return preparation::stopped(transport::Failure::Local);
    };
    let classification = arc_detector::classify_argv(&argv);
    let tracking = match preparation::prepare(
        &socket,
        &job_id,
        &observer,
        classification,
        deadline,
        &signals,
    ) {
        Ok(tracking) => tracking,
        Err(error) => return preparation::stopped(error),
    };
    let mut command = Command::new(&argv[0]);
    use std::os::unix::process::CommandExt;
    command.args(&argv[1..]);
    if !shared_job {
        command.process_group(0);
    }
    signals.prepare_child(&mut command);
    descriptors::restore_in_child(&mut command);
    let spawned = match preparation::admit(|| !signals.admit_spawn(), || command.spawn()) {
        Ok(spawned) => spawned,
        Err(error) => return preparation::stopped(error),
    };
    let mut child = match spawned {
        Ok(child) => child,
        Err(error) if error.kind() == io::ErrorKind::Interrupted && signals.interrupted() => {
            return preparation::stopped(transport::Failure::Cancelled);
        }
        Err(error) => {
            let reason = match error.kind() {
                io::ErrorKind::NotFound => SpawnReason::NotFound,
                io::ErrorKind::PermissionDenied => SpawnReason::PermissionDenied,
                io::ErrorKind::InvalidInput => SpawnReason::InvalidInput,
                io::ErrorKind::WouldBlock | io::ErrorKind::OutOfMemory => {
                    SpawnReason::ResourceUnavailable
                }
                _ => SpawnReason::Other,
            };
            if tracking == preparation::Tracking::Managed {
                let _ = complete(
                    &socket,
                    &job_id,
                    &observer,
                    Outcome::SpawnFailed {
                        reason,
                        os_code: error.raw_os_error(),
                    },
                );
            }
            eprintln!("SPAWN_FAILED\nNo child was started. Check the executable and execution permissions.");
            return ExitCode::FAILURE;
        }
    };
    signals.attach(child.id());
    let mut diagnostics = preparation::started(tracking, &job_id);
    // Do not reap before capture: native identity remains available for a fast
    // exit's zombie, without pretending it is a currently running process.
    let attached = tracking == preparation::Tracking::Managed
        && ProcessIdentity::capture(child.id())
            .ok()
            .and_then(|identity| {
                start_until(
                    &socket,
                    StartRequest::Attach {
                        job_id: job_id.clone(),
                        observer: observer.clone(),
                        child: identity,
                    },
                    Instant::now() + Duration::from_secs(2),
                    &|| false,
                )
                .ok()
            })
            .is_some();
    if tracking == preparation::Tracking::Managed && !attached {
        preparation::uncertain(&mut diagnostics, &job_id);
    }
    let status = loop {
        match signals.wait(&mut child) {
            Ok(status) => break status,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => (),
            Err(_) => {
                signals.reaped();
                diagnostics.emit(format_args!("OUTCOME_UNKNOWN\nThe child outcome is unknown. Do not launch another copy to recover tracking."));
                diagnostics.finish();
                return ExitCode::FAILURE;
            }
        }
    };
    signals.reaped();
    let outcome = match status.code() {
        Some(code) => Outcome::Exited { code: code as u8 },
        None => Outcome::Signaled {
            signal: status.signal().unwrap_or(libc::SIGTERM) as u8,
            core_dumped: status.core_dumped(),
        },
    };
    if tracking == preparation::Tracking::Managed
        && complete(&socket, &job_id, &observer, outcome).is_err()
    {
        let observed = status.code().map_or_else(
            || {
                format!(
                    "The child terminated by signal {}.",
                    status.signal().unwrap_or(0)
                )
            },
            |code| format!("The child exited with code {code}."),
        );
        diagnostics.emit(format_args!("COMPLETION_UNCONFIRMED\nJob: {}\n{observed} Recording is unconfirmed. Inspect arcctl job status for this job. Do not launch another copy.", job_id.as_str()));
    }
    diagnostics.finish();
    drop(signals);
    if let Some(signal) = status.signal() {
        signals::reraise(signal);
    }
    ExitCode::from(status.code().unwrap_or(1) as u8)
}
fn complete(
    socket: &Path,
    job_id: &JobId,
    observer: &ProcessIdentity,
    outcome: Outcome,
) -> Result<(), ()> {
    let request = CompleteRequest {
        job_id: job_id.clone(),
        observer: observer.clone(),
        outcome,
    };
    let value = transport::call(socket, OPERATION_JOB_COMPLETE, json!(request))?;
    let result: CompleteResult = serde_json::from_value(value).map_err(|_| ())?;
    if !result.job.is_valid()
        || result.job.job_id != request.job_id
        || result.job.observer != request.observer
        || result.job.outcome.as_ref() != Some(&request.outcome)
    {
        return Err(());
    }
    Ok(())
}

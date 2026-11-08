//! One CLI-owned native child, independent of every metadata retry.
mod descriptors;
mod signals;
mod terminal;
pub(crate) mod transport;
use arc_core::process_identity::random_job_id;
use arc_ipc::jobs::{
    CompleteRequest, CompleteResult, JobId, Outcome, Phase, ProcessIdentity, SpawnReason,
    StartRequest, StartResult, OPERATION_JOB_COMPLETE, OPERATION_JOB_START,
};
use serde_json::json;
use std::{
    ffi::OsString,
    io,
    os::unix::process::ExitStatusExt,
    path::Path,
    process::{Command, ExitCode},
};

pub(crate) fn invalid() -> ExitCode {
    eprintln!("INVALID_REQUEST\nUse arcctl exec -- <PROGRAM> [ARG...]. No child was started.");
    ExitCode::from(2)
}
fn not_started() -> ExitCode {
    eprintln!("NOT_STARTED\nNo child was started. Check the ARC daemon connection.");
    ExitCode::FAILURE
}
fn start(socket: &Path, input: StartRequest) -> Result<StartResult, ()> {
    let value = transport::call(socket, OPERATION_JOB_START, json!(input))?;
    let result: StartResult = serde_json::from_value(value).map_err(|_| ())?;
    if result.job.job_id != *input.job_id()
        || result.job.observer != *input.observer()
        || !result.job.is_valid()
    {
        return Err(());
    }
    match &input {
        StartRequest::Register { classification, .. }
            if result.job.classification != *classification =>
        {
            return Err(())
        }
        StartRequest::Attach { child, .. } if result.job.child.as_ref() != Some(child) => {
            return Err(())
        }
        _ => (),
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
    let Ok(socket) = arc_core::socket_path() else {
        return not_started();
    };
    let shared_job = terminal::shares_job();
    let Ok(signals) = signals::Signals::install(shared_job) else {
        return not_started();
    };
    let Ok(observer) = ProcessIdentity::capture(std::process::id()) else {
        return not_started();
    };
    let Ok(job_id) = random_job_id().and_then(|s| JobId::try_from(s).map_err(io::Error::other))
    else {
        return not_started();
    };
    let classification = arc_detector::classify_argv(&argv);
    let registered = start(
        &socket,
        StartRequest::Register {
            job_id: job_id.clone(),
            observer: observer.clone(),
            classification,
        },
    );
    if !matches!(registered, Ok(ref r) if r.job.phase == Phase::Registered && r.job.classification == classification)
    {
        return not_started();
    }
    let claimed = start(
        &socket,
        StartRequest::Claim {
            job_id: job_id.clone(),
            observer: observer.clone(),
        },
    );
    // An UNCHANGED claim is safe only in this still-live invocation before its
    // one spawn site. A new invocation always allocates a fresh random key.
    if !matches!(claimed, Ok(ref r) if r.job.phase == Phase::Launching) || signals.interrupted() {
        return not_started();
    }
    let mut command = Command::new(&argv[0]);
    use std::os::unix::process::CommandExt;
    command.args(&argv[1..]);
    if !shared_job {
        command.process_group(0);
    }
    signals.prepare_child(&mut command);
    descriptors::restore_in_child(&mut command);
    let mut child = match command.spawn() {
        Ok(child) => child,
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
            let _ = complete(
                &socket,
                &job_id,
                &observer,
                Outcome::SpawnFailed {
                    reason,
                    os_code: error.raw_os_error(),
                },
            );
            eprintln!("SPAWN_FAILED\nNo child was started. Check the executable and execution permissions.");
            return ExitCode::FAILURE;
        }
    };
    signals.attach(child.id());
    // Do not reap before capture: native identity remains available for a fast
    // exit's zombie, without pretending it is a currently running process.
    let attached = ProcessIdentity::capture(child.id())
        .ok()
        .and_then(|identity| {
            start(
                &socket,
                StartRequest::Attach {
                    job_id: job_id.clone(),
                    observer: observer.clone(),
                    child: identity,
                },
            )
            .ok()
        })
        .is_some();
    if !attached {
        eprintln!("TRACKING_UNCERTAIN\nJob: {}\nLifecycle tracking is uncertain. Inspect arcctl job status for this job. Do not launch another copy.", job_id.as_str());
    }
    let status = loop {
        match signals.wait(&mut child) {
            Ok(status) => break status,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => (),
            Err(_) => {
                signals.reaped();
                eprintln!("OUTCOME_UNKNOWN\nJob: {}\nThe child outcome is unknown. Inspect arcctl job status for this job. Do not launch another copy.", job_id.as_str());
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
    if complete(&socket, &job_id, &observer, outcome).is_err() {
        let observed = status.code().map_or_else(
            || {
                format!(
                    "The child terminated by signal {}.",
                    status.signal().unwrap_or(0)
                )
            },
            |code| format!("The child exited with code {code}."),
        );
        eprintln!("COMPLETION_UNCONFIRMED\nJob: {}\n{observed} Recording is unconfirmed. Inspect arcctl job status for this job. Do not launch another copy.", job_id.as_str());
    }
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

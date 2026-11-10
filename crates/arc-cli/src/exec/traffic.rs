//! The CLI owns one spawn; Traffic only coordinates its admission and metadata.
use super::{
    preparation::{self, Tracking},
    signals::Signals,
    transport::{self, Failure},
    witness::Witness,
};
use arc_ipc::{
    jobs::{JobId, Outcome, ProcessIdentity},
    traffic::{
        Capabilities, JobRequest, JobResult, LineageContext, CAPABILITY,
        OPERATION_TRAFFIC_CAPABILITIES, OPERATION_TRAFFIC_JOB,
    },
};
use serde_json::json;
use std::{
    path::Path,
    time::{Duration, Instant},
};

pub(super) const CONTEXT_ENV: &str = "ARC_TRAFFIC_LINEAGE";

pub(super) enum Mode {
    Legacy(Tracking),
    Scheduled {
        generation: u64,
        context: LineageContext,
        waited: bool,
        witness: Witness,
    },
    Uncertain(Witness),
}

fn job(
    socket: &Path,
    request: &JobRequest,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
) -> Result<JobResult, Failure> {
    let value = transport::call_until(
        socket,
        OPERATION_TRAFFIC_JOB,
        json!(request),
        deadline,
        cancelled,
    )?;
    let result: JobResult = serde_json::from_value(value).map_err(|_| Failure::InvalidReply)?;
    if !result.is_valid() {
        return Err(Failure::InvalidReply);
    }
    Ok(result)
}

pub(super) fn prepare(
    socket: &Path,
    id: &JobId,
    observer: &ProcessIdentity,
    classification: arc_ipc::Classification,
    deadline: Instant,
    signals: &Signals,
) -> Result<Mode, Failure> {
    use arc_detector::{Locality, Reason};
    if classification.locality != Locality::HostLocal
        || !matches!(
            classification.reason,
            Reason::CargoBuild | Reason::CargoTest | Reason::CargoClippy | Reason::CargoFmt
        )
    {
        return preparation::prepare(socket, id, observer, classification, deadline, signals)
            .map(Mode::Legacy);
    }
    let cancelled = || signals.interrupted();
    let probe = transport::call_until(
        socket,
        OPERATION_TRAFFIC_CAPABILITIES,
        serde_json::Value::Null,
        deadline,
        &cancelled,
    );
    match probe {
        Err(Failure::Unsupported) => {
            return preparation::prepare(socket, id, observer, classification, deadline, signals)
                .map(Mode::Legacy)
        }
        Err(Failure::Unavailable) => return Ok(Mode::Legacy(Tracking::Unavailable)),
        Err(Failure::Uncertain | Failure::Busy) => return Ok(Mode::Legacy(Tracking::Uncertain)),
        Err(error) => return Err(error),
        Ok(value) => {
            let capabilities: Capabilities =
                serde_json::from_value(value).map_err(|_| Failure::InvalidReply)?;
            if capabilities.capability != CAPABILITY {
                return Err(Failure::InvalidReply);
            }
        }
    }
    let inherited = std::env::var(CONTEXT_ENV)
        .ok()
        .and_then(|v| LineageContext::try_from(v).ok());
    // An otherwise unused opaque lineage claim gives the daemon an
    // owner-bound key even when the enqueue acknowledgement is lost.
    // A claim alone never authorizes permit reuse; ancestry is still checked.
    let claim = match inherited.clone() {
        Some(value) => value,
        None => {
            let first = arc_core::process_identity::random_job_id().map_err(|_| Failure::Local)?;
            let second = arc_core::process_identity::random_job_id().map_err(|_| Failure::Local)?;
            LineageContext::try_from(format!("{first}{second}")).map_err(|_| Failure::Local)?
        }
    };
    let witness = match Witness::create(socket, id, &claim, observer) {
        Ok(witness) => witness,
        // A healthy daemon must not grant work without durable child evidence.
        Err(_) => return Err(Failure::Witness),
    };
    witness.arm();
    let enqueue = JobRequest::Enqueue {
        job_id: id.clone(),
        observer: observer.clone(),
        classification,
        lineage_context: Some(claim.clone()),
    };
    let mut result = match job(socket, &enqueue, deadline, &cancelled) {
        Ok(result) => result,
        Err(
            Failure::Unavailable | Failure::Uncertain | Failure::Busy | Failure::InvalidEndpoint,
        ) => return Ok(Mode::Uncertain(witness)),
        Err(Failure::Cancelled) => {
            witness.finish_unspawned(|| cancel(socket, id, observer, None));
            return Err(Failure::Cancelled);
        }
        Err(error) => {
            witness.finish_unspawned(|| cancel(socket, id, observer, None));
            return Err(error);
        }
    };
    let mut waited = false;
    loop {
        match result {
            JobResult::Granted {
                permit_generation,
                lineage_context,
                ..
            } => {
                return Ok(Mode::Scheduled {
                    generation: permit_generation,
                    context: lineage_context,
                    waited,
                    witness,
                })
            }
            JobResult::Reused {
                permit_generation, ..
            } => {
                return Ok(Mode::Scheduled {
                    generation: permit_generation,
                    context: claim,
                    waited,
                    witness,
                });
            }
            JobResult::Bypassed { .. } => {
                witness.discard();
                return preparation::prepare(
                    socket,
                    id,
                    observer,
                    classification,
                    Instant::now() + Duration::from_millis(1800),
                    signals,
                )
                .map(Mode::Legacy);
            }
            JobResult::Queued { .. } => {
                if !waited {
                    queued_notice(id);
                    waited = true;
                }
                // The request and SQLite ticket have both been released here.
                std::thread::sleep(Duration::from_millis(40));
                if cancelled() {
                    witness.finish_unspawned(|| cancel(socket, id, observer, None));
                    return Err(Failure::Cancelled);
                }
                // An inherited context is only a claim. Re-enqueue is idempotent
                // and lets the daemon reverify ancestry on every wait cycle.
                let poll = if inherited.is_some() {
                    enqueue.clone()
                } else {
                    JobRequest::Poll {
                        job_id: id.clone(),
                        observer: observer.clone(),
                    }
                };
                result = match job(
                    socket,
                    &poll,
                    Instant::now() + Duration::from_millis(1800),
                    &cancelled,
                ) {
                    Ok(value) => value,
                    Err(
                        Failure::Unavailable
                        | Failure::Uncertain
                        | Failure::Busy
                        | Failure::InvalidEndpoint,
                    ) => return Ok(Mode::Uncertain(witness)),
                    Err(Failure::Cancelled) => {
                        witness.finish_unspawned(|| cancel(socket, id, observer, None));
                        return Err(Failure::Cancelled);
                    }
                    Err(error) => {
                        witness.finish_unspawned(|| cancel(socket, id, observer, None));
                        return Err(error);
                    }
                };
            }
            _ => {
                witness.finish_unspawned(|| cancel(socket, id, observer, None));
                return Err(Failure::InvalidReply);
            }
        }
    }
}

/// Write once without changing the inherited descriptor's shared status flags.
fn queued_notice(id: &JobId) {
    let message = format!("QUEUED\nWaiting to start.\nReference: {}\n", id.as_str());
    write_notice(&message);
}

fn write_notice(message: &str) {
    // A duplicate descriptor shares O_NONBLOCK with the caller and siblings.
    // A short-lived writer instead performs one blocking write; it is reaped
    // before returning, so a delayed notice cannot follow the target spawn.
    // The helper also owns a timeout in case its parent is killed mid-write.
    unsafe {
        let mut all = std::mem::zeroed();
        let mut original = std::mem::zeroed();
        libc::sigfillset(&mut all);
        if libc::pthread_sigmask(libc::SIG_BLOCK, &all, &mut original) != 0 {
            return;
        }
        let pid = libc::fork();
        if pid == 0 {
            // Only async-signal-safe operations may run between fork and exit.
            // Restore the default action even if the wrapper inherited a custom
            // SIGALRM handler, and unblock only this helper's timeout signal.
            let mut action: libc::sigaction = std::mem::zeroed();
            action.sa_sigaction = libc::SIG_DFL;
            let mut timeout_signal = std::mem::zeroed();
            if libc::sigemptyset(&mut action.sa_mask) != 0
                || libc::sigaction(libc::SIGALRM, &action, std::ptr::null_mut()) != 0
                || libc::sigemptyset(&mut timeout_signal) != 0
                || libc::sigaddset(&mut timeout_signal, libc::SIGALRM) != 0
                || libc::sigprocmask(libc::SIG_UNBLOCK, &timeout_signal, std::ptr::null_mut()) != 0
            {
                libc::_exit(1);
            }
            libc::alarm(1);
            libc::write(libc::STDERR_FILENO, message.as_ptr().cast(), message.len());
            libc::_exit(0);
        }
        libc::pthread_sigmask(libc::SIG_SETMASK, &original, std::ptr::null_mut());
        if pid < 0 {
            return;
        }
        let deadline = Instant::now() + Duration::from_millis(50);
        let mut status = 0;
        loop {
            let result = libc::waitpid(pid, &mut status, libc::WNOHANG);
            if result == pid
                || (result < 0
                    && std::io::Error::last_os_error().raw_os_error() != Some(libc::EINTR))
            {
                return;
            }
            if Instant::now() >= deadline {
                libc::kill(pid, libc::SIGKILL);
                while libc::waitpid(pid, &mut status, 0) < 0
                    && std::io::Error::last_os_error().raw_os_error() == Some(libc::EINTR)
                {
                }
                return;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    }
}

pub(super) fn cancel(
    socket: &Path,
    id: &JobId,
    observer: &ProcessIdentity,
    generation: Option<u64>,
) -> bool {
    let request = JobRequest::Cancel {
        job_id: id.clone(),
        observer: observer.clone(),
        permit_generation: generation,
    };
    let result = job(
        socket,
        &request,
        Instant::now() + Duration::from_millis(500),
        &|| false,
    );
    let confirmed = matches!(result, Ok(JobResult::Cancelled {}));
    if !confirmed {
        write_notice("QUEUE_CLEANUP_UNCONFIRMED\nQueue cleanup is unconfirmed.\n");
    }
    confirmed
}

pub(super) fn attach(
    socket: &Path,
    id: &JobId,
    observer: &ProcessIdentity,
    generation: u64,
    child: ProcessIdentity,
) -> bool {
    let request = JobRequest::Attach {
        job_id: id.clone(),
        observer: observer.clone(),
        child,
        permit_generation: generation,
    };
    matches!(
        job(
            socket,
            &request,
            Instant::now() + Duration::from_secs(2),
            &|| false
        ),
        Ok(JobResult::Running {})
    )
}

pub(super) fn complete(
    socket: &Path,
    id: &JobId,
    observer: &ProcessIdentity,
    generation: u64,
    outcome: Outcome,
) -> bool {
    let request = JobRequest::Complete {
        job_id: id.clone(),
        observer: observer.clone(),
        outcome: outcome.clone(),
        permit_generation: generation,
    };
    matches!(job(socket, &request, Instant::now() + Duration::from_secs(2), &|| false), Ok(JobResult::Completed { outcome: actual, .. }) if actual == outcome)
}

pub(super) fn complete_or_warn(
    socket: &Path,
    id: &JobId,
    observer: &ProcessIdentity,
    generation: u64,
    outcome: Outcome,
) -> bool {
    let completed = complete(socket, id, observer, generation, outcome);
    if !completed {
        write_notice("COMPLETION_UNCONFIRMED\nRecording is unconfirmed.\n");
    }
    completed
}

pub(super) fn fallback(
    socket: &Path,
    id: &JobId,
    observer: &ProcessIdentity,
    child: Option<ProcessIdentity>,
    outcome: Option<Outcome>,
) -> bool {
    let request = JobRequest::Fallback {
        job_id: id.clone(),
        observer: observer.clone(),
        permit_generation: None,
        child,
        outcome,
    };
    matches!(
        job(
            socket,
            &request,
            Instant::now() + Duration::from_secs(2),
            &|| false,
        ),
        Ok(JobResult::Completed { .. } | JobResult::Abandoned {} | JobResult::Cancelled {})
    )
}

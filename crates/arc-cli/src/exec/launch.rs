//! Final native launch fence. READY is not proof of target execution.
use super::{
    preparation::write_notice,
    signals::Signals,
    traffic::Mode,
    transport::{Continuity, Failure},
};
use arc_ipc::{
    client::Session,
    jobs::{JobId, Outcome, ProcessIdentity, SpawnReason},
    policy::{launch::*, ObservationAvailability},
    Request,
};
use serde_json::json;
use std::{
    io,
    os::unix::process::CommandExt,
    path::Path,
    process::Command,
    time::{Duration, Instant},
};

pub(super) enum Coordination {
    Fenced(Gate),
    Legacy,
    Outage,
}
pub(super) struct Gate {
    session: Session,
    request: GateRequest,
    waited: bool,
    deadline: Instant,
}

enum GateRequest {
    Legacy(LaunchRequest),
    Governor {
        request: arc_ipc::governor::GovernorLaunchRequest,
        result: arc_ipc::governor::GovernorLaunchResult,
    },
}
mod checkpoint;
mod governor;

/// Delay-only cancellation fixture before the first durable submission. The
/// existing explicit acceptance controls are inert in ordinary execution.
pub(super) fn before_enqueue(
    value: &serde_json::Value,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Failure> {
    checkpoint::pause("before_enqueue", value, cancelled)
}

pub(super) fn after_queue(
    value: &serde_json::Value,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Failure> {
    checkpoint::pause("queued", value, cancelled)
}

/// Delay-only acceptance seam after native spawn, before ACK/attachment. A
/// failed or cancelled checkpoint cannot turn an existing child into unspawned.
pub(super) fn after_spawn(id: &JobId, child_pid: u32, signals: &Signals) {
    let _ = checkpoint::pause(
        "spawned",
        &json!({"job_id":id,"child_pid":child_pid}),
        &|| signals.interrupted(),
    );
}

pub(super) fn prepare(
    socket: &Path,
    id: &JobId,
    observer: &ProcessIdentity,
    mode: &mut Mode,
    signals: &Signals,
) -> Result<Coordination, Failure> {
    if matches!(
        mode,
        Mode::Unreserved { .. }
            | Mode::Scheduled {
                route: super::traffic::Route::Governor | super::traffic::Route::BunScripts,
                ..
            }
    ) {
        return governor::prepare(socket, id, observer, mode, signals);
    }
    let Mode::Scheduled {
        generation,
        continuity,
        ..
    } = mode
    else {
        return Ok(Coordination::Legacy);
    };
    let request = LaunchRequest {
        job_id: id.clone(),
        observer: observer.clone(),
        permit_generation: *generation,
    };
    checkpoint::pause("reserved", &json!(request), &|| signals.interrupted())?;
    wait(
        socket,
        request,
        continuity,
        &|| signals.interrupted(),
        || {
            write_notice("QUEUED\nWaiting for memory.\nStarts automatically when memory permits; other queue rules still apply.\n")
        },
    )
}

fn wait(
    socket: &Path,
    input: LaunchRequest,
    continuity: &Continuity,
    cancelled: &dyn Fn() -> bool,
    mut announce: impl FnMut(),
) -> Result<Coordination, Failure> {
    let mut held = false;
    let mut synchronized = false;
    loop {
        if cancelled() {
            return Err(Failure::Cancelled);
        }
        let started = Instant::now();
        let mut session = match Session::connect_until(
            socket,
            started + Duration::from_millis(2000 + LAUNCH_ACK_GRACE_MS),
            cancelled,
            Some(continuity),
        ) {
            Ok(session) => session,
            Err(Failure::Unavailable) if !held => return Ok(Coordination::Outage),
            Err(error) => return Err(error),
        };
        // Independent of display-capability negotiation. No retries on this
        // connection: a lost release never causes another native spawn.
        let mut request = Request::new(crate::request_id(), OPERATION_TRAFFIC_LAUNCH);
        request.payload = json!(input);
        let value = match session.exchange(&request, cancelled) {
            Err(Failure::Unsupported) if !held => return Ok(Coordination::Legacy),
            Err(Failure::Busy) => {
                drop(session);
                std::thread::sleep(Duration::from_millis(20));
                continue;
            }
            other => other?,
        };
        let result = decode_launch_result(&value).map_err(|_| Failure::InvalidReply)?;
        match result {
            LaunchResult::Held { .. } => {
                if !held {
                    announce();
                    held = true;
                }
                drop(session);
                std::thread::sleep(Duration::from_millis(40));
            }
            LaunchResult::Ready {
                permit_generation,
                policy,
            } if permit_generation == input.permit_generation => {
                let mut deadline = started + Duration::from_secs(2);
                if policy.observation.availability == ObservationAvailability::Available {
                    let remaining = policy
                        .observation
                        .stale_after_ms
                        .zip(policy.observation.age_ms)
                        .and_then(|(threshold, age)| threshold.checked_sub(age))
                        .ok_or(Failure::InvalidReply)?;
                    deadline = deadline.min(started + Duration::from_millis(remaining));
                }
                if !synchronized {
                    checkpoint::pause(
                        "ready",
                        &json!({"request":input,"policy":policy}),
                        cancelled,
                    )?;
                    synchronized = true;
                }
                let mut gate = Gate {
                    session,
                    request: GateRequest::Legacy(input.clone()),
                    waited: held,
                    deadline,
                };
                if Instant::now() >= deadline {
                    // No command was installed or spawned. A confirmed negative
                    // ACK retracts the marker; uncertainty must never retry.
                    if !gate.acknowledge(None) {
                        return Err(Failure::Uncertain);
                    }
                    drop(gate);
                    continue;
                }
                return Ok(Coordination::Fenced(gate));
            }
            LaunchResult::Reused { permit_generation }
                if permit_generation == input.permit_generation =>
            {
                return Ok(Coordination::Legacy)
            }
            LaunchResult::Cancelled {} => return Err(Failure::Cancelled),
            LaunchResult::NotFound {} => return Err(Failure::Rejected),
            _ => return Err(Failure::InvalidReply),
        }
    }
}
impl Gate {
    pub(super) fn waited(&self) -> bool {
        self.waited
    }
    pub(super) fn install(&self, command: &mut Command) {
        let fd = self.session.raw_fd();
        let deadline = self.deadline.min(self.session.deadline());
        // SAFETY: only a monotonic clock read, nonblocking socket syscall,
        // errno inspection and stack storage occur between fork and exec.
        // CLOEXEC closes the inherited transport descriptor at native exec.
        unsafe {
            command.pre_exec(move || {
                if Instant::now() >= deadline {
                    return Err(io::Error::from_raw_os_error(libc::ETIMEDOUT));
                }
                let mut byte = 0u8;
                let count = libc::recv(
                    fd,
                    (&mut byte as *mut u8).cast(),
                    1,
                    libc::MSG_PEEK | libc::MSG_DONTWAIT,
                );
                if count >= 0 {
                    return Err(io::Error::from_raw_os_error(libc::ECONNABORTED));
                }
                let error = io::Error::last_os_error();
                if error.kind() != io::ErrorKind::WouldBlock {
                    return Err(error);
                }
                if Instant::now() >= deadline {
                    return Err(io::Error::from_raw_os_error(libc::ETIMEDOUT));
                }
                Ok(())
            });
        }
    }
    pub(super) fn acknowledge(&mut self, child: Option<ProcessIdentity>) -> bool {
        use arc_ipc::governor::*;
        let (operation, payload) = match &self.request {
            GateRequest::Legacy(input) => {
                let ack = match child {
                    Some(child) => LaunchAck::Started {
                        job_id: input.job_id.clone(),
                        observer: input.observer.clone(),
                        permit_generation: input.permit_generation,
                        child,
                    },
                    None => LaunchAck::Unspawned {
                        job_id: input.job_id.clone(),
                        observer: input.observer.clone(),
                        permit_generation: input.permit_generation,
                    },
                };
                (OPERATION_TRAFFIC_LAUNCH_ACK, json!(ack))
            }
            GateRequest::Governor { request, result } => {
                let ack = match result {
                    GovernorLaunchResult::Unreserved { release_id, .. } => {
                        GovernorLaunchAck::Unreserved {
                            ack: match child {
                                Some(child) => GovernorUnreservedAck::Started {
                                    job_id: request.job_id().clone(),
                                    observer: request.observer().clone(),
                                    release_id: release_id.clone(),
                                    child,
                                },
                                None => GovernorUnreservedAck::Unspawned {
                                    job_id: request.job_id().clone(),
                                    observer: request.observer().clone(),
                                    release_id: release_id.clone(),
                                },
                            },
                        }
                    }
                    GovernorLaunchResult::Reserved { .. } => {
                        let GovernorLaunchRequest::Reserved { request } = request else {
                            return false;
                        };
                        GovernorLaunchAck::Reserved {
                            ack: match child {
                                Some(child) => LaunchAck::Started {
                                    job_id: request.job_id.clone(),
                                    observer: request.observer.clone(),
                                    permit_generation: request.permit_generation,
                                    child,
                                },
                                None => LaunchAck::Unspawned {
                                    job_id: request.job_id.clone(),
                                    observer: request.observer.clone(),
                                    permit_generation: request.permit_generation,
                                },
                            },
                        }
                    }
                    _ => return false,
                };
                if !ack.matches(request, result) {
                    return false;
                }
                (OPERATION_LAUNCH_ACK, json!(ack))
            }
        };
        let mut request = Request::new(crate::request_id(), operation);
        request.payload = payload;
        matches!(self.session.exchange(&request, &|| false), Ok(value) if value.as_object().is_some_and(|o| o.is_empty()))
    }
}

#[cfg(test)]
mod tests;

/// Preserve the native spawn error in the shared terminal outcome vocabulary.
pub(super) fn spawn_failure(error: &io::Error) -> Outcome {
    let reason = match error.kind() {
        io::ErrorKind::NotFound => SpawnReason::NotFound,
        io::ErrorKind::PermissionDenied => SpawnReason::PermissionDenied,
        io::ErrorKind::InvalidInput => SpawnReason::InvalidInput,
        io::ErrorKind::WouldBlock | io::ErrorKind::OutOfMemory => SpawnReason::ResourceUnavailable,
        _ => SpawnReason::Other,
    };
    Outcome::SpawnFailed {
        reason,
        os_code: error.raw_os_error(),
    }
}

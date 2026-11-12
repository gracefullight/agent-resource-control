//! One CLI-owned native child, independent of every metadata retry.
mod accounting;
mod descriptors;
mod launch;
mod lifecycle;
mod lineage;
use lifecycle::{complete, start_until};
pub(crate) mod policy;
mod preparation;
mod signals;
mod terminal;
mod traffic;
pub(crate) mod transport;
mod witness;
use arc_core::process_identity::random_job_id;
use arc_ipc::jobs::{JobId, Outcome, ProcessIdentity, SpawnReason, StartRequest};
use std::{
    ffi::OsString,
    io,
    os::unix::process::ExitStatusExt,
    process::{Command, ExitCode},
    time::{Duration, Instant},
};

pub(crate) use preparation::invalid;

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
    let mut tracking = match traffic::prepare(
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
    let (accounting_relation, accounting_record) =
        accounting::lineage(&socket, &observer, &job_id, &tracking);
    let mut command = Command::new(&argv[0]);
    use std::os::unix::process::CommandExt;
    command.args(&argv[1..]);
    // Keep the sidecar alive until the native child exits. New clients prefer
    // it while this wrapper is a live ancestor.
    let lineage_record = match &tracking {
        traffic::Mode::Scheduled {
            context,
            generation,
            witness,
            route,
            continuity,
            ..
        } => match lineage::Lineage::create(&socket, &observer, context) {
            Ok(lineage) => Some(lineage),
            Err(_) => {
                witness.finish_unspawned(|| {
                    traffic::cancel(
                        &socket,
                        &job_id,
                        &observer,
                        Some(*generation),
                        *route,
                        continuity,
                    )
                });
                return preparation::stopped(transport::Failure::Local);
            }
        },
        _ => None,
    };
    // Protocol-1 clients cannot read the sidecar, and it is no longer an
    // ancestor after a wrapper dies. Preserve any caller-supplied value.
    if std::env::var_os(traffic::CONTEXT_ENV).is_none() {
        if let traffic::Mode::Scheduled { context, .. } = &tracking {
            command.env(traffic::CONTEXT_ENV, context.as_str());
        }
    }
    if !shared_job {
        command.process_group(0);
    }
    signals.prepare_child(&mut command);
    descriptors::restore_in_child(&mut command);
    match &tracking {
        traffic::Mode::Scheduled { witness, .. } | traffic::Mode::Uncertain { witness, .. } => {
            witness.install(&mut command);
        }
        _ => (),
    }
    let mut launch_gate = match launch::prepare(&socket, &job_id, &observer, &tracking, &signals) {
        Ok(launch::Coordination::Fenced(gate)) => {
            if gate.waited() {
                if let traffic::Mode::Scheduled { waited, .. } = &mut tracking {
                    *waited = true;
                }
            }
            gate.install(&mut command);
            Some(gate)
        }
        Ok(launch::Coordination::Legacy) => None,
        Ok(launch::Coordination::Outage) => {
            if let traffic::Mode::Scheduled {
                witness,
                route,
                continuity,
                ..
            } = tracking
            {
                tracking = traffic::Mode::Uncertain {
                    witness,
                    route,
                    continuity,
                };
            }
            None
        }
        Err(error) => {
            if let traffic::Mode::Scheduled {
                generation,
                witness,
                route,
                continuity,
                ..
            } = &tracking
            {
                witness.finish_unspawned(|| {
                    traffic::cancel(
                        &socket,
                        &job_id,
                        &observer,
                        Some(*generation),
                        *route,
                        continuity,
                    )
                });
            }
            return preparation::stopped(error);
        }
    };
    let mut spawned_at = Instant::now();
    let spawned = match preparation::admit(
        || !signals.admit_spawn(),
        || {
            spawned_at = Instant::now();
            command.spawn()
        },
    ) {
        Ok(spawned) => spawned,
        Err(error) => {
            // Release the launch fence before cancellation needs the writer.
            // Admission was denied locally, so no child could have spawned.
            if let Some(gate) = &mut launch_gate {
                let _ = gate.acknowledge(None);
            }
            drop(launch_gate.take());
            match &tracking {
                traffic::Mode::Scheduled {
                    generation,
                    witness,
                    route,
                    continuity,
                    ..
                } => {
                    witness.finish_unspawned(|| {
                        traffic::cancel(
                            &socket,
                            &job_id,
                            &observer,
                            Some(*generation),
                            *route,
                            continuity,
                        )
                    });
                }
                traffic::Mode::Uncertain {
                    witness,
                    route,
                    continuity,
                } => {
                    witness.finish_unspawned(|| {
                        traffic::cancel(&socket, &job_id, &observer, None, *route, continuity)
                            && *route != traffic::Route::Bun
                    });
                }
                _ => (),
            }
            return preparation::stopped(error);
        }
    };
    if let Some(gate) = &mut launch_gate {
        let confirmed = match &spawned {
            Ok(child) => ProcessIdentity::capture(child.id())
                .is_ok_and(|identity| gate.acknowledge(Some(identity))),
            Err(_) => gate.acknowledge(None),
        };
        if !confirmed {
            preparation::write_notice("LAUNCH_RECORDING_UNCONFIRMED\nDo not launch another copy. Inspect arcctl job status.\n");
        }
    }
    drop(launch_gate);
    let mut child = match spawned {
        Ok(child) => child,
        Err(error) if error.kind() == io::ErrorKind::Interrupted && signals.interrupted() => {
            match &tracking {
                traffic::Mode::Scheduled {
                    generation,
                    witness,
                    route,
                    continuity,
                    ..
                } => {
                    witness.finish_unspawned(|| {
                        traffic::cancel(
                            &socket,
                            &job_id,
                            &observer,
                            Some(*generation),
                            *route,
                            continuity,
                        )
                    });
                }
                traffic::Mode::Uncertain {
                    witness,
                    route,
                    continuity,
                } => {
                    witness.finish_unspawned(|| {
                        traffic::cancel(&socket, &job_id, &observer, None, *route, continuity)
                            && *route != traffic::Route::Bun
                    });
                }
                _ => (),
            }
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
            let outcome = Outcome::SpawnFailed {
                reason,
                os_code: error.raw_os_error(),
            };
            match &tracking {
                traffic::Mode::Legacy(preparation::Tracking::Managed) => {
                    let _ = complete(&socket, &job_id, &observer, outcome);
                }
                traffic::Mode::Scheduled {
                    generation,
                    witness,
                    route,
                    continuity,
                    ..
                } => {
                    witness.finish_spawn_failed(&outcome, || {
                        traffic::complete_or_warn(
                            &socket,
                            &job_id,
                            &observer,
                            *generation,
                            outcome.clone(),
                            *route,
                            continuity,
                        )
                    });
                }
                traffic::Mode::Uncertain {
                    witness,
                    route,
                    continuity,
                } => {
                    witness.finish_spawn_failed(&outcome, || {
                        traffic::fallback(
                            &socket,
                            &job_id,
                            &observer,
                            None,
                            Some(outcome.clone()),
                            *route,
                            continuity,
                        ) && *route != traffic::Route::Bun
                            && continuity.confirmed()
                    });
                }
                _ => (),
            }
            preparation::best_effort("SPAWN_FAILED\nNo child was started. Check the executable and execution permissions.");
            return ExitCode::FAILURE;
        }
    };
    signals.attach(child.id());
    let mut diagnostics = match &tracking {
        traffic::Mode::Legacy(mode) => preparation::started(*mode, &job_id),
        traffic::Mode::Uncertain { .. } => {
            preparation::started(preparation::Tracking::Uncertain, &job_id)
        }
        traffic::Mode::Scheduled { waited, .. } => {
            let mut diagnostics = preparation::Diagnostics::default();
            if *waited {
                diagnostics.emit(format_args!("STARTED\nCommand started."));
            }
            diagnostics
        }
    };
    // Do not reap before capture: native identity remains available for a fast
    // exit's zombie, without pretending it is a currently running process.
    let child_identity = ProcessIdentity::capture(child.id()).ok();
    let attached = match &tracking {
        traffic::Mode::Legacy(preparation::Tracking::Managed) => child_identity
            .clone()
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
            .is_some(),
        traffic::Mode::Scheduled {
            generation,
            route,
            continuity,
            ..
        } => child_identity.clone().is_some_and(|identity| {
            traffic::attach(
                &socket,
                &job_id,
                &observer,
                *generation,
                identity,
                *route,
                continuity,
            ) && continuity.confirmed()
        }),
        traffic::Mode::Uncertain {
            route, continuity, ..
        } => {
            let _ = traffic::fallback(
                &socket,
                &job_id,
                &observer,
                child_identity.clone(),
                None,
                *route,
                continuity,
            );
            true
        }
        _ => true,
    };
    if !attached {
        preparation::uncertain(&mut diagnostics, &job_id);
    }
    let termination = loop {
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
    let report_deadline = termination.observed_at + accounting::REPORT_WINDOW;
    // Registration-outage fallback has no authoritative child/terminal binding.
    let meter_report = if attached
        && matches!(
            &tracking,
            traffic::Mode::Legacy(preparation::Tracking::Managed) | traffic::Mode::Scheduled { .. }
        ) {
        accounting::report(
            &job_id,
            &observer,
            child_identity.as_ref(),
            accounting_relation,
            spawned_at,
            &termination,
        )
    } else {
        None
    };
    let status = termination.status;
    signals.reaped();
    let outcome = match status.code() {
        Some(code) => Outcome::Exited { code: code as u8 },
        None => Outcome::Signaled {
            signal: status.signal().unwrap_or(libc::SIGTERM) as u8,
            core_dumped: status.core_dumped(),
        },
    };
    let completed = match &tracking {
        traffic::Mode::Legacy(preparation::Tracking::Managed) => {
            complete(&socket, &job_id, &observer, outcome).is_ok()
        }
        traffic::Mode::Scheduled {
            generation,
            witness,
            route,
            continuity,
            ..
        } => {
            let completed = traffic::complete(
                &socket,
                &job_id,
                &observer,
                *generation,
                outcome,
                *route,
                continuity,
            );
            let completed = completed && continuity.confirmed();
            if completed {
                witness.discard();
            }
            completed
        }
        traffic::Mode::Uncertain {
            witness,
            route,
            continuity,
        } => {
            if traffic::fallback(
                &socket,
                &job_id,
                &observer,
                None,
                Some(outcome),
                *route,
                continuity,
            ) && *route != traffic::Route::Bun
                && continuity.confirmed()
            {
                witness.discard();
            }
            true
        }
        _ => true,
    };
    if !completed {
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
    if let Some(report) = meter_report {
        let _ = accounting::deliver(&socket, &report, report_deadline);
    }
    diagnostics.finish();
    drop(signals);
    drop(lineage_record);
    drop(accounting_record);
    if let Some(signal) = status.signal() {
        signals::reraise(signal);
    }
    ExitCode::from(status.code().unwrap_or(1) as u8)
}

//! One CLI-owned native child, independent of every metadata retry.
mod accounting;
mod descriptors;
mod governor;
mod launch;
mod lifecycle;
mod lineage;
mod metadata;
use lifecycle::{complete, start_until};
pub(crate) mod policy;
mod preparation;
mod signals;
mod terminal;
mod traffic;
pub(crate) mod transport;
mod witness;
use arc_core::process_identity::random_job_id;
use arc_ipc::jobs::{JobId, Outcome, ProcessIdentity, StartRequest};
use std::{
    ffi::OsString,
    io,
    os::unix::process::ExitStatusExt,
    process::{Command, ExitCode},
    time::{Duration, Instant},
};

pub(crate) use preparation::invalid;

pub(crate) fn run(argv: Vec<OsString>) -> ExitCode {
    if !preparation::valid_argv(&argv) {
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
    let classification = metadata::classify(&argv);
    let mut tracking = match governor::prepare(
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
    let mut launch_gate =
        match launch::prepare(&socket, &job_id, &observer, &mut tracking, &signals) {
            Ok(launch::Coordination::Fenced(gate)) => {
                if gate.waited() {
                    if let traffic::Mode::Scheduled { waited, .. }
                    | traffic::Mode::Unreserved { waited, .. } = &mut tracking
                    {
                        *waited = true;
                    }
                }
                Some(gate)
            }
            Ok(launch::Coordination::Legacy) => None,
            Ok(launch::Coordination::Outage) => {
                let old = std::mem::replace(
                    &mut tracking,
                    traffic::Mode::Legacy(preparation::Tracking::Uncertain),
                );
                tracking = match old {
                    traffic::Mode::Scheduled {
                        witness,
                        route,
                        continuity,
                        ..
                    } => traffic::Mode::Uncertain {
                        witness,
                        route,
                        continuity,
                    },
                    traffic::Mode::Unreserved {
                        witness,
                        continuity,
                        ..
                    } => traffic::Mode::Uncertain {
                        witness,
                        route: traffic::Route::Governor,
                        continuity,
                    },
                    other => other,
                };
                None
            }
            Err(error) => {
                tracking.finish_unspawned(&socket, &job_id, &observer);
                return preparation::stopped(error);
            }
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
                // The lineage write happens after authorization. Retract the
                // known-unspawned release before cleanup queues writer work.
                if let Some(gate) = &mut launch_gate {
                    let _ = gate.acknowledge(None);
                }
                drop(launch_gate.take());
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
        traffic::Mode::Scheduled { witness, .. }
        | traffic::Mode::Unreserved { witness, .. }
        | traffic::Mode::Uncertain { witness, .. } => {
            witness.install(&mut command);
        }
        _ => (),
    }
    if let Some(gate) = &launch_gate {
        gate.install(&mut command);
    }
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
            tracking.finish_unspawned(&socket, &job_id, &observer);
            return preparation::stopped(error);
        }
    };
    if let Ok(child) = &spawned {
        launch::after_spawn(&job_id, child.id(), &signals);
    }
    let spawn_failure = spawned.as_ref().err().map(launch::spawn_failure);
    let cancelled_spawn = spawned
        .as_ref()
        .err()
        .is_some_and(|error| error.kind() == io::ErrorKind::Interrupted && signals.interrupted());
    let unreserved_failure = match (&tracking, &spawn_failure) {
        (traffic::Mode::Unreserved { witness, .. }, Some(outcome)) if !cancelled_spawn => {
            // Persist terminal evidence while the release is still claimed.
            // UNSPAWNED would clear its authority and requeue the failed job.
            witness.finish_spawn_failed(outcome, || false);
            true
        }
        _ => false,
    };
    if let Some(gate) = launch_gate.as_mut().filter(|_| !unreserved_failure) {
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
            tracking.finish_unspawned(&socket, &job_id, &observer);
            return preparation::stopped(transport::Failure::Cancelled);
        }
        Err(_) => {
            let outcome = spawn_failure.expect("failed native spawn has its terminal outcome");
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
                traffic::Mode::Unreserved {
                    witness,
                    continuity,
                    release_id,
                    ..
                } => {
                    // Failure evidence was synced before dropping the fence.
                    // Only committed terminal completion can discard it.
                    witness.discard_if(release_id.as_ref().is_some_and(|release| {
                        governor::complete(
                            &socket,
                            &job_id,
                            &observer,
                            release,
                            outcome.clone(),
                            continuity,
                        )
                    }));
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
        traffic::Mode::Scheduled { waited, .. } | traffic::Mode::Unreserved { waited, .. } => {
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
        traffic::Mode::Unreserved {
            continuity,
            release_id,
            ..
        } => child_identity.clone().is_some_and(|identity| {
            release_id.as_ref().is_some_and(|release| {
                governor::attach(&socket, &job_id, &observer, release, identity, continuity)
            })
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
            traffic::Mode::Legacy(preparation::Tracking::Managed)
                | traffic::Mode::Scheduled { .. }
                | traffic::Mode::Unreserved { .. }
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
        traffic::Mode::Unreserved {
            witness,
            continuity,
            release_id,
            ..
        } => {
            let completed = release_id.as_ref().is_some_and(|release| {
                governor::complete(&socket, &job_id, &observer, release, outcome, continuity)
            });
            witness.discard_if(completed);
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
        diagnostics.emit(format_args!(
            "{}",
            preparation::completion_notice(&job_id, &status)
        ));
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

use super::{
    accounting, descriptors, governor, invalid, launch, lifecycle::complete, lineage,
    native_environment, preparation, signals, start_until, terminal, traffic, transport,
};
use arc_core::process_identity::random_job_id;
use arc_ipc::jobs::{JobId, ProcessIdentity, StartRequest};
use std::{
    ffi::OsString,
    io,
    os::unix::process::ExitStatusExt,
    process::{Command, ExitCode},
    time::{Duration, Instant},
};

/// The native fixture supplies synthetic facts while exercising this exact run path.
pub(super) fn run_with_classifier(
    argv: Vec<OsString>,
    preserve_environment: bool,
    now: &dyn Fn() -> Instant,
    delivery_now: &dyn Fn() -> Instant,
    classify: &dyn Fn(&[OsString]) -> arc_detector::Classification,
) -> ExitCode {
    run_with_classifier_and_preparation_clock(
        argv,
        preserve_environment,
        now,
        delivery_now,
        classify,
        &Instant::now,
    )
}

pub(super) fn run_with_classifier_and_preparation_clock(
    argv: Vec<OsString>,
    preserve_environment: bool,
    now: &dyn Fn() -> Instant,
    delivery_now: &dyn Fn() -> Instant,
    classify: &dyn Fn(&[OsString]) -> arc_detector::Classification,
    preparation_now: &dyn Fn() -> Instant,
) -> ExitCode {
    let _relay_scope = transport::relay_scope();
    if !preparation::valid_argv(&argv) {
        return invalid();
    }
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
    let classification = classify(&argv);
    // Endpoint capture can use its own bounded I/O budget. Registration and
    // claim share a separate budget beginning only after capture completes.
    let deadline = preparation::deadline(preparation_now());
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
    if preserve_environment {
        native_environment::restore_shell_underscore(&mut command, &argv[0]);
    }
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
    // Older clients need the environment claim for legacy families. Native
    // tools and endpoint admission inherit the caller's environment exactly.
    if let traffic::Mode::Scheduled { context, route, .. } = &tracking {
        native_environment::configure(
            &mut command,
            classification,
            context,
            preserve_environment || *route == traffic::Route::Endpoint,
        );
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
    let invocation = if preserve_environment {
        native_environment::preserve_path_invocation(&mut command, &argv)
    } else {
        Ok(())
    };
    let (spawned_at, spawned) = match preparation::admit(
        || !signals.admit_spawn(),
        || accounting::timed_spawn(now, || invocation.and_then(|_| command.spawn())),
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
        Err(error) => {
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
            return match (preserve_environment, error.kind()) {
                (true, io::ErrorKind::NotFound) => ExitCode::from(127),
                (true, io::ErrorKind::PermissionDenied) => ExitCode::from(126),
                _ => ExitCode::FAILURE,
            };
        }
    };
    signals.attach(child.id());
    let mut diagnostics = preparation::started_for_mode(&tracking, &job_id);
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
    } else if matches!(
        tracking,
        traffic::Mode::Legacy(preparation::Tracking::Managed)
    ) {
        // Only the directly managed lifecycle route registers a capability.
        signals.offer_reclaim(&socket, &job_id, &observer, child_identity.as_ref());
    }
    let termination = loop {
        match signals.wait_with_clocks(&mut child, || {}, now, delivery_now) {
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
    // The injected clock measures the native run interval only. IPC uses the
    // system monotonic clock, so its delivery budget must use that clock too.
    let report_deadline = termination.delivery_observed_at + accounting::REPORT_WINDOW;
    // Registration-outage fallback has no authoritative child/terminal binding.
    let meter_report = if attached && tracking.authoritative() {
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
    let outcome = preparation::outcome_from_status(&status);
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
        let _ = accounting::deliver(&socket, &report, report_deadline, delivery_now);
    }
    diagnostics.finish();
    drop(signals);
    drop(lineage_record);
    drop(accounting_record);
    // Signal-compatible exit uses _exit after re-raising, so release the
    // owned callback socket before destructors become unavailable.
    drop(_relay_scope);
    if let Some(signal) = status.signal() {
        signals::reraise(signal);
    }
    ExitCode::from(status.code().unwrap_or(1) as u8)
}

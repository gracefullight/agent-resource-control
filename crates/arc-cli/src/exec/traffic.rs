//! The CLI owns one spawn; Traffic only coordinates its admission and metadata.
use super::{
    preparation::{self, write_notice, Tracking},
    signals::Signals,
    transport::{self, Continuity, Failure},
    witness::Witness,
};
use arc_ipc::{
    jobs::{JobId, Outcome, ProcessIdentity},
    traffic::{
        Capabilities, JobRequest, JobResult, LineageContext, BUN_CAPABILITY, CAPABILITY,
        OPERATION_BUN_CAPABILITIES, OPERATION_BUN_JOB, OPERATION_TRAFFIC_CAPABILITIES,
        OPERATION_TRAFFIC_JOB,
    },
};
use serde_json::json;
use std::{
    path::Path,
    time::{Duration, Instant},
};

pub(super) const CONTEXT_ENV: &str = "ARC_TRAFFIC_LINEAGE";

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Route {
    Cargo,
    Bun,
}
impl Route {
    fn operation(self) -> &'static str {
        match self {
            Self::Cargo => OPERATION_TRAFFIC_JOB,
            Self::Bun => OPERATION_BUN_JOB,
        }
    }
}

pub(super) enum Mode {
    Legacy(Tracking),
    Scheduled {
        generation: u64,
        context: LineageContext,
        waited: bool,
        witness: Witness,
        route: Route,
        continuity: Continuity,
    },
    Uncertain {
        witness: Witness,
        route: Route,
        continuity: Continuity,
    },
}

fn job(
    socket: &Path,
    request: &JobRequest,
    route: Route,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
    possibly_committed: &mut bool,
    continuity: &Continuity,
) -> Result<JobResult, Failure> {
    let value = transport::call_until_with_continuity(
        socket,
        route.operation(),
        json!(request),
        deadline,
        cancelled,
        possibly_committed,
        Some(continuity),
    )?;
    let result: JobResult = serde_json::from_value(value).map_err(|_| {
        *possibly_committed = true;
        Failure::InvalidReply
    })?;
    if !result.is_valid() {
        *possibly_committed = true;
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
            Reason::CargoBuild
                | Reason::CargoTest
                | Reason::CargoClippy
                | Reason::CargoFmt
                | Reason::BunTest
        )
    {
        return preparation::prepare(socket, id, observer, classification, deadline, signals)
            .map(Mode::Legacy);
    }
    let route = if classification.reason == Reason::BunTest {
        Route::Bun
    } else {
        Route::Cargo
    };
    let legacy_classification = if route == Route::Bun {
        arc_ipc::Classification {
            classification: arc_ipc::JobClass::Unknown,
            locality: Locality::Unknown,
            reason: Reason::UnresolvedCommand,
            uncertainty: arc_ipc::Uncertainty::Observe,
        }
    } else {
        classification
    };
    let cancelled = || signals.interrupted();
    let probe = transport::call_until(
        socket,
        if route == Route::Bun {
            OPERATION_BUN_CAPABILITIES
        } else {
            OPERATION_TRAFFIC_CAPABILITIES
        },
        serde_json::Value::Null,
        deadline,
        &cancelled,
    );
    match probe {
        Err(Failure::Unsupported) => {
            return preparation::prepare(
                socket,
                id,
                observer,
                legacy_classification,
                deadline,
                signals,
            )
            .map(Mode::Legacy)
        }
        Err(Failure::Unavailable) => return Ok(Mode::Legacy(Tracking::Unavailable)),
        Err(Failure::Uncertain | Failure::Busy) => return Ok(Mode::Legacy(Tracking::Uncertain)),
        Err(error) => return Err(error),
        Ok(value) => {
            let capabilities: Capabilities =
                serde_json::from_value(value).map_err(|_| Failure::InvalidReply)?;
            if capabilities.capability
                != if route == Route::Bun {
                    BUN_CAPABILITY
                } else {
                    CAPABILITY
                }
            {
                return Err(Failure::InvalidReply);
            }
        }
    }
    let inherited = super::lineage::inherited(socket).or_else(|| {
        std::env::var(CONTEXT_ENV)
            .ok()
            .and_then(|v| LineageContext::try_from(v).ok())
    });
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
    let mut possibly_committed = false;
    let continuity = Continuity::default();
    let mut result = match job(
        socket,
        &enqueue,
        route,
        deadline,
        &cancelled,
        &mut possibly_committed,
        &continuity,
    ) {
        Ok(result) => result,
        Err(Failure::Unsupported) if route == Route::Bun && !possibly_committed => {
            witness.discard();
            return preparation::prepare(
                socket,
                id,
                observer,
                legacy_classification,
                Instant::now() + Duration::from_millis(1800),
                signals,
            )
            .map(Mode::Legacy);
        }
        Err(
            Failure::Unavailable | Failure::Uncertain | Failure::Busy | Failure::InvalidEndpoint,
        ) => {
            return Ok(Mode::Uncertain {
                witness,
                route,
                continuity,
            })
        }
        Err(Failure::Cancelled) => {
            witness.finish_unspawned(|| cancel(socket, id, observer, None, route, &continuity));
            return Err(Failure::Cancelled);
        }
        Err(error) => {
            witness.finish_unspawned(|| cancel(socket, id, observer, None, route, &continuity));
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
                    route,
                    continuity,
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
                    route,
                    continuity,
                });
            }
            JobResult::Bypassed { .. } => {
                if possibly_committed {
                    return Ok(Mode::Uncertain {
                        witness,
                        route,
                        continuity,
                    });
                }
                witness.discard();
                return preparation::prepare(
                    socket,
                    id,
                    observer,
                    legacy_classification,
                    Instant::now() + Duration::from_millis(1800),
                    signals,
                )
                .map(Mode::Legacy);
            }
            JobResult::Queued { .. } => {
                possibly_committed = true;
                if !waited {
                    queued_notice(id);
                    waited = true;
                }
                // The request and SQLite ticket have both been released here.
                std::thread::sleep(Duration::from_millis(40));
                if cancelled() {
                    witness.finish_unspawned(|| {
                        cancel(socket, id, observer, None, route, &continuity)
                    });
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
                    route,
                    Instant::now() + Duration::from_millis(1800),
                    &cancelled,
                    &mut possibly_committed,
                    &continuity,
                ) {
                    Ok(value) => value,
                    Err(
                        Failure::Unavailable
                        | Failure::Uncertain
                        | Failure::Busy
                        | Failure::InvalidEndpoint,
                    ) => {
                        return Ok(Mode::Uncertain {
                            witness,
                            route,
                            continuity,
                        })
                    }
                    Err(Failure::Cancelled) => {
                        witness.finish_unspawned(|| {
                            cancel(socket, id, observer, None, route, &continuity)
                        });
                        return Err(Failure::Cancelled);
                    }
                    Err(error) => {
                        witness.finish_unspawned(|| {
                            cancel(socket, id, observer, None, route, &continuity)
                        });
                        return Err(error);
                    }
                };
            }
            JobResult::NotFound {} if possibly_committed => {
                return Ok(Mode::Uncertain {
                    witness,
                    route,
                    continuity,
                });
            }
            _ => {
                witness.finish_unspawned(|| cancel(socket, id, observer, None, route, &continuity));
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

pub(super) fn cancel(
    socket: &Path,
    id: &JobId,
    observer: &ProcessIdentity,
    generation: Option<u64>,
    route: Route,
    continuity: &Continuity,
) -> bool {
    let request = JobRequest::Cancel {
        job_id: id.clone(),
        observer: observer.clone(),
        permit_generation: generation,
    };
    let result = job(
        socket,
        &request,
        route,
        Instant::now() + Duration::from_millis(500),
        &|| false,
        &mut false,
        continuity,
    );
    let confirmed = matches!(result, Ok(JobResult::Cancelled {})) && continuity.confirmed();
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
    route: Route,
    continuity: &Continuity,
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
            route,
            Instant::now() + Duration::from_secs(2),
            &|| false,
            &mut false,
            continuity,
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
    route: Route,
    continuity: &Continuity,
) -> bool {
    let request = JobRequest::Complete {
        job_id: id.clone(),
        observer: observer.clone(),
        outcome: outcome.clone(),
        permit_generation: generation,
    };
    matches!(job(socket, &request, route, Instant::now() + Duration::from_secs(2), &|| false, &mut false, continuity), Ok(JobResult::Completed { outcome: actual, .. }) if actual == outcome)
}

pub(super) fn complete_or_warn(
    socket: &Path,
    id: &JobId,
    observer: &ProcessIdentity,
    generation: u64,
    outcome: Outcome,
    route: Route,
    continuity: &Continuity,
) -> bool {
    let completed = complete(socket, id, observer, generation, outcome, route, continuity)
        && continuity.confirmed();
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
    route: Route,
    continuity: &Continuity,
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
            route,
            Instant::now() + Duration::from_secs(2),
            &|| false,
            &mut false,
            continuity,
        ),
        Ok(JobResult::Completed { .. } | JobResult::Abandoned {} | JobResult::Cancelled {})
    )
}

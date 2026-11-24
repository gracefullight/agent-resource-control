//! The CLI owns one spawn; Traffic only coordinates its admission and metadata.
use super::{
    preparation::{self, Tracking},
    signals::Signals,
    transport::{self, Continuity, Failure},
    witness::Witness,
};
use arc_ipc::{
    jobs::{JobId, ProcessIdentity},
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

mod cadence;
pub(super) use cadence::Cadence;
mod lifecycle;
pub(super) use lifecycle::{attach, cancel, complete, complete_or_warn, fallback};
mod notice;
pub(super) use notice::{announce_if_needed, queued_notice, NoticeState};

pub(super) const CONTEXT_ENV: &str = "ARC_TRAFFIC_LINEAGE";

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Route {
    Cargo,
    Bun,
    Governor,
    BunScripts,
    PythonTests,
    NativeTools,
    Endpoint,
}
impl Route {
    pub(super) fn governed(self) -> bool {
        matches!(
            self,
            Self::Governor
                | Self::BunScripts
                | Self::PythonTests
                | Self::NativeTools
                | Self::Endpoint
        )
    }
    pub(super) fn operation(self) -> &'static str {
        match self {
            Self::Cargo => OPERATION_TRAFFIC_JOB,
            Self::Bun => OPERATION_BUN_JOB,
            Self::Governor => arc_ipc::governor::OPERATION_JOB,
            Self::BunScripts => arc_ipc::traffic::OPERATION_BUN_SCRIPTS_JOB,
            Self::PythonTests => arc_ipc::traffic::OPERATION_PYTHON_TESTS_JOB,
            Self::NativeTools => arc_ipc::traffic::native::OPERATION_JOB,
            Self::Endpoint => arc_ipc::traffic::endpoint::OPERATION_JOB,
        }
    }
}

pub(super) enum Mode {
    Legacy(Tracking),
    Unreserved {
        waited: bool,
        witness: Witness,
        continuity: Continuity,
        release_id: Option<String>,
    },
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
    /// A redelivered hook event (`event_key`): one child, nothing recorded.
    Duplicate,
}

impl Mode {
    /// The daemon holds an authoritative job and child binding for this run.
    pub(super) fn authoritative(&self) -> bool {
        matches!(
            self,
            Self::Legacy(Tracking::Managed) | Self::Scheduled { .. } | Self::Unreserved { .. }
        )
    }
    pub(super) fn finish_unspawned(&self, socket: &Path, id: &JobId, observer: &ProcessIdentity) {
        match self {
            Self::Scheduled {
                witness,
                generation,
                route,
                continuity,
                ..
            } => witness.finish_unspawned(|| {
                cancel(socket, id, observer, Some(*generation), *route, continuity)
            }),
            Self::Unreserved {
                witness,
                continuity,
                ..
            } => witness.finish_unspawned(|| {
                cancel(socket, id, observer, None, Route::Governor, continuity)
            }),
            Self::Uncertain {
                witness,
                route,
                continuity,
            } => witness.finish_unspawned(|| {
                cancel(socket, id, observer, None, *route, continuity) && *route != Route::Bun
            }),
            _ => (),
        }
    }
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
    if route.governed() {
        return match super::governor::call_routed(
            socket,
            &arc_ipc::governor::GovernorJobRequest::Traffic {
                request: request.clone(),
            },
            deadline,
            cancelled,
            continuity,
            route,
            possibly_committed,
        )? {
            arc_ipc::governor::GovernorJobResult::Traffic { result } => Ok(result),
            _ => Err(Failure::Uncertain),
        };
    }
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

// A responsive DB_BUSY coordinator is not a daemon outage. Retain admission
// and cancellation across bounded exchanges without allowing a native spawn.
fn admission(
    socket: &Path,
    request: &JobRequest,
    route: Route,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
    possibly_committed: &mut bool,
    continuity: &Continuity,
) -> Result<JobResult, Failure> {
    admission_with(deadline, cancelled, |deadline| {
        job(
            socket,
            request,
            route,
            deadline,
            cancelled,
            possibly_committed,
            continuity,
        )
    })
}
fn admission_with(
    mut deadline: Instant,
    cancelled: &dyn Fn() -> bool,
    mut exchange: impl FnMut(Instant) -> Result<JobResult, Failure>,
) -> Result<JobResult, Failure> {
    loop {
        if cancelled() {
            return Err(Failure::Cancelled);
        }
        match exchange(deadline) {
            Err(Failure::Busy) => {
                deadline = Instant::now() + Duration::from_millis(1800);
            }
            result => return result,
        }
    }
}

#[cfg(test)]
mod admission_tests;

pub(super) fn prepare(
    socket: &Path,
    id: &JobId,
    observer: &ProcessIdentity,
    classification: arc_ipc::Classification,
    deadline: Instant,
    signals: &Signals,
) -> Result<Mode, Failure> {
    use arc_detector::{Locality, Reason};
    if !classification.locality.is_local_compute()
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
        // This route serves peers that declined the current admission route;
        // they are sent protocol-1 bytes only and keep sequence order.
        project: None,
        event: None,
        usage: None,
    };
    let mut possibly_committed = false;
    let continuity = Continuity::default();
    let mut result = match admission(
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
                deadline,
                signals,
            )
            .map(Mode::Legacy);
        }
        Err(Failure::Unavailable | Failure::Uncertain | Failure::InvalidEndpoint) => {
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
    let mut pressure_notice = NoticeState::default();
    let mut cadence = Cadence::new(socket);
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
                    deadline,
                    signals,
                )
                .map(Mode::Legacy);
            }
            JobResult::Queued { reason, .. } => {
                possibly_committed = true;
                let pressure = super::policy::job(
                    socket,
                    id,
                    Instant::now() + Duration::from_millis(150),
                    &cancelled,
                )
                .is_ok_and(|result| super::policy::confirmed_hold(&result));
                announce_if_needed(&mut pressure_notice, pressure, |pressure| {
                    queued_notice(id, pressure)
                });
                waited = true;
                // The request and SQLite ticket have both been released here.
                cadence.pause(reason, &|slice| std::thread::sleep(slice), &cancelled);
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
                result = match admission(
                    socket,
                    &poll,
                    route,
                    Instant::now() + Duration::from_millis(1800),
                    &cancelled,
                    &mut possibly_committed,
                    &continuity,
                ) {
                    Ok(value) => value,
                    Err(Failure::Unavailable | Failure::Uncertain | Failure::InvalidEndpoint) => {
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

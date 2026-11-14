//! Negotiated current-client admission. Eligibility never authorizes spawn.
mod lifecycle;
use super::{
    preparation::write_notice,
    signals::Signals,
    traffic::{self, Mode, Route},
    transport::{Continuity, Failure},
    witness::Witness,
};
use arc_ipc::{
    client::Session,
    governor::*,
    jobs::{JobId, ProcessIdentity},
    traffic::{JobRequest, JobResult, LineageContext},
    Request,
};
pub(super) use lifecycle::{attach, complete};
use serde_json::json;
use std::{
    path::Path,
    time::{Duration, Instant},
};

pub(super) fn prepare(
    socket: &Path,
    id: &JobId,
    observer: &ProcessIdentity,
    classification: arc_ipc::Classification,
    deadline: Instant,
    signals: &Signals,
) -> Result<Mode, Failure> {
    use arc_detector::{Locality, Reason};
    let route = if classification.is_bun_script() {
        Route::BunScripts
    } else {
        Route::Governor
    };
    let fallback_classification = if classification.is_bun_script() {
        classification.legacy()
    } else {
        classification
    };
    if classification.locality != Locality::HostLocal
        || !matches!(
            classification.reason,
            Reason::CargoBuild
                | Reason::CargoTest
                | Reason::CargoClippy
                | Reason::CargoFmt
                | Reason::BunTest
                | Reason::BunScriptBuild
                | Reason::BunScriptTest
                | Reason::BunScriptLint
                | Reason::BunScriptTypecheck
        )
    {
        return traffic::prepare(socket, id, observer, classification, deadline, signals);
    }
    let cancelled = || signals.interrupted();
    let probe_operation = if route == Route::BunScripts {
        arc_ipc::traffic::OPERATION_BUN_SCRIPTS_CAPABILITIES
    } else {
        OPERATION_CAPABILITIES
    };
    let probe = if route == Route::BunScripts {
        super::transport::negotiated(
            socket,
            probe_operation,
            serde_json::Value::Null,
            deadline,
            &cancelled,
            &mut false,
            None,
        )
    } else {
        Session::connect_until(socket, deadline, &cancelled, None).and_then(|mut session| {
            session.exchange(
                &Request::new(crate::request_id(), probe_operation),
                &cancelled,
            )
        })
    };
    match probe {
        Err(Failure::Unsupported) => {
            return traffic::prepare(
                socket,
                id,
                observer,
                fallback_classification,
                deadline,
                signals,
            )
        }
        Err(Failure::Unavailable | Failure::Uncertain | Failure::Busy) => {
            return traffic::prepare(
                socket,
                id,
                observer,
                fallback_classification,
                deadline,
                signals,
            )
        }
        Err(error) => return Err(error),
        Ok(value) => {
            if route == Route::BunScripts {
                let caps: arc_ipc::traffic::Capabilities =
                    serde_json::from_value(value).map_err(|_| Failure::InvalidReply)?;
                if caps.capability != arc_ipc::traffic::BUN_SCRIPTS_CAPABILITY {
                    return Err(Failure::InvalidReply);
                }
            } else {
                let caps: GovernorCapabilities =
                    serde_json::from_value(value.clone()).map_err(|_| Failure::InvalidReply)?;
                if !value.is_object() || !caps.is_valid() {
                    return Err(Failure::InvalidReply);
                }
            }
        }
    }
    let inherited = super::lineage::inherited(socket).or_else(|| {
        std::env::var(traffic::CONTEXT_ENV)
            .ok()
            .and_then(|v| LineageContext::try_from(v).ok())
    });
    let claim = inherited.clone().map(Ok).unwrap_or_else(|| {
        let first = arc_core::process_identity::random_job_id().map_err(|_| Failure::Local)?;
        let second = arc_core::process_identity::random_job_id().map_err(|_| Failure::Local)?;
        LineageContext::try_from(format!("{first}{second}")).map_err(|_| Failure::Local)
    })?;
    let witness = Witness::create(socket, id, &claim, observer).map_err(|_| Failure::Witness)?;
    witness.arm();
    let continuity = Continuity::default();
    let enqueue = JobRequest::Enqueue {
        job_id: id.clone(),
        observer: observer.clone(),
        classification,
        lineage_context: Some(claim.clone()),
    };
    super::launch::before_enqueue(&json!({"request":enqueue}), &cancelled)?;
    let result = await_admission(
        AdmissionPeer {
            socket,
            id,
            observer,
            continuity: &continuity,
            route,
        },
        &enqueue,
        inherited.is_some(),
        Some(&claim),
        deadline,
        &cancelled,
    );
    match result {
        Ok(Admission::Reserved {
            generation,
            context,
            waited,
        }) => Ok(Mode::Scheduled {
            generation,
            context,
            waited,
            witness,
            route,
            continuity,
        }),
        Ok(Admission::Unreserved { waited }) => Ok(Mode::Unreserved {
            waited,
            witness,
            continuity,
            release_id: None,
        }),
        Err(Failure::Unavailable | Failure::Uncertain | Failure::InvalidEndpoint) => {
            Ok(Mode::Uncertain {
                witness,
                route,
                continuity,
            })
        }
        Err(Failure::Unsupported) if route == Route::BunScripts => {
            // await_admission only returns Unsupported when no prior attempt
            // could have committed, even if the peer changed after the probe.
            witness.discard();
            preparation_fallback(socket, id, observer, classification, deadline, signals)
        }
        Ok(Admission::Unconfirmed) => {
            witness.finish_unspawned(|| {
                traffic::cancel(socket, id, observer, None, route, &continuity)
            });
            Err(Failure::Uncertain)
        }
        Err(error) => {
            // A missing release acknowledgement must never become outage spawn.
            witness.finish_unspawned(|| {
                traffic::cancel(socket, id, observer, None, route, &continuity)
            });
            Err(error)
        }
    }
}

fn preparation_fallback(
    socket: &Path,
    id: &JobId,
    observer: &ProcessIdentity,
    classification: arc_ipc::Classification,
    deadline: Instant,
    signals: &Signals,
) -> Result<Mode, Failure> {
    super::preparation::prepare(
        socket,
        id,
        observer,
        classification.legacy(),
        deadline,
        signals,
    )
    .map(Mode::Legacy)
}

enum Admission {
    Unconfirmed,
    Reserved {
        generation: u64,
        context: LineageContext,
        waited: bool,
    },
    Unreserved {
        waited: bool,
    },
}
struct AdmissionPeer<'a> {
    socket: &'a Path,
    id: &'a JobId,
    observer: &'a ProcessIdentity,
    continuity: &'a Continuity,
    route: Route,
}
fn await_admission(
    peer: AdmissionPeer<'_>,
    initial: &JobRequest,
    inherited: bool,
    claim: Option<&LineageContext>,
    mut deadline: Instant,
    cancelled: &dyn Fn() -> bool,
) -> Result<Admission, Failure> {
    let AdmissionPeer {
        socket,
        id,
        observer,
        continuity,
        route,
    } = peer;
    let mut request = initial.clone();
    let mut waited = false;
    let mut possibly_committed = false;
    loop {
        if cancelled() {
            return Err(Failure::Cancelled);
        }
        let result = call_routed(
            socket,
            &GovernorJobRequest::Traffic {
                request: request.clone(),
            },
            deadline,
            cancelled,
            continuity,
            route,
            &mut possibly_committed,
        );
        match result {
            Ok(GovernorJobResult::Unreserved { job_id, .. }) if &job_id == id => {
                return Ok(Admission::Unreserved { waited })
            }
            Ok(GovernorJobResult::Unconfirmed { .. }) => return Ok(Admission::Unconfirmed),
            Ok(GovernorJobResult::Traffic { result }) => match result {
                JobResult::Granted {
                    permit_generation,
                    lineage_context,
                    ..
                } => {
                    return Ok(Admission::Reserved {
                        generation: permit_generation,
                        context: lineage_context,
                        waited,
                    })
                }
                JobResult::Reused {
                    permit_generation, ..
                } => {
                    return Ok(Admission::Reserved {
                        generation: permit_generation,
                        context: claim.cloned().ok_or(Failure::InvalidReply)?,
                        waited,
                    })
                }
                JobResult::Queued { .. } => {
                    if !waited {
                        write_notice("QUEUED\nWaiting to start.\n");
                        waited = true;
                        super::launch::after_queue(&json!({"request":request}), cancelled)?;
                    }
                    std::thread::sleep(Duration::from_millis(40));
                    request = if inherited {
                        initial.clone()
                    } else {
                        JobRequest::Poll {
                            job_id: id.clone(),
                            observer: observer.clone(),
                        }
                    };
                }
                JobResult::Cancelled {} => return Err(Failure::Cancelled),
                JobResult::NotFound {} if waited || matches!(initial, JobRequest::Poll { .. }) => {
                    return Err(Failure::Uncertain)
                }
                _ => return Err(Failure::InvalidReply),
            },
            Err(Failure::Busy) => {
                if route == Route::BunScripts {
                    std::thread::sleep(Duration::from_millis(20));
                }
            }
            Err(Failure::Unsupported) if possibly_committed => return Err(Failure::Uncertain),
            Err(error) => return Err(error),
            Ok(_) => return Err(Failure::InvalidReply),
        }
        deadline = Instant::now() + Duration::from_millis(1800);
    }
}

pub(super) fn repoll(
    socket: &Path,
    id: &JobId,
    observer: &ProcessIdentity,
    mode: &mut Mode,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Failure> {
    let route = match mode {
        Mode::Scheduled { route, .. } => *route,
        _ => Route::Governor,
    };
    let continuity = match mode {
        Mode::Scheduled { continuity, .. } | Mode::Unreserved { continuity, .. } => continuity,
        _ => return Err(Failure::InvalidReply),
    };
    // Poll returns the authoritative Granted context; Reused is only legal for
    // an inherited reservation, so no root poll may synthesize that context.
    let admission = await_admission(
        AdmissionPeer {
            socket,
            id,
            observer,
            continuity,
            route,
        },
        &JobRequest::Poll {
            job_id: id.clone(),
            observer: observer.clone(),
        },
        false,
        None,
        Instant::now() + Duration::from_millis(1800),
        cancelled,
    )?;
    let old = std::mem::replace(mode, Mode::Legacy(super::preparation::Tracking::Uncertain));
    let (witness, continuity, prior_waited) = match old {
        Mode::Scheduled {
            witness,
            continuity,
            waited,
            ..
        }
        | Mode::Unreserved {
            witness,
            continuity,
            waited,
            ..
        } => (witness, continuity, waited),
        _ => unreachable!(),
    };
    *mode = match admission {
        Admission::Unconfirmed => {
            *mode = Mode::Unreserved {
                waited: prior_waited,
                witness,
                continuity,
                release_id: None,
            };
            return Err(Failure::Uncertain);
        }
        Admission::Reserved {
            generation,
            context,
            waited,
        } => Mode::Scheduled {
            generation,
            context,
            waited: waited || prior_waited,
            witness,
            route,
            continuity,
        },
        Admission::Unreserved { waited } => Mode::Unreserved {
            waited: waited || prior_waited,
            witness,
            continuity,
            release_id: None,
        },
    };
    Ok(())
}

pub(super) fn call(
    socket: &Path,
    request: &GovernorJobRequest,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
    continuity: &Continuity,
) -> Result<GovernorJobResult, Failure> {
    call_routed(
        socket,
        request,
        deadline,
        cancelled,
        continuity,
        Route::Governor,
        &mut false,
    )
}

pub(super) fn call_routed(
    socket: &Path,
    request: &GovernorJobRequest,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
    continuity: &Continuity,
    route: Route,
    possibly_committed: &mut bool,
) -> Result<GovernorJobResult, Failure> {
    if route == Route::BunScripts {
        // Session authenticates the endpoint and validates request ID/protocol.
        // Unlike release requests, an admission may be reconciled using the
        // same job/witness after a lost reply; an old peer cannot erase it.
        let value = super::transport::negotiated(
            socket,
            arc_ipc::traffic::OPERATION_BUN_SCRIPTS_JOB,
            json!(request),
            deadline,
            cancelled,
            possibly_committed,
            Some(continuity),
        )?;
        return decode_job_result(&value).map_err(|_| Failure::InvalidReply);
    }
    let value = super::transport::call_until_with_continuity(
        socket,
        OPERATION_JOB,
        json!(request),
        deadline,
        cancelled,
        possibly_committed,
        Some(continuity),
    )?;
    decode_job_result(&value).map_err(|_| Failure::InvalidReply)
}

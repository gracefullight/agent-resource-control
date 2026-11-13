//! Negotiated current-client admission. Eligibility never authorizes spawn.
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
        return traffic::prepare(socket, id, observer, classification, deadline, signals);
    }
    let cancelled = || signals.interrupted();
    let probe =
        Session::connect_until(socket, deadline, &cancelled, None).and_then(|mut session| {
            session.exchange(
                &Request::new(crate::request_id(), OPERATION_CAPABILITIES),
                &cancelled,
            )
        });
    match probe {
        Err(Failure::Unsupported) => {
            return traffic::prepare(socket, id, observer, classification, deadline, signals)
        }
        Err(Failure::Unavailable | Failure::Uncertain | Failure::Busy) => {
            return traffic::prepare(socket, id, observer, classification, deadline, signals)
        }
        Err(error) => return Err(error),
        Ok(value) => {
            let caps: GovernorCapabilities =
                serde_json::from_value(value.clone()).map_err(|_| Failure::InvalidReply)?;
            if !value.is_object() || !caps.is_valid() {
                return Err(Failure::InvalidReply);
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
    let result = await_admission(
        AdmissionPeer {
            socket,
            id,
            observer,
            continuity: &continuity,
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
            route: Route::Governor,
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
                route: Route::Governor,
                continuity,
            })
        }
        Ok(Admission::Unconfirmed) => {
            witness.finish_unspawned(|| {
                traffic::cancel(socket, id, observer, None, Route::Governor, &continuity)
            });
            Err(Failure::Uncertain)
        }
        Err(error) => {
            // A missing release acknowledgement must never become outage spawn.
            witness.finish_unspawned(|| {
                traffic::cancel(socket, id, observer, None, Route::Governor, &continuity)
            });
            Err(error)
        }
    }
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
    } = peer;
    let mut request = initial.clone();
    let mut waited = false;
    loop {
        if cancelled() {
            return Err(Failure::Cancelled);
        }
        let result = call(
            socket,
            &GovernorJobRequest::Traffic {
                request: request.clone(),
            },
            deadline,
            cancelled,
            continuity,
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
            Err(Failure::Busy) => (),
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
            route: Route::Governor,
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
    let value = super::transport::call_until_with_continuity(
        socket,
        OPERATION_JOB,
        json!(request),
        deadline,
        cancelled,
        &mut false,
        Some(continuity),
    )?;
    decode_job_result(&value).map_err(|_| Failure::InvalidReply)
}
pub(super) fn attach(
    socket: &Path,
    id: &JobId,
    observer: &ProcessIdentity,
    release: &str,
    child: ProcessIdentity,
    continuity: &Continuity,
) -> bool {
    matches!(
        call(
            socket,
            &GovernorJobRequest::Attach {
                job_id: id.clone(),
                observer: observer.clone(),
                release_id: release.into(),
                child
            },
            Instant::now() + Duration::from_secs(2),
            &|| false,
            continuity
        ),
        Ok(GovernorJobResult::Traffic {
            result: JobResult::Running {}
        })
    ) && continuity.confirmed()
}
pub(super) fn complete(
    socket: &Path,
    id: &JobId,
    observer: &ProcessIdentity,
    release: &str,
    outcome: arc_ipc::jobs::Outcome,
    continuity: &Continuity,
) -> bool {
    matches!(call(socket, &GovernorJobRequest::Complete { job_id: id.clone(), observer: observer.clone(), release_id: release.into(), outcome: outcome.clone() }, Instant::now()+Duration::from_secs(2), &|| false, continuity), Ok(GovernorJobResult::Traffic { result: JobResult::Completed { outcome: actual, .. } }) if actual == outcome)
        && continuity.confirmed()
}

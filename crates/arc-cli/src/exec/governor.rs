//! Negotiated current-client admission. Eligibility never authorizes spawn.
mod admission;
mod lifecycle;
use super::{
    signals::Signals,
    traffic::{self, Mode, Route},
    transport::{Continuity, Failure},
    witness::Witness,
};
#[cfg(test)]
pub(in crate::exec) use admission::test_clock;
use admission::{await_admission, Admission, AdmissionPeer};
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
    use arc_detector::Reason;
    // A compound's negotiated record is job-scoped; its conservative admission
    // and every existing request retain the protocol-1 projection.
    let classification =
        super::workspace_compound::publish(socket, id, classification, deadline, &|| {
            signals.interrupted()
        })
        .host_local_projection();
    let route = if classification.reason == Reason::LocalEndpointBuild {
        Route::Endpoint
    } else if classification.is_bun_script() {
        Route::BunScripts
    } else if classification.is_python_test() {
        Route::PythonTests
    } else if classification.is_native_tool() {
        Route::NativeTools
    } else {
        Route::Governor
    };
    let fallback_classification = if classification.is_bun_script()
        || classification.is_python_test()
        || classification.is_native_tool()
        || classification.is_endpoint_locality()
    {
        classification.legacy()
    } else {
        classification
    };
    if !classification.locality.is_local_compute()
        || !(classification.is_native_tool()
            || matches!(
                classification.reason,
                Reason::CargoBuild
                    | Reason::LocalEndpointBuild
                    | Reason::CargoTest
                    | Reason::CargoClippy
                    | Reason::CargoFmt
                    | Reason::BunTest
                    | Reason::BunScriptBuild
                    | Reason::BunScriptTest
                    | Reason::BunScriptLint
                    | Reason::BunScriptTypecheck
                    | Reason::PythonPytest
                    | Reason::PythonUnittest
            ))
    {
        return traffic::prepare(socket, id, observer, classification, deadline, signals);
    }
    let cancelled = || signals.interrupted();
    let negotiated = match route {
        Route::BunScripts => Some((
            arc_ipc::traffic::OPERATION_BUN_SCRIPTS_CAPABILITIES,
            arc_ipc::traffic::BUN_SCRIPTS_CAPABILITY,
        )),
        Route::PythonTests => Some((
            arc_ipc::traffic::OPERATION_PYTHON_TESTS_CAPABILITIES,
            arc_ipc::traffic::PYTHON_TESTS_CAPABILITY,
        )),
        Route::NativeTools => Some((
            arc_ipc::traffic::native::OPERATION_CAPABILITIES,
            arc_ipc::traffic::native::CAPABILITY,
        )),
        Route::Endpoint => Some((
            arc_ipc::traffic::endpoint::OPERATION_CAPABILITIES,
            arc_ipc::traffic::endpoint::CAPABILITY,
        )),
        _ => None,
    };
    // Records which daemon answered the probe; it gates the project key.
    let prober = Continuity::default();
    let probe = if let Some((probe_operation, _)) = negotiated {
        super::transport::call_until_with_continuity(
            socket,
            probe_operation,
            serde_json::Value::Null,
            deadline,
            &cancelled,
            &mut false,
            Some(&prober),
        )
    } else {
        Session::connect_until(socket, deadline, &cancelled, Some(&prober)).and_then(
            |mut session| {
                session.exchange(
                    &Request::new(crate::request_id(), OPERATION_CAPABILITIES),
                    &cancelled,
                )
            },
        )
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
            if let Some((_, capability)) = negotiated {
                if !value.is_object() {
                    return Err(Failure::InvalidReply);
                }
                let caps: arc_ipc::traffic::Capabilities =
                    serde_json::from_value(value).map_err(|_| Failure::InvalidReply)?;
                if caps.capability != capability {
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
        project: super::project_key::negotiated(socket, &prober),
        event: super::event_key::negotiated(socket, &prober),
        usage: super::usage_fields::negotiated(socket, &prober),
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
        Ok(Admission::Duplicate) => {
            // The daemon recorded nothing under this run's id: no job, queue
            // entry or permit exists that the witness could ever recover.
            witness.discard();
            Ok(Mode::Duplicate)
        }
        Err(Failure::Unavailable | Failure::Uncertain | Failure::InvalidEndpoint) => {
            Ok(Mode::Uncertain {
                witness,
                route,
                continuity,
            })
        }
        Err(Failure::Unsupported)
            if matches!(
                route,
                Route::BunScripts | Route::PythonTests | Route::NativeTools | Route::Endpoint
            ) =>
        {
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
        // A poll carries no event key, so it is never a duplicate delivery.
        Admission::Unconfirmed | Admission::Duplicate => {
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
    // Session authenticates the endpoint and validates request ID/protocol.
    // Reconcile admissions through the same job/witness after a lost reply.
    let value = super::transport::call_until_with_continuity(
        socket,
        route.operation(),
        json!(request),
        deadline,
        cancelled,
        possibly_committed,
        Some(continuity),
    )?;
    decode_job_result(&value).map_err(|_| Failure::InvalidReply)
}

//! CLI-only preparation decisions following shared-client failures.
use super::{call_until, Failure};
use crate::exec::preparation::{prepare_with, Tracking};
use arc_ipc::jobs::{
    IdentityObservation, JobId, JobSnapshot, LifecycleReason, ObservationStatus, ObservationTarget,
    Phase, ProcessIdentity, StartDisposition, StartRequest, StartResult,
};
use std::{
    ffi::OsString,
    os::unix::ffi::OsStringExt,
    path::PathBuf,
    time::{Duration, Instant},
};
fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(2)
}

fn registered(
    id: &JobId,
    observer: &ProcessIdentity,
    classification: arc_ipc::Classification,
) -> StartResult {
    let reason = LifecycleReason::RegisteredNotClaimed;
    StartResult {
        disposition: StartDisposition::Created,
        job: JobSnapshot {
            job_id: id.clone(),
            phase: Phase::Registered,
            classification,
            observer: observer.clone(),
            observer_verified_at_ms: 1,
            child: None,
            child_verified_at_ms: None,
            observation: IdentityObservation {
                target: ObservationTarget::Observer,
                status: ObservationStatus::VerifiedMatch,
                observed_at_ms: 1,
                reason,
            },
            reason,
            outcome: None,
            registered_at_ms: 1,
            launch_claimed_at_ms: None,
            completed_at_ms: None,
            updated_at_ms: 1,
            revision: 1,
        },
    }
}

#[test]
fn confirmed_registration_then_denied_claim_is_uncertain() {
    let id = JobId::try_from("0123456789abcdef0123456789abcdef".to_string()).unwrap();
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let classification = arc_detector::classify_argv(&["fixture".into()]);
    let end = deadline();
    let mut calls = 0;
    let result = prepare_with(
        &id,
        &observer,
        classification,
        end,
        &|| false,
        |request, same_end, cancelled| {
            assert_eq!(same_end, end);
            calls += 1;
            match request {
                StartRequest::Register { .. } if calls == 1 => {
                    Ok(registered(&id, &observer, classification))
                }
                StartRequest::Claim { .. } if calls == 2 => {
                    assert!(!cancelled());
                    Err(Failure::Unavailable)
                }
                _ => panic!("unexpected request order"),
            }
        },
    );
    assert_eq!(calls, 2);
    assert_eq!(result, Ok(Tracking::Uncertain));
}

#[test]
fn confirmed_register_then_invalid_endpoint_is_uncertain_but_local_failure_stops() {
    let id = JobId::try_from("0123456789abcdef0123456789abcdef".to_string()).unwrap();
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let classification = arc_detector::classify_argv(&["fixture".into()]);
    for failure in [
        Failure::InvalidEndpoint,
        Failure::Local,
        Failure::Cancelled,
        Failure::Rejected,
        Failure::InvalidReply,
    ] {
        let mut calls = 0;
        let result = prepare_with(
            &id,
            &observer,
            classification,
            Instant::now() + Duration::from_secs(2),
            &|| false,
            |request, _, _| {
                calls += 1;
                match request {
                    StartRequest::Register { .. } => Ok(registered(&id, &observer, classification)),
                    StartRequest::Claim { .. } => Err(failure),
                    _ => unreachable!(),
                }
            },
        );
        assert_eq!(calls, 2);
        assert_eq!(
            result,
            if failure == Failure::InvalidEndpoint {
                Ok(Tracking::Uncertain)
            } else {
                Err(failure)
            }
        );
    }
}

#[test]
fn invalid_addresses_preserve_nonlaunching_cli_preparation() {
    let id = JobId::try_from("0123456789abcdef0123456789abcdef".to_string()).unwrap();
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let classification = arc_detector::classify_argv(&["fixture".into()]);
    for path in [
        PathBuf::from("x".repeat(200)),
        PathBuf::from(OsString::from_vec(b"socket\0suffix".to_vec())),
    ] {
        assert_eq!(
            call_until(
                &path,
                "REGISTER",
                serde_json::Value::Null,
                deadline(),
                &|| false
            ),
            Err(Failure::InvalidEndpoint)
        );
        let mut calls = 0;
        let result = prepare_with(
            &id,
            &observer,
            classification,
            deadline(),
            &|| false,
            |request, end, cancelled| {
                calls += 1;
                crate::exec::start_until(&path, request, end, cancelled)
            },
        );
        assert_eq!(calls, 1);
        assert_eq!(result, Err(Failure::InvalidEndpoint));
    }
}

//! Production desktop bridge + private real daemon, CLI and native child.
//! No display, browser, production database or existing daemon is controlled.
#![cfg(unix)]

mod daemon_bridge_support;
#[path = "../../../../crates/arc-daemon/tests/support/native_exec.rs"]
mod execution;
#[allow(dead_code)] // Other native suites use the remaining shared fixture ports.
#[path = "../../../../crates/arc-daemon/tests/support/native_fixture.rs"]
mod native;
#[path = "../../../../crates/arc-daemon/tests/support/mod.rs"]
mod serialization;

use arc_core::process_identity::{probe_live, IdentityStatus, ProcessIdentity};
use arc_desktop::{
    dto::JobLookup,
    error::{DesktopError, ErrorCode},
};
use arc_ipc::jobs::{Outcome, Phase};
use daemon_bridge_support::{desktop_client, detail, Fault, HeldJob, RealDaemon};
use serde_json::json;
use std::{fs, time::Instant};

fn recorded_exit(case: &mut RealDaemon, code: u8) -> (String, arc_desktop::dto::JobDetail) {
    let mut job = HeldJob::launch(case, code);
    let running = case.wait_running(&job.identity);
    let id = detail(&running).job_id.as_str().to_owned();
    assert_eq!(detail(&running).phase, Phase::Running);
    assert_eq!(detail(&running).outcome, None, "running is not success");
    let wrapper_exit = job.release_and_wait();
    assert_eq!(wrapper_exit.code(), Some(i32::from(code)));
    let recorded = case.bridge.job_by_id(&id).unwrap();
    let found = detail(&recorded);
    assert_eq!(found.phase, Phase::Completed);
    assert_eq!(found.outcome, Some(Outcome::Exited { code }));
    assert!(found.registered_at_ms <= found.launch_claimed_at_ms.unwrap());
    assert!(found.launch_claimed_at_ms.unwrap() <= found.completed_at_ms.unwrap());
    assert!(found.completed_at_ms.unwrap() <= found.updated_at_ms);
    assert!(found.updated_at_ms <= recorded.received_at_ms);
    assert!(found.observation.observed_at_ms <= recorded.received_at_ms);
    assert!(recorded.received_at_ms >= running.received_at_ms);
    let wire = case.snapshot(&id);
    assert_eq!(wire.child.as_ref(), Some(&job.identity));
    assert_eq!(wire.observer, job.wrapper_identity);
    assert_eq!(wire.outcome, found.outcome);
    assert_eq!(
        fs::read_to_string(case.fixture.root.join("launches"))
            .unwrap()
            .lines()
            .count(),
        1
    );
    case.save("recorded-exit", &json!({
        "nativeWrapperExit": wrapper_exit.code(), "childHandshake": job.handshake,
        "wrapperIdentity": job.wrapper_identity, "typedReply": recorded,
        "wireSnapshot": wire, "profiles": case.profiles(&[job.identity.pid, job.wrapper_identity.pid]),
    }));
    (id, found.clone())
}

#[test]
fn records_exit_zero() {
    let _serial = serialization::serial();
    let mut case = RealDaemon::new("records_exit_zero");
    recorded_exit(&mut case, 0);
    case.stop("exit-zero");
}

#[test]
fn records_exit_seventeen() {
    let _serial = serialization::serial();
    let mut case = RealDaemon::new("records_exit_seventeen");
    let (id, failed) = recorded_exit(&mut case, 17);
    // The native command succeeded as a lookup while the job itself failed.
    assert_eq!(
        detail(&case.bridge.job_by_id(&id).unwrap()).outcome,
        failed.outcome
    );
    case.stop("exit-seventeen");
}

#[test]
fn disconnect_reconnect() {
    let _serial = serialization::serial();
    let mut case = RealDaemon::new("disconnect_reconnect");
    let (id, cached) = recorded_exit(&mut case, 17);
    let first_daemon = case.identity();
    case.stop("before-reconnect");
    let started = Instant::now();
    let health_error = case.bridge.daemon_health().unwrap_err();
    let detail_error = case.bridge.job_by_id(&id).unwrap_err();
    assert_eq!(health_error, DesktopError::new(ErrorCode::Unavailable));
    assert_eq!(detail_error, DesktopError::new(ErrorCode::Unavailable));
    assert!(started.elapsed().as_secs_f64() < 3.0);
    // A failed refresh returns no replacement result. The last recorded exit
    // and its observation/completion times remain usable as stale client data.
    assert_eq!(cached.outcome, Some(Outcome::Exited { code: 17 }));
    case.start();
    assert_ne!(case.identity(), first_daemon);
    let reconnected = case.bridge.daemon_health().unwrap();
    assert_eq!(reconnected.data.protocol_version, 1);
    let selected = case.bridge.job_by_id(&id).unwrap();
    assert_eq!(detail(&selected).outcome, cached.outcome);
    assert_eq!(detail(&selected).completed_at_ms, cached.completed_at_ms);
    assert_eq!(detail(&selected).registered_at_ms, cached.registered_at_ms);
    assert_eq!(
        fs::read_to_string(case.fixture.root.join("launches"))
            .unwrap()
            .lines()
            .count(),
        1
    );
    assert_eq!(case.bridge.recent_jobs().unwrap().data.jobs.len(), 1);
    assert!(matches!(
        case.bridge
            .job_by_id("ffffffffffffffffffffffffffffffff")
            .unwrap()
            .data,
        JobLookup::NotFound {}
    ));
    case.save(
        "reconnect",
        &json!({
            "disconnectedHealth": health_error, "disconnectedDetail": detail_error,
            "retainedStaleDetail": cached, "firstDaemon": first_daemon,
            "reconnectedDaemon": case.identity(), "health": reconnected, "selected": selected,
            "launchCount": 1,
        }),
    );
    case.stop("after-reconnect");
}

#[test]
fn rejects_malformed_response() {
    let _serial = serialization::serial();
    let mut case = RealDaemon::new("rejects_malformed_response");
    let (id, cached) = recorded_exit(&mut case, 17);
    for fault in [
        Fault::Json,
        Fault::Oversized,
        Fault::Shape,
        Fault::Correlation,
        Fault::Protocol,
        Fault::Stall,
    ] {
        let (bridge, server) = case.fault_server(fault, None);
        let started = Instant::now();
        let error = bridge.job_by_id(&id).unwrap_err();
        let elapsed = started.elapsed();
        assert!(
            elapsed.as_secs_f64() < 3.0,
            "read exceeded the production two-second deadline plus scheduling margin"
        );
        assert_eq!(error.code, fault.expected_code());
        assert_eq!(
            detail(&case.bridge.job_by_id(&id).unwrap()).outcome,
            cached.outcome
        );
        case.save(
            fault.name(),
            &json!({
                "faultServerRequest": server.join().unwrap(), "failure": error,
                "elapsedMs": elapsed.as_millis(), "retainedStaleDetail": cached,
            }),
        );
    }
    case.stop("malformed");
}

#[test]
fn rejects_mismatched_job_id() {
    let _serial = serialization::serial();
    let mut case = RealDaemon::new("rejects_mismatched_job_id");
    let (id, cached) = recorded_exit(&mut case, 0);
    let mut substituted = case.snapshot(&id);
    substituted.job_id =
        arc_ipc::jobs::JobId::try_from("ffffffffffffffffffffffffffffffff".to_owned()).unwrap();
    assert!(
        substituted.is_valid(),
        "reject valid snapshots for the wrong selection too"
    );
    let (bridge, server) = case.fault_server(Fault::Selection, Some(substituted));
    let started = Instant::now();
    let error = bridge.job_by_id(&id).unwrap_err();
    assert_eq!(error, DesktopError::new(ErrorCode::InvalidReply));
    assert!(started.elapsed().as_secs_f64() < 3.0);
    assert_eq!(
        detail(&case.bridge.job_by_id(&id).unwrap()).outcome,
        cached.outcome
    );
    case.save(
        "mismatched-selection",
        &json!({
            "faultServerRequest": server.join().unwrap(), "failure": error,
            "retainedSelectedDetail": cached,
        }),
    );
    case.stop("selection");
}

#[test]
fn daemon_survives_desktop_exit() {
    // Re-exec this exact test as the headless desktop client. It takes no
    // fixture lock and terminates only itself via the registered quit handler.
    if daemon_bridge_support::is_client_process() {
        daemon_bridge_support::run_client_process();
    }
    let _serial = serialization::serial();
    let mut case = RealDaemon::new("daemon_survives_desktop_exit");
    let (id, cached) = recorded_exit(&mut case, 17);
    let daemon = case.identity();
    let client = desktop_client(&case, "daemon_survives_desktop_exit", &id);
    assert_eq!(probe_live(&daemon), IdentityStatus::VerifiedMatch);
    assert_eq!(ProcessIdentity::capture(daemon.pid).unwrap(), daemon);
    assert_eq!(
        case.bridge.daemon_health().unwrap().data.protocol_version,
        1
    );
    let selected = case.bridge.job_by_id(&id).unwrap();
    assert_eq!(detail(&selected).outcome, cached.outcome);
    case.save(
        "daemon-survival",
        &json!({
            "daemonBefore": daemon, "daemonAfter": case.identity(),
            "client": client, "selectedAfterClientExit": selected,
        }),
    );
    case.stop("survived-desktop");
}

#[test]
fn active_child_survives_desktop_exit() {
    if daemon_bridge_support::is_client_process() {
        daemon_bridge_support::run_client_process();
    }
    let _serial = serialization::serial();
    let mut case = RealDaemon::new("active_child_survives_desktop_exit");
    let mut job = HeldJob::launch(&case, 17);
    let running = case.wait_running(&job.identity);
    let id = detail(&running).job_id.as_str().to_owned();
    let daemon = case.identity();
    let client = desktop_client(&case, "active_child_survives_desktop_exit", &id);
    for identity in [&daemon, &job.identity, &job.wrapper_identity] {
        assert_eq!(probe_live(identity), IdentityStatus::VerifiedMatch);
        assert_eq!(&ProcessIdentity::capture(identity.pid).unwrap(), identity);
    }
    let survived = case.bridge.job_by_id(&id).unwrap();
    assert_eq!(detail(&survived).phase, Phase::Running);
    assert_eq!(detail(&survived).outcome, None);
    let wire = case.snapshot(&id);
    assert_eq!(wire.child.as_ref(), Some(&job.identity));
    assert_eq!(wire.observer, job.wrapper_identity);
    assert_eq!(job.release_and_wait().code(), Some(17));
    let completed = case.bridge.job_by_id(&id).unwrap();
    assert_eq!(
        detail(&completed).outcome,
        Some(Outcome::Exited { code: 17 })
    );
    assert_eq!(
        fs::read_to_string(case.fixture.root.join("launches"))
            .unwrap()
            .lines()
            .count(),
        1
    );
    case.save("active-child-survival", &json!({
        "daemonBefore": daemon, "daemonAfter": case.identity(), "childHandshake": job.handshake,
        "wrapperIdentity": job.wrapper_identity, "client": client, "runningAfterClientExit": survived,
        "completedAfterRelease": completed, "wireAfterClientExit": wire,
        "profiles": case.profiles(&[job.identity.pid, job.wrapper_identity.pid]), "launchCount": 1,
    }));
    case.stop("active-child");
}

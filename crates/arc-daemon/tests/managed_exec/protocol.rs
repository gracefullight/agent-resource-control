use super::{runtime::Fixture, support};
use arc_core::process_identity::ProcessIdentity;
use arc_ipc::{
    jobs::{
        CompleteDisposition, CompleteRequest, CompleteResult, JobId, JobSnapshot, LifecycleReason,
        Outcome, Phase, StartDisposition, StartRequest, StartResult, StatusRequest, StatusResult,
        ERROR_JOB_CONFLICT, OPERATION_JOB_COMPLETE, OPERATION_JOB_START, OPERATION_JOB_STATUS,
    },
    Request, Response,
};
use serde_json::{json, Value};
use std::thread;
pub fn call(f: &Fixture, op: &str, payload: Value) -> Response {
    let mut request = Request::new("lifecycle-fixture", op);
    request.payload = payload;
    arc_ipc::exchange(&mut f.connect(), &request).unwrap()
}
pub fn id(n: u32) -> JobId {
    JobId::try_from(format!("{n:032x}")).unwrap()
}
pub fn observer() -> ProcessIdentity {
    ProcessIdentity::capture(std::process::id()).unwrap()
}
pub fn register(n: u32) -> StartRequest {
    StartRequest::Register {
        job_id: id(n),
        observer: observer(),
        classification: serde_json::from_value(json!({"classification":"UNKNOWN","locality":"UNKNOWN","reason":"UNRESOLVED_COMMAND","uncertainty":"OBSERVE"})).unwrap(),
    }
}
pub fn start(f: &Fixture, request: &StartRequest) -> StartResult {
    let response = call(f, OPERATION_JOB_START, json!(request));
    assert_eq!(response.status, "ok", "{response:?}");
    serde_json::from_value(response.result.unwrap()).unwrap()
}
pub fn status(f: &Fixture, job_id: &JobId) -> JobSnapshot {
    let response = call(
        f,
        OPERATION_JOB_STATUS,
        json!(StatusRequest::ById {
            job_id: job_id.clone()
        }),
    );
    assert_eq!(response.status, "ok", "{response:?}");
    let StatusResult::Found { job } = serde_json::from_value(response.result.unwrap()).unwrap()
    else {
        panic!("missing lifecycle");
    };
    *job
}
#[test]
fn retries_response_loss_concurrent_claim_and_completion_are_idempotent() {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("");
    // Disconnect before reading each real committed response, then retry by key.
    for request in [
        register(1),
        StartRequest::Claim {
            job_id: id(1),
            observer: observer(),
        },
    ] {
        let mut stream = f.connect();
        let mut wire = Request::new("lost", OPERATION_JOB_START);
        wire.payload = json!(request);
        arc_ipc::write_message(&mut stream, &wire).unwrap();
        drop(stream);
        let retried = start(&f, &request);
        assert!(retried.job.is_valid());
    }
    thread::scope(|scope| {
        for _ in 0..8 {
            scope.spawn(|| {
                let result = start(
                    &f,
                    &StartRequest::Claim {
                        job_id: id(1),
                        observer: observer(),
                    },
                );
                assert_eq!(result.job.phase, Phase::Launching);
                assert_eq!(result.disposition, StartDisposition::Unchanged);
            });
        }
    });
    let complete = CompleteRequest {
        job_id: id(1),
        observer: observer(),
        outcome: Outcome::Exited { code: 17 },
    };
    let first = call(&f, OPERATION_JOB_COMPLETE, json!(complete));
    assert_eq!(first.status, "ok");
    let second = call(&f, OPERATION_JOB_COMPLETE, json!(complete));
    let result: CompleteResult = serde_json::from_value(second.result.unwrap()).unwrap();
    assert_eq!(result.disposition, CompleteDisposition::Unchanged);
    let mut conflict = complete;
    conflict.outcome = Outcome::Exited { code: 0 };
    assert_eq!(
        call(&f, OPERATION_JOB_COMPLETE, json!(conflict))
            .error
            .unwrap()
            .code,
        ERROR_JOB_CONFLICT
    );
    assert_eq!(
        status(&f, &id(1)).outcome,
        Some(Outcome::Exited { code: 17 })
    );
    daemon.stop(libc::SIGTERM);
    f.report();
}
#[test]
fn identity_mismatch_missing_and_restart_never_invent_an_exit() {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    for n in [2, 3] {
        start(&f, &register(n));
        start(
            &f,
            &StartRequest::Claim {
                job_id: id(n),
                observer: observer(),
            },
        );
        let mut child = observer();
        if n == 2 {
            child.start_token = (child.start_token.parse::<u64>().unwrap() + 1).to_string();
        } else {
            child.pid = i32::MAX as u32;
        }
        let result = start(
            &f,
            &StartRequest::Attach {
                job_id: id(n),
                observer: observer(),
                child,
            },
        );
        assert_eq!(result.job.phase, Phase::OutcomeUnknown);
    }
    daemon.stop(libc::SIGTERM);
    let mut daemon = super::cli::restart(&f);
    for (n, reason) in [
        (2, LifecycleReason::IdentityMismatch),
        (3, LifecycleReason::ProcessMissing),
    ] {
        let job = status(&f, &id(n));
        assert_eq!(job.reason, reason);
        assert_eq!(job.phase, Phase::OutcomeUnknown);
        assert!(job.outcome.is_none());
    }
    daemon.stop(libc::SIGTERM);
    f.report();
}
#[test]
fn version_first_typed_payload_validation_recent_bound_and_secret_absence() {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let sentinel = "PRIVATE_LIFECYCLE_SENTINEL";
    let mut bad = Request::new("safe", OPERATION_JOB_START);
    bad.protocol_version = 99;
    bad.payload = json!({"secret":sentinel});
    assert_eq!(
        arc_ipc::exchange(&mut f.connect(), &bad)
            .unwrap()
            .error
            .unwrap()
            .code,
        arc_ipc::ERROR_PROTOCOL_INCOMPATIBLE
    );
    bad.protocol_version = 1;
    let invalid = arc_ipc::exchange(&mut f.connect(), &bad).unwrap();
    assert_eq!(invalid.error.unwrap().code, arc_ipc::ERROR_INVALID_REQUEST);
    for n in 10..62 {
        start(&f, &register(n));
    }
    let recent = call(&f, OPERATION_JOB_STATUS, json!(StatusRequest::Recent {}));
    let StatusResult::Recent {
        jobs,
        truncated,
        liveness_fresh,
        ..
    } = serde_json::from_value(recent.result.unwrap()).unwrap()
    else {
        panic!()
    };
    assert_eq!(jobs.len(), 50);
    assert!(truncated);
    assert!(!liveness_fresh);
    assert_eq!(
        call(
            &f,
            OPERATION_JOB_STATUS,
            json!(StatusRequest::ById { job_id: id(999) })
        )
        .result
        .unwrap()["kind"],
        "NOT_FOUND"
    );
    daemon.stop(libc::SIGTERM);
    f.report();
    for path in [&f.database, &f.root.join("stdout"), &f.root.join("stderr")] {
        assert!(!std::fs::read(path)
            .unwrap()
            .windows(sentinel.len())
            .any(|w| w == sentinel.as_bytes()));
    }
}

#[test]
fn unreaped_quick_exit_keeps_identity_but_is_not_reported_running() {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    start(&f, &register(77));
    start(
        &f,
        &StartRequest::Claim {
            job_id: id(77),
            observer: observer(),
        },
    );
    let mut child = std::process::Command::new("/usr/bin/true").spawn().unwrap();
    let mut info = unsafe { std::mem::zeroed() };
    assert_eq!(
        unsafe {
            libc::waitid(
                libc::P_PID,
                child.id(),
                &mut info,
                libc::WEXITED | libc::WNOWAIT,
            )
        },
        0
    );
    let identity = ProcessIdentity::capture(child.id()).unwrap();
    let attached = start(
        &f,
        &StartRequest::Attach {
            job_id: id(77),
            observer: observer(),
            child: identity,
        },
    );
    assert_eq!(attached.job.phase, Phase::OutcomeUnknown);
    assert!(attached.job.child_verified_at_ms.is_some());
    assert_eq!(status(&f, &id(77)).phase, Phase::OutcomeUnknown);
    let observed = status(&f, &id(77)).observation.observed_at_ms;
    let response = call(
        &f,
        OPERATION_JOB_COMPLETE,
        json!(CompleteRequest {
            job_id: id(77),
            observer: observer(),
            outcome: Outcome::Exited { code: 0 }
        }),
    );
    assert_eq!(response.status, "ok");
    let completed: CompleteResult = serde_json::from_value(response.result.unwrap()).unwrap();
    assert_eq!(completed.job.observation.observed_at_ms, observed);
    assert!(child.wait().unwrap().success());
    daemon.stop(libc::SIGTERM);
    f.report();
}

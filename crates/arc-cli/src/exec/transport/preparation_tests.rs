//! CLI-only preparation decisions following shared-client failures.
use super::{call_until, Failure};
use crate::exec::preparation::{prepare_with, Tracking};
use arc_ipc::jobs::{
    IdentityObservation, JobId, JobSnapshot, LifecycleReason, ObservationStatus, ObservationTarget,
    Phase, ProcessIdentity, StartDisposition, StartRequest, StartResult,
};
use std::{
    cell::RefCell,
    collections::VecDeque,
    ffi::OsString,
    os::unix::ffi::OsStringExt,
    path::PathBuf,
    time::{Duration, Instant},
};

// Thread-local scripted transport lets the actual admission/fallback functions
// expose their deadlines without timing a subprocess or changing production I/O.
type ScriptedExchange =
    Box<dyn FnMut(&str, &serde_json::Value, Instant) -> Result<serde_json::Value, Failure>>;
thread_local! {
    static EXCHANGE: RefCell<Option<ScriptedExchange>> = RefCell::new(None);
}
pub(super) fn scripted_exchange(
    operation: &str,
    payload: &serde_json::Value,
    deadline: Instant,
) -> Option<Result<serde_json::Value, Failure>> {
    EXCHANGE.with(|slot| {
        slot.borrow_mut()
            .as_mut()
            .map(|exchange| exchange(operation, payload, deadline))
    })
}
struct ScriptedExchangeGuard;
impl Drop for ScriptedExchangeGuard {
    fn drop(&mut self) {
        EXCHANGE.with(|slot| *slot.borrow_mut() = None);
    }
}

#[test]
fn fallback_routes_keep_the_exact_original_deadline_even_when_already_expired() {
    use crate::exec::{governor, signals::Signals, traffic::Mode};
    use std::{fs, os::unix::fs::DirBuilderExt, rc::Rc};

    const NAME: &str = "exec::transport::preparation_tests::fallback_routes_keep_the_exact_original_deadline_even_when_already_expired";
    const EXECUTED: &str = "ARC_DEADLINE_EXECUTED";
    // Signals owns process-wide handlers. Follow the existing native signal
    // tests: install them only in an isolated, exact-selected libtest process.
    if std::env::var("ARC_DEADLINE_CASE").as_deref() != Ok(NAME) {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", NAME, "--nocapture"])
            .env("ARC_DEADLINE_CASE", NAME)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        assert!(String::from_utf8_lossy(&output.stdout)
            .lines()
            .any(|line| line == EXECUTED));
        return;
    }

    let id = JobId::try_from("0123456789abcdef0123456789abcdef".to_owned()).unwrap();
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let root = arc_core::runtime_root().unwrap().join(format!(
        "deadline-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
    let socket = root.join("missing-ipc");
    let signals = Signals::install(true).unwrap();
    for expired in [false, true] {
        for bun in [false, true] {
            let original = if expired {
                Instant::now() - Duration::from_secs(1)
            } else {
                Instant::now() + Duration::from_secs(60)
            };
            let (probe, operation, capability, argv) = if bun {
                (
                    "traffic.bun.capabilities",
                    "traffic.bun.job",
                    "traffic.bun.v1",
                    ["bun".into(), "test".into()],
                )
            } else {
                (
                    "traffic.capabilities",
                    "traffic.job",
                    "traffic.v1",
                    ["cargo".into(), "build".into()],
                )
            };
            let expected = Rc::new(RefCell::new(VecDeque::from([
                probe,
                operation,
                "job.start",
            ])));
            let pending = expected.clone();
            EXCHANGE.with(|slot| {
                assert!(slot
                    .borrow_mut()
                    .replace(Box::new(move |actual, payload, deadline| {
                        assert_eq!(
                            deadline, original,
                            "{actual} renewed the pre-spawn deadline"
                        );
                        assert_eq!(pending.borrow_mut().pop_front(), Some(actual));
                        if actual == probe {
                            Ok(serde_json::json!({"capability":capability}))
                        } else if actual == operation {
                            if bun {
                                Err(Failure::Unsupported)
                            } else {
                                Ok(serde_json::json!(arc_ipc::traffic::JobResult::Bypassed {
                                    reason: arc_ipc::traffic::BypassReason::Unsupported,
                                }))
                            }
                        } else {
                            assert_eq!(payload["action"], "REGISTER");
                            Err(Failure::Uncertain)
                        }
                    }))
                    .is_none());
            });
            let _guard = ScriptedExchangeGuard;
            // The unavailable Governor probe and both legacy Traffic fallbacks
            // run their production control flow. Only the transport is scripted.
            let result = governor::prepare(
                &socket,
                &id,
                &observer,
                arc_detector::classify_argv(&argv),
                original,
                &signals,
            );
            assert!(matches!(result, Ok(Mode::Legacy(Tracking::Uncertain))));
            assert!(expected.borrow().is_empty());
        }
    }
    drop(signals);
    fs::remove_dir_all(root).unwrap();
    println!("{EXECUTED}");
}
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

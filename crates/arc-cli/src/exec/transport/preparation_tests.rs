//! CLI-only preparation decisions following shared-client failures.
mod native_tools;
mod python_tests;
mod stalled_registration;
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
type ScriptedExchange = Box<
    dyn FnMut(
        &str,
        &serde_json::Value,
        Instant,
        &dyn Fn() -> bool,
    ) -> Result<serde_json::Value, Failure>,
>;
thread_local! {
    static EXCHANGE: RefCell<Option<ScriptedExchange>> = RefCell::new(None);
}
pub(super) fn scripted_exchange(
    operation: &str,
    payload: &serde_json::Value,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
) -> Option<Result<serde_json::Value, Failure>> {
    EXCHANGE.with(|slot| {
        slot.borrow_mut()
            .as_mut()
            .map(|exchange| exchange(operation, payload, deadline, cancelled))
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
        for kind in 0..4 {
            let bun = kind == 1;
            let script = kind == 2;
            let python = kind == 3;
            let injected_now = if expired {
                Instant::now() - Duration::from_secs(2)
            } else {
                Instant::now()
            };
            let original = crate::exec::preparation::deadline(injected_now);
            assert_eq!(
                original.duration_since(injected_now),
                Duration::from_millis(1800)
            );
            let (probe, operation, capability, argv) = if python {
                (
                    arc_ipc::traffic::OPERATION_PYTHON_TESTS_CAPABILITIES,
                    arc_ipc::traffic::OPERATION_PYTHON_TESTS_JOB,
                    arc_ipc::traffic::PYTHON_TESTS_CAPABILITY,
                    ["pytest".into(), "-q".into()],
                )
            } else if script {
                (
                    arc_ipc::traffic::OPERATION_BUN_SCRIPTS_CAPABILITIES,
                    arc_ipc::traffic::OPERATION_BUN_SCRIPTS_JOB,
                    arc_ipc::traffic::BUN_SCRIPTS_CAPABILITY,
                    ["bun".into(), "run".into()],
                )
            } else if bun {
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
                    .replace(Box::new(move |actual, payload, deadline, _| {
                        assert_eq!(
                            deadline, original,
                            "{actual} renewed the pre-spawn deadline"
                        );
                        assert_eq!(pending.borrow_mut().pop_front(), Some(actual));
                        if actual == probe {
                            Ok(serde_json::json!({"capability":capability}))
                        } else if actual == operation {
                            if bun || script || python {
                                Err(Failure::Unsupported)
                            } else {
                                Ok(serde_json::json!(arc_ipc::traffic::JobResult::Bypassed {
                                    reason: arc_ipc::traffic::BypassReason::Unsupported,
                                }))
                            }
                        } else {
                            assert_eq!(payload["action"], "REGISTER");
                            if python {
                                assert_eq!(
                                    payload["classification"]["reason"],
                                    "UNRESOLVED_COMMAND"
                                );
                                assert_eq!(payload["classification"]["uncertainty"], "OBSERVE");
                            }
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
                if script {
                    serde_json::from_value(serde_json::json!({
                        "classification":"TYPECHECK", "locality":"HOST_LOCAL",
                        "reason":"BUN_SCRIPT_TYPECHECK", "uncertainty":"NONE"
                    }))
                    .unwrap()
                } else {
                    arc_detector::classify_argv(&argv)
                },
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

#[test]
fn acknowledged_preparation_cancellation_never_launches() {
    use std::{
        io::{Read, Write},
        os::fd::AsRawFd,
        process::{Command, Stdio},
    };
    const NAME: &str =
        "exec::transport::preparation_tests::acknowledged_preparation_cancellation_never_launches";
    if let Ok(action) = std::env::var("ARC_CANCEL_AT") {
        let marker = PathBuf::from(std::env::var_os("ARC_CANCEL_MARKER").unwrap());
        EXCHANGE.with(|slot| {
            *slot.borrow_mut() = Some(Box::new(move |operation, payload, end, cancelled| {
                if operation == arc_ipc::governor::OPERATION_CAPABILITIES {
                    return Err(Failure::Unsupported);
                }
                assert_eq!(operation, "job.start");
                let request: StartRequest = serde_json::from_value(payload.clone()).unwrap();
                let stage = payload["action"].as_str().unwrap();
                if stage != action {
                    let StartRequest::Register {
                        job_id,
                        observer,
                        classification,
                    } = request
                    else {
                        panic!("expected REGISTER")
                    };
                    return Ok(serde_json::to_value(registered(
                        &job_id,
                        &observer,
                        classification,
                    ))
                    .unwrap());
                }
                println!("preparing:{stage}");
                std::io::stdout().flush().unwrap();
                // This transport admission barrier is explicitly released by a real
                // SIGTERM. No production timeout is extended or reset while held.
                let watchdog = Instant::now() + Duration::from_secs(60);
                while !cancelled() {
                    assert!(Instant::now() < watchdog);
                    std::thread::yield_now();
                }
                assert!(end <= watchdog);
                Err(Failure::Cancelled)
            }))
        });
        let result = crate::exec::run(vec![
            "/usr/bin/touch".into(),
            marker.clone().into_os_string(),
        ]);
        assert_eq!(result, std::process::ExitCode::FAILURE);
        assert!(!marker.exists());
        std::process::exit(1);
    }
    let root = arc_core::runtime_root().unwrap().join(format!(
        "cancel-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    std::fs::create_dir(&root).unwrap();
    let marker = root.join("no-child");
    for stage in ["REGISTER", "CLAIM"] {
        let mut child = native::OwnedChild::new(
            Command::new(std::env::current_exe().unwrap())
                .args(["--exact", NAME, "--nocapture"])
                .env("ARC_CANCEL_AT", stage)
                .env("ARC_CANCEL_MARKER", &marker)
                .env(arc_core::ARC_SOCKET_PATH, root.join("unused"))
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .stdin(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let mut stdout = child.stdout.take().unwrap();
        let fd = stdout.as_raw_fd();
        assert!(unsafe { libc::fcntl(fd, libc::F_SETFL, libc::O_NONBLOCK) } >= 0);
        let mut observed = Vec::new();
        let watchdog = Instant::now() + Duration::from_secs(60);
        while !String::from_utf8_lossy(&observed).contains(&format!("preparing:{stage}\n")) {
            let mut bytes = [0; 1024];
            match stdout.read(&mut bytes) {
                Ok(0) => panic!(
                    "preparation child exited: {}",
                    String::from_utf8_lossy(&observed)
                ),
                Ok(n) => observed.extend_from_slice(&bytes[..n]),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(1))
                }
                Err(e) => panic!("{e}"),
            }
            assert!(Instant::now() < watchdog, "preparation admission watchdog");
        }
        child.stdout = Some(stdout);
        assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGTERM) }, 0);
        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert_eq!(
            output.stderr,
            b"NOT_STARTED\nExecution cancelled. No child was started.\n"
        );
        assert!(!marker.exists());
    }
    std::fs::remove_dir(root).unwrap();
}

use crate::native_fixture as native;

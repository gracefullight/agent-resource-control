//! Real authenticated IPC exercises commit tracking and admission together.
use super::*;
use arc_ipc::{Request, Response};
use std::{
    os::unix::net::UnixListener,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    thread::{self, JoinHandle},
};

#[derive(Clone, Copy)]
enum Scenario {
    BusyThenUnsupported,
    QueuedBusyThenUnsupported,
    BusyBeforeQueue,
    BusyAfterQueue,
    HealthyQueue,
    CancelBusy(bool),
}

struct Observation {
    request: Value,
    witness: Option<(u64, Vec<u8>)>,
}

#[derive(Default)]
struct Events {
    operations: Vec<(String, Value)>,
    admissions: Vec<Observation>,
    cancel_witness: Option<Vec<u8>>,
    busy_count: usize,
}

struct Peer {
    stop: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
    events: Arc<Mutex<Events>>,
}

impl Peer {
    fn new(root: &Path, scenario: Scenario) -> Self {
        let listener = UnixListener::bind(root.join("s")).unwrap();
        listener.set_nonblocking(true).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let events = Arc::new(Mutex::new(Events::default()));
        let (thread_stop, thread_events, root) = (stop.clone(), events.clone(), root.to_owned());
        let join = thread::spawn(move || {
            let mut claim = None;
            while !thread_stop.load(Ordering::SeqCst) {
                let mut stream = match listener.accept() {
                    Ok((stream, _)) => stream,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(1));
                        continue;
                    }
                    Err(e) => panic!("listener: {e}"),
                };
                // Darwin inherits the listener's nonblocking flag on accept. A
                // partial frame must wait for its remainder instead of being
                // mistaken for a lost reply and triggering outage fail-open.
                stream.set_nonblocking(false).unwrap();
                // Darwin may reject SO_RCVTIMEO if the client closed between
                // accept and setup, e.g. when its admission deadline expired.
                if let Err(error) = stream.set_read_timeout(Some(Duration::from_secs(2))) {
                    if matches!(
                        error.raw_os_error(),
                        Some(libc::EINVAL) | Some(libc::ENOTCONN)
                    ) {
                        continue;
                    }
                    panic!("setting accepted peer timeout: {error}");
                }
                let request: Request = match arc_ipc::read_message(&mut stream) {
                    Ok(request) => request,
                    // Cancellation may close a connection before sending a frame.
                    Err(_) => continue,
                };
                let mut e = thread_events.lock().unwrap();
                e.operations
                    .push((request.operation.clone(), request.payload.clone()));
                let mut cancel_after_reply = false;
                let response = if request.operation == OPERATION_CAPABILITIES {
                    Response::ok(&request.request_id, json!({"capability": CAPABILITY}))
                } else if request.operation == OPERATION_JOB {
                    let job = &request.payload["request"];
                    let action = job["action"].as_str().unwrap();
                    if action == "ENQUEUE" || action == "POLL" {
                        // The child is never started while an admission is pending.
                        assert!(!root.join("child-started").exists());
                        if action == "ENQUEUE" {
                            claim = Some(job["lineage_context"].clone());
                        }
                        let witness_path = root.join(format!(
                            ".traffic-witness-{}",
                            job["job_id"].as_str().unwrap()
                        ));
                        let witness = fs::read(&witness_path)
                            .ok()
                            .map(|bytes| (fs::metadata(&witness_path).unwrap().ino(), bytes));
                        e.admissions.push(Observation {
                            request: job.clone(),
                            witness,
                        });
                        let count = e.admissions.len();
                        let queue = matches!(
                            scenario,
                            Scenario::BusyAfterQueue
                                | Scenario::QueuedBusyThenUnsupported
                                | Scenario::CancelBusy(true)
                        ) && count == 1
                            || matches!(scenario, Scenario::HealthyQueue) && count <= 4;
                        if queue {
                            Response::ok(
                                &request.request_id,
                                json!({"state":"TRAFFIC", "result": {
                                    "state":"QUEUED", "sequence":1, "queued_at_ms":1, "reason":"CLASS_WINDOW"
                                }}),
                            )
                        } else if matches!(scenario, Scenario::HealthyQueue) {
                            Response::ok(
                                &request.request_id,
                                json!({"state":"TRAFFIC", "result": {
                                    "state":"GRANTED", "sequence":1, "queued_at_ms":1, "granted_at_ms":2,
                                    "permit_generation":1, "reservation":{"cpu_millis":1000,"memory_bytes":1024},
                                    "lineage_context":claim.clone().unwrap()
                                }}),
                            )
                        } else if matches!(scenario, Scenario::BusyThenUnsupported) && count > 1
                            || matches!(scenario, Scenario::QueuedBusyThenUnsupported) && count > 2
                        {
                            Response::error(
                                &request.request_id,
                                arc_ipc::ERROR_UNSUPPORTED_CAPABILITY,
                            )
                        } else {
                            e.busy_count += 1;
                            cancel_after_reply =
                                matches!(scenario, Scenario::CancelBusy(_)) && e.busy_count == 3;
                            Response::error(&request.request_id, arc_ipc::ERROR_DB_BUSY)
                        }
                    } else if action == "CANCEL" {
                        let id = job["job_id"].as_str().unwrap();
                        e.cancel_witness =
                            fs::read(root.join(format!(".traffic-witness-{id}"))).ok();
                        Response::ok(
                            &request.request_id,
                            json!({"state":"TRAFFIC", "result":{"state":"CANCELLED"}}),
                        )
                    } else {
                        // Post-spawn reconciliation can fail without authorizing replay.
                        Response::error(&request.request_id, arc_ipc::ERROR_UNSUPPORTED_CAPABILITY)
                    }
                } else {
                    Response::error(&request.request_id, arc_ipc::ERROR_UNSUPPORTED_CAPABILITY)
                };
                drop(e);
                let _ = arc_ipc::write_response(&mut stream, &response);
                if cancel_after_reply {
                    assert_eq!(unsafe { libc::kill(libc::getpid(), libc::SIGTERM) }, 0);
                }
            }
        });
        Self {
            stop,
            join: Some(join),
            events,
        }
    }
}

impl Drop for Peer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        let result = self.join.take().unwrap().join();
        if !thread::panicking() {
            result.unwrap();
        }
    }
}

// A broken unbounded loop must fail the isolated test instead of hanging it.
struct Watchdog(mpsc::Sender<()>, Option<JoinHandle<()>>);
impl Watchdog {
    fn start() -> Self {
        let (tx, rx) = mpsc::channel();
        let join = thread::spawn(move || {
            if matches!(
                rx.recv_timeout(Duration::from_secs(6)),
                Err(mpsc::RecvTimeoutError::Timeout)
            ) {
                unsafe { libc::kill(libc::getpid(), libc::SIGTERM) };
            }
        });
        Self(tx, Some(join))
    }
}
impl Drop for Watchdog {
    fn drop(&mut self) {
        let _ = self.0.send(());
        self.1.take().unwrap().join().unwrap();
    }
}

fn check_stable_admission(events: &Events, queued: bool) -> (String, (u64, Vec<u8>)) {
    assert!(events.admissions.len() >= 2);
    let first = &events.admissions[0];
    assert_eq!(first.request["action"], "ENQUEUE");
    let id = first.request["job_id"].as_str().unwrap().to_owned();
    let original = first.witness.clone().unwrap();
    assert_eq!(&original.1[..8], b"ARCWIT03");
    assert_eq!(&original.1[24..40], &[0; 16]);
    assert_eq!(&original.1[104..], id.as_bytes());
    assert_eq!(
        &original.1[40..104],
        first.request["lineage_context"]
            .as_str()
            .unwrap()
            .as_bytes()
    );
    for item in &events.admissions[1..] {
        assert_eq!(item.request["job_id"], id);
        assert_eq!(item.request["observer"], first.request["observer"]);
        if queued {
            assert_eq!(item.request["action"], "POLL");
        } else {
            assert_eq!(item.request, first.request);
        }
        assert_eq!(item.witness.as_ref().unwrap(), &original);
    }
    assert!(!events.operations.iter().any(|(op, _)| op == "job.start"));
    (id, original)
}

#[test]
fn native_tools_busy_then_unsupported_preserves_possible_commit_at_transport_boundary() {
    let root = Root::new();
    let peer = Peer::new(&root.0, Scenario::BusyThenUnsupported);
    let continuity = crate::exec::transport::Continuity::default();
    let payload = json!({"action":"TRAFFIC", "request":{"action":"POLL","job_id":"a".repeat(32)}});
    let mut committed = false;
    let exchange = |committed: &mut bool| {
        crate::exec::transport::call_until_with_continuity(
            &root.0.join("s"),
            OPERATION_JOB,
            payload.clone(),
            deadline(),
            &|| false,
            committed,
            Some(&continuity),
        )
    };
    assert_eq!(exchange(&mut committed), Err(Failure::Busy));
    assert!(committed, "DB_BUSY may follow COMMIT");
    assert_eq!(exchange(&mut committed), Err(Failure::Uncertain));
    assert!(committed);
    assert!(continuity.confirmed());
    let events = peer.events.lock().unwrap();
    assert_eq!(events.admissions.len(), 2);
    assert_eq!(events.admissions[0].request, events.admissions[1].request);
}

#[test]
fn native_tools_busy_then_unsupported_keeps_witness_and_executes_only_once() {
    const NAME: &str = "exec::transport::preparation_tests::native_tools::busy::live::native_tools_busy_then_unsupported_keeps_witness_and_executes_only_once";
    if isolated(NAME) {
        return;
    }
    for (scenario, queued) in [
        (Scenario::BusyThenUnsupported, false),
        (Scenario::QueuedBusyThenUnsupported, true),
    ] {
        let root = Root::new();
        let executable = root.0.join("go");
        fs::write(
            &executable,
            b"#!/bin/sh\nprintf 'ran\\n' >> \"$ARC_NATIVE_TOOLS_BUSY_MARKER\"\n",
        )
        .unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        std::env::set_var(arc_core::ARC_SOCKET_PATH, root.0.join("s"));
        std::env::set_var("ARC_NATIVE_TOOLS_BUSY_MARKER", root.0.join("child-started"));
        let peer = Peer::new(&root.0, scenario);
        let _watchdog = Watchdog::start();
        assert_eq!(
            crate::exec::run(vec![executable.into_os_string(), "test".into()]),
            ExitCode::SUCCESS
        );
        assert_eq!(fs::read(root.0.join("child-started")).unwrap(), b"ran\n");
        let events = peer.events.lock().unwrap();
        assert_eq!(events.admissions.len(), if queued { 3 } else { 2 });
        assert_eq!(events.busy_count, 1);
        let (id, (inode, before)) = check_stable_admission(&events, queued);
        let witness_path = root.0.join(format!(".traffic-witness-{id}"));
        let after = fs::read(&witness_path).unwrap();
        assert_eq!(fs::metadata(witness_path).unwrap().ino(), inode);
        assert_eq!(&after[..24], &before[..24]);
        assert_eq!(&after[40..], &before[40..]);
        assert!(after[24..40].iter().any(|b| *b != 0));
        for (op, payload) in &events.operations {
            if op == OPERATION_JOB {
                assert_eq!(payload["request"]["job_id"], id);
            }
        }
    }
    println!("{NAME}:executed");
}

#[test]
fn native_tools_busy_live_preserves_identity_through_retries_and_healthy_queue() {
    const NAME: &str = "exec::transport::preparation_tests::native_tools::busy::live::native_tools_busy_live_preserves_identity_through_retries_and_healthy_queue";
    if isolated(NAME) {
        return;
    }
    for (scenario, queued, healthy) in [
        (Scenario::BusyBeforeQueue, false, false),
        (Scenario::BusyAfterQueue, true, false),
        (Scenario::HealthyQueue, true, true),
    ] {
        let root = Root::new();
        let peer = Peer::new(&root.0, scenario);
        let signals = Signals::install(true).unwrap();
        let _watchdog = Watchdog::start();
        let id = JobId::try_from("a".repeat(32)).unwrap();
        let observer = ProcessIdentity::capture(std::process::id()).unwrap();
        let classification = arc_detector::classify_argv(&["go".into(), "test".into()]);
        let original_deadline = Instant::now() + Duration::from_millis(500);
        let result = governor::prepare(
            &root.0.join("s"),
            &id,
            &observer,
            classification,
            original_deadline,
            &signals,
        );
        let ended = Instant::now();
        if healthy {
            assert!(matches!(
                result,
                Ok(Mode::Scheduled {
                    route: Route::NativeTools,
                    waited: true,
                    ..
                })
            ));
        } else {
            assert!(matches!(
                result,
                Ok(Mode::Uncertain {
                    route: Route::NativeTools,
                    ..
                })
            ));
            assert!(
                ended < original_deadline + Duration::from_secs(4),
                "busy extended the deadline"
            );
        }
        let events = peer.events.lock().unwrap();
        let (_, (inode, bytes)) = check_stable_admission(&events, queued);
        let path = root.0.join(format!(".traffic-witness-{}", id.as_str()));
        assert_eq!(fs::metadata(&path).unwrap().ino(), inode);
        assert_eq!(fs::read(path).unwrap(), bytes);
        if !healthy {
            let busy = if queued {
                &events.admissions[1..]
            } else {
                &events.admissions[..]
            };
            assert!(busy.len() >= 2);
        }
        drop(events);
        drop(result);
        drop(signals);
    }
    println!("{NAME}:executed");
}

#[test]
fn native_tools_busy_outage_fails_open_once_with_the_same_witness() {
    const NAME: &str = "exec::transport::preparation_tests::native_tools::busy::live::native_tools_busy_outage_fails_open_once_with_the_same_witness";
    if isolated(NAME) {
        return;
    }
    for (scenario, queued) in [
        (Scenario::BusyBeforeQueue, false),
        (Scenario::BusyAfterQueue, true),
    ] {
        let root = Root::new();
        let executable = root.0.join("go");
        fs::write(
            &executable,
            b"#!/bin/sh\nprintf 'ran\\n' >> \"$ARC_NATIVE_TOOLS_BUSY_MARKER\"\n",
        )
        .unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        std::env::set_var(arc_core::ARC_SOCKET_PATH, root.0.join("s"));
        std::env::set_var("ARC_NATIVE_TOOLS_BUSY_MARKER", root.0.join("child-started"));
        let peer = Peer::new(&root.0, scenario);
        let _watchdog = Watchdog::start();
        assert_eq!(
            crate::exec::run(vec![executable.into_os_string(), "test".into()]),
            ExitCode::SUCCESS
        );
        assert_eq!(fs::read(root.0.join("child-started")).unwrap(), b"ran\n");
        let events = peer.events.lock().unwrap();
        let (id, (inode, before)) = check_stable_admission(&events, queued);
        let path = root.0.join(format!(".traffic-witness-{id}"));
        let after = fs::read(&path).unwrap();
        assert_eq!(fs::metadata(path).unwrap().ino(), inode);
        assert_eq!(&after[40..], &before[40..]);
        assert!(after[24..40].iter().any(|b| *b != 0));
    }
    println!("{NAME}:executed");
}

#[test]
fn native_tools_busy_real_transport_cancellation_never_executes() {
    const NAME: &str = "exec::transport::preparation_tests::native_tools::busy::live::native_tools_busy_real_transport_cancellation_never_executes";
    if isolated(NAME) {
        return;
    }
    for queued in [false, true] {
        let root = Root::new();
        let executable = root.0.join("go");
        fs::write(
            &executable,
            b"#!/bin/sh\nprintf ran > \"$ARC_NATIVE_TOOLS_BUSY_MARKER\"\n",
        )
        .unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        std::env::set_var(arc_core::ARC_SOCKET_PATH, root.0.join("s"));
        std::env::set_var("ARC_NATIVE_TOOLS_BUSY_MARKER", root.0.join("child-started"));
        let peer = Peer::new(&root.0, Scenario::CancelBusy(queued));
        let _watchdog = Watchdog::start();
        assert_eq!(
            crate::exec::run(vec![executable.into_os_string(), "test".into()]),
            ExitCode::FAILURE
        );
        assert!(!root.0.join("child-started").exists());
        let events = peer.events.lock().unwrap();
        let (_, (_, before)) = check_stable_admission(&events, queued);
        let cancel = events.cancel_witness.as_ref().unwrap();
        assert_eq!(&cancel[..8], b"ARCNOS03");
        assert_eq!(&cancel[8..], &before[8..]);
    }
    println!("{NAME}:executed");
}

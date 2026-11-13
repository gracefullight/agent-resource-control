use super::*;

#[test]
fn protocol_admission_enforces_windows_budget_fifo_and_bypass() {
    use arc_ipc::{
        jobs::{JobId, ProcessIdentity},
        traffic::{JobRequest, JobResult},
    };
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let enqueue = |number: u32, classification| JobRequest::Enqueue {
        job_id: JobId::try_from(format!("{number:032x}")).unwrap(),
        observer: observer.clone(),
        classification,
        lineage_context: None,
    };
    let test = enqueue(1, cargo_class("TEST", "CARGO_TEST"));
    let lint = enqueue(2, cargo_class("LINT", "CARGO_CLIPPY"));
    let waiting_test = enqueue(3, cargo_class("TEST", "CARGO_TEST"));
    let waiting_build = enqueue(4, cargo_class("BUILD", "CARGO_BUILD"));
    let JobResult::Granted {
        permit_generation: test_generation,
        ..
    } = traffic_result(traffic(&f, &test))
    else {
        panic!("test must be granted")
    };
    let JobResult::Granted {
        permit_generation: lint_generation,
        ..
    } = traffic_result(traffic(&f, &lint))
    else {
        panic!("lint must be granted")
    };
    assert!(matches!(
        traffic_result(traffic(&f, &waiting_test)),
        JobResult::Queued { .. }
    ));
    assert!(matches!(
        traffic_result(traffic(&f, &waiting_build)),
        JobResult::Queued { .. }
    ));
    let before = active(&f);
    assert_eq!((before.0, before.1, before.2), (4, 2, 2000));
    for classification in [
        serde_json::from_value(serde_json::json!({"classification":"REMOTE_CI","locality":"REMOTE","reason":"GH_WORKFLOW_RUN","uncertainty":"NONE"})).unwrap(),
        serde_json::from_value(serde_json::json!({"classification":"UNKNOWN","locality":"UNKNOWN","reason":"UNRESOLVED_COMMAND","uncertainty":"OBSERVE"})).unwrap(),
    ] {
        assert!(matches!(traffic_result(traffic(&f, &enqueue(10, classification))), JobResult::Bypassed { .. }));
        assert_eq!(active(&f), before);
    }
    let cancel = |number: u32, generation| JobRequest::Cancel {
        job_id: JobId::try_from(format!("{number:032x}")).unwrap(),
        observer: observer.clone(),
        permit_generation: generation,
    };
    assert_eq!(
        traffic_result(traffic(&f, &cancel(1, Some(test_generation)))),
        JobResult::Cancelled {}
    );
    let poll = |number: u32| JobRequest::Poll {
        job_id: JobId::try_from(format!("{number:032x}")).unwrap(),
        observer: observer.clone(),
    };
    assert!(matches!(
        traffic_result(traffic(&f, &poll(4))),
        JobResult::Queued { .. }
    ));
    let JobResult::Granted {
        permit_generation: second_test_generation,
        ..
    } = traffic_result(traffic(&f, &poll(3)))
    else {
        panic!("FIFO head must be granted")
    };
    assert!(matches!(
        traffic_result(traffic(&f, &poll(4))),
        JobResult::Queued { .. }
    ));
    for (number, generation) in [(2, lint_generation), (3, second_test_generation)] {
        assert_eq!(
            traffic_result(traffic(&f, &cancel(number, Some(generation)))),
            JobResult::Cancelled {}
        );
    }
    let JobResult::Granted {
        permit_generation: build_generation,
        ..
    } = traffic_result(traffic(&f, &poll(4)))
    else {
        panic!("build must be granted after release")
    };
    assert_eq!(
        traffic_result(traffic(&f, &cancel(4, Some(build_generation)))),
        JobResult::Cancelled {}
    );
    assert_eq!(active(&f).1, 0);
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn lost_grant_and_completion_acknowledgements_are_idempotent() {
    use arc_ipc::{
        jobs::{JobId, Outcome, ProcessIdentity, SpawnReason},
        traffic::{JobRequest, JobResult},
    };
    use std::io::Write;
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let id = JobId::try_from("b123456789abcdef0123456789abcdef".to_owned()).unwrap();
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let enqueue = JobRequest::Enqueue {
        job_id: id.clone(),
        observer: observer.clone(),
        classification: cargo_class("BUILD", "CARGO_BUILD"),
        lineage_context: None,
    };
    // A disconnected partial frame cannot commit an enqueue.
    let mut partial = f.connect();
    partial.write_all(&[0, 0]).unwrap();
    drop(partial);
    assert_eq!(active(&f).0, 0);
    let mut wire = arc_ipc::Request::new("lost-grant", arc_ipc::traffic::OPERATION_TRAFFIC_JOB);
    wire.payload = serde_json::to_value(&enqueue).unwrap();
    let mut grant_stream = f.connect();
    arc_ipc::write_message(&mut grant_stream, &wire).unwrap();
    eventually(|| active(&f).1 == 1);
    drop(grant_stream);
    let first = traffic_result(traffic(&f, &enqueue));
    let duplicate = traffic_result(traffic(&f, &enqueue));
    assert_eq!(first, duplicate);
    let JobResult::Granted {
        permit_generation, ..
    } = first
    else {
        panic!("expected one grant")
    };
    assert_eq!(active(&f).1, 1);
    let complete = JobRequest::Complete {
        job_id: id,
        observer,
        permit_generation,
        outcome: Outcome::SpawnFailed {
            reason: SpawnReason::NotFound,
            os_code: Some(libc::ENOENT),
        },
    };
    let mut wire =
        arc_ipc::Request::new("lost-completion", arc_ipc::traffic::OPERATION_TRAFFIC_JOB);
    wire.payload = serde_json::to_value(&complete).unwrap();
    let mut completion_stream = f.connect();
    arc_ipc::write_message(&mut completion_stream, &wire).unwrap();
    eventually(|| active(&f).1 == 0);
    drop(completion_stream);
    let completed = traffic_result(traffic(&f, &complete));
    assert!(matches!(completed, JobResult::Completed { .. }));
    assert_eq!(traffic_result(traffic(&f, &complete)), completed);
    assert_eq!(active(&f).1, 0);
    let total: i64 = database(&f)
        .query_row("SELECT count(*) FROM permits", [], |r| r.get(0))
        .unwrap();
    assert_eq!(total, 1);
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn traffic_outage_fails_open_once_within_the_exchange_budget() {
    use std::{
        os::unix::net::UnixListener,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
    };
    let _serial = support::serial();
    let f = Fixture::new();
    let path = f.root.join("outage.sock");
    let listener = UnixListener::bind(&path).unwrap();
    listener.set_nonblocking(true).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = stop.clone();
    let proxy = thread::spawn(move || {
        while !worker_stop.load(Ordering::SeqCst) {
            let (mut stream, _) = match listener.accept() {
                Ok(stream) => stream,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(1));
                    continue;
                }
                Err(error) => panic!("{error}"),
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let request: arc_ipc::Request = arc_ipc::read_message(&mut stream).unwrap();
            if let Some(response) = policy_compat::unsupported_probe(&request) {
                arc_ipc::write_response(&mut stream, &response).unwrap();
            } else if request.operation == arc_ipc::traffic::OPERATION_TRAFFIC_CAPABILITIES {
                let response = arc_ipc::Response::ok(
                    &request.request_id,
                    serde_json::to_value(arc_ipc::traffic::Capabilities::supported()).unwrap(),
                );
                arc_ipc::write_response(&mut stream, &response).unwrap();
            }
        }
    });
    symlink(binary("arc-exec-fixture"), f.root.join("cargo")).unwrap();
    let marker = f.root.join("outage-invocations");
    let started = Instant::now();
    let mut wrapper = OwnedChild::new(
        Command::new(binary("arcctl"))
            .args(["exec", "--", "cargo", "build"])
            .env(arc_core::ARC_SOCKET_PATH, &path)
            .env("PATH", &f.root)
            .env("ARC_FIXTURE_MODE", "echo")
            .env("ARC_FIXTURE_MARKER", &marker)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::from(
                File::create(f.root.join("outage-stderr")).unwrap(),
            ))
            .spawn()
            .unwrap(),
    );
    eventually(|| marker.exists());
    assert!(started.elapsed() < Duration::from_millis(2500));
    eventually(|| wrapper.child.try_wait().unwrap().is_some());
    assert_eq!(wrapper.child.wait().unwrap().code(), Some(17));
    assert_eq!(fs::read_to_string(marker).unwrap().lines().count(), 1);
    assert!(fs::read_to_string(f.root.join("outage-stderr"))
        .unwrap()
        .contains("TRACKING_UNCERTAIN"));
    stop.store(true, Ordering::SeqCst);
    proxy.join().unwrap();
}

#[test]
fn new_cli_uses_legacy_lifecycle_after_explicit_unsupported_probe() {
    use std::{
        os::unix::net::{UnixListener, UnixStream},
        sync::{
            atomic::{AtomicBool, AtomicUsize, Ordering},
            Arc,
        },
    };
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let proxy_path = f.root.join("legacy.sock");
    let listener = UnixListener::bind(&proxy_path).unwrap();
    listener.set_nonblocking(true).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let probes = Arc::new(AtomicUsize::new(0));
    let worker_stop = stop.clone();
    let worker_probes = probes.clone();
    let backend = f.socket.clone();
    let proxy = thread::spawn(move || {
        while !worker_stop.load(Ordering::SeqCst) {
            let (mut stream, _) = match listener.accept() {
                Ok(stream) => stream,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(1));
                    continue;
                }
                Err(error) => panic!("{error}"),
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let request: arc_ipc::Request = arc_ipc::read_message(&mut stream).unwrap();
            let response = if let Some(response) = policy_compat::unsupported_probe(&request) {
                response
            } else if request.operation == arc_ipc::traffic::OPERATION_TRAFFIC_CAPABILITIES {
                worker_probes.fetch_add(1, Ordering::SeqCst);
                arc_ipc::Response::error(&request.request_id, arc_ipc::ERROR_UNSUPPORTED_CAPABILITY)
            } else {
                arc_ipc::exchange(&mut UnixStream::connect(&backend).unwrap(), &request).unwrap()
            };
            arc_ipc::write_response(&mut stream, &response).unwrap();
        }
    });
    symlink(binary("arc-exec-fixture"), f.root.join("cargo")).unwrap();
    let output = Command::new(binary("arcctl"))
        .args(["exec", "--", "cargo", "build"])
        .env(arc_core::ARC_SOCKET_PATH, &proxy_path)
        .env("PATH", &f.root)
        .env("ARC_FIXTURE_MODE", "echo")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    stop.store(true, Ordering::SeqCst);
    proxy.join().unwrap();
    assert_eq!(output.status.code(), Some(17), "{output:?}");
    assert_eq!(probes.load(Ordering::SeqCst), 1);
    assert_eq!(active(&f).1, 0);
    let count: i64 = database(&f)
        .query_row(
            "SELECT count(*) FROM job_lifecycle WHERE outcome_kind='EXITED'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1);
    daemon.stop(libc::SIGTERM);
    f.report();
}

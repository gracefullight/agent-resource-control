use super::*;
use std::{
    os::unix::net::{UnixListener, UnixStream},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
};

#[test]
fn new_status_client_falls_back_when_a_legacy_daemon_rejects_status_v2() {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    symlink(binary("arc-exec-fixture"), f.root.join("cargo")).unwrap();
    let output = Command::new(binary("arcctl"))
        .args(["exec", "--", "cargo", "build"])
        .env(arc_core::ARC_SOCKET_PATH, &f.socket)
        .env("PATH", &f.root)
        .env("ARC_FIXTURE_MODE", "echo")
        .env_remove("ARC_TRAFFIC_LINEAGE")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert_eq!(output.code(), Some(17));
    let id: String = database(&f)
        .query_row("SELECT job_id FROM job_lifecycle", [], |row| row.get(0))
        .unwrap();
    let path = f.root.join("legacy-status.sock");
    let listener = UnixListener::bind(&path).unwrap();
    listener.set_nonblocking(true).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let rejected = Arc::new(AtomicUsize::new(0));
    let legacy = Arc::new(AtomicUsize::new(0));
    let thread_stop = stop.clone();
    let thread_rejected = rejected.clone();
    let thread_legacy = legacy.clone();
    let backend = f.socket.clone();
    let server = thread::spawn(move || {
        while !thread_stop.load(Ordering::SeqCst) {
            let (mut stream, _) = match listener.accept() {
                Ok(pair) => pair,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(1));
                    continue;
                }
                Err(error) => panic!("status fixture accept: {error}"),
            };
            stream.set_nonblocking(false).unwrap();
            let request: arc_ipc::Request = arc_ipc::read_message(&mut stream).unwrap();
            let response = if request.operation == arc_ipc::traffic::OPERATION_TRAFFIC_JOB {
                assert_eq!(request.payload["action"], "STATUS_V2");
                thread_rejected.fetch_add(1, Ordering::SeqCst);
                arc_ipc::Response::error(&request.request_id, arc_ipc::ERROR_INVALID_REQUEST)
            } else {
                assert_eq!(request.operation, arc_ipc::jobs::OPERATION_JOB_STATUS);
                thread_legacy.fetch_add(1, Ordering::SeqCst);
                arc_ipc::exchange(&mut UnixStream::connect(&backend).unwrap(), &request).unwrap()
            };
            arc_ipc::write_response(&mut stream, &response).unwrap();
        }
    });
    let status = Command::new(binary("arcctl"))
        .args(["job", "status", &id])
        .env(arc_core::ARC_SOCKET_PATH, &path)
        .output()
        .unwrap();
    stop.store(true, Ordering::SeqCst);
    server.join().unwrap();
    assert!(status.status.success(), "{status:?}");
    let text = String::from_utf8(status.stdout).unwrap();
    assert!(
        text.contains("State: completed") && text.contains("Exit code: 17"),
        "{text}"
    );
    assert_eq!(rejected.load(Ordering::SeqCst), 1);
    assert_eq!(legacy.load(Ordering::SeqCst), 1);
    assert_eq!(active(&f).1, 0);
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn delayed_recent_status_reports_unknown_for_unattached_and_dead_queued_jobs() {
    use arc_ipc::{
        jobs::{JobId, ProcessIdentity},
        traffic::{JobRequest, JobResult, StatusResult},
    };
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let id = JobId::try_from("f123456789abcdef0123456789abcdef".to_owned()).unwrap();
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let enqueue = JobRequest::Enqueue {
        job_id: id.clone(),
        observer: observer.clone(),
        classification: cargo_class("BUILD", "CARGO_BUILD"),
        lineage_context: None,
    };
    let JobResult::Granted {
        permit_generation, ..
    } = traffic_result(traffic(&f, &enqueue))
    else {
        panic!("expected grant")
    };
    let project = projects(&f.root, "delayed_status");
    let mut queued = spawn_build(&f, &project[0], true);
    eventually(|| entry(&f, queued.child.id()).is_some_and(|entry| entry.0 == "QUEUED"));
    let queued_id = JobId::try_from(
        database(&f)
            .query_row(
                "SELECT job_id FROM job_lifecycle WHERE observer_pid=?1",
                [queued.child.id()],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
    )
    .unwrap();
    queued.child.kill().unwrap();
    queued.child.wait().unwrap();
    let path = f.root.join("delayed-status.sock");
    let listener = UnixListener::bind(&path).unwrap();
    listener.set_nonblocking(true).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let delayed = Arc::new(AtomicUsize::new(0));
    let thread_stop = stop.clone();
    let thread_delayed = delayed.clone();
    let backend = f.socket.clone();
    let server = thread::spawn(move || {
        while !thread_stop.load(Ordering::SeqCst) {
            let (mut stream, _) = match listener.accept() {
                Ok(pair) => pair,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(1));
                    continue;
                }
                Err(error) => panic!("status fixture accept: {error}"),
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(1)))
                .unwrap();
            let Ok(request) = arc_ipc::read_message::<_, arc_ipc::Request>(&mut stream) else {
                continue;
            };
            if request.operation == arc_ipc::traffic::OPERATION_TRAFFIC_JOB {
                thread_delayed.fetch_add(1, Ordering::SeqCst);
                thread::sleep(Duration::from_millis(350));
            } else {
                let response =
                    arc_ipc::exchange(&mut UnixStream::connect(&backend).unwrap(), &request)
                        .unwrap();
                let _ = arc_ipc::write_response(&mut stream, &response);
            }
        }
    });
    let output = Command::new(binary("arcctl"))
        .args(["job", "status"])
        .env(arc_core::ARC_SOCKET_PATH, &path)
        .output()
        .unwrap();
    let by_id: Vec<_> = [&id, &queued_id]
        .iter()
        .map(|job_id| {
            Command::new(binary("arcctl"))
                .args(["job", "status", job_id.as_str()])
                .env(arc_core::ARC_SOCKET_PATH, &path)
                .output()
                .unwrap()
        })
        .collect();
    stop.store(true, Ordering::SeqCst);
    server.join().unwrap();
    assert!(output.status.success(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(delayed.load(Ordering::SeqCst) > 0);
    for output in by_id {
        assert!(output.status.success(), "{output:?}");
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(
            text.contains("State: unknown") && text.contains("Scheduling: status unavailable"),
            "{text}"
        );
    }
    for job_id in [&id, &queued_id] {
        let block = text
            .split("Job: ")
            .find(|block| block.starts_with(job_id.as_str()))
            .unwrap();
        assert!(block.contains("State: unknown"), "{text}");
    }
    let read = |job_id: &JobId| {
        serde_json::from_value::<StatusResult>(
            traffic(
                &f,
                &JobRequest::StatusV2 {
                    job_id: job_id.clone(),
                },
            )
            .result
            .unwrap(),
        )
        .unwrap()
    };
    assert!(matches!(read(&id), StatusResult::Unconfirmed {}));
    assert!(matches!(read(&queued_id), StatusResult::Abandoned {}));
    assert!(!project[0].join("started").exists());
    assert_eq!(
        traffic_result(traffic(
            &f,
            &JobRequest::Cancel {
                job_id: id,
                observer,
                permit_generation: Some(permit_generation)
            }
        )),
        JobResult::Cancelled {}
    );
    assert_eq!(active(&f).1, 0);
    daemon.stop(libc::SIGTERM);
    f.report();
}

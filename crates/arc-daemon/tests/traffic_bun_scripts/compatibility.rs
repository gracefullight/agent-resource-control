//! Explicit old-peer faults and bounded outage, separate from real daemon runs.
use super::{projects::*, *};
use std::{
    os::unix::net::UnixListener,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
struct Peer {
    stop: Arc<AtomicBool>,
    requests: Arc<Mutex<Vec<arc_ipc::Request>>>,
    thread: Option<thread::JoinHandle<()>>,
}
impl Peer {
    fn new(path: &Path, unsupported: bool) -> Self {
        let listener = UnixListener::bind(path).unwrap();
        listener.set_nonblocking(true).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let seen = requests.clone();
        let handle = thread::spawn(move || {
            let mut held = Vec::new();
            while !stopped.load(Ordering::Acquire) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let mut stream = super::protocol::bounded_stream(stream);
                        let request: arc_ipc::Request = arc_ipc::read_message(&mut stream).unwrap();
                        assert_eq!(request.protocol_version, 1);
                        seen.lock().unwrap().push(request.clone());
                        if unsupported
                            && (request.operation == "traffic.bun-scripts.capabilities"
                                || request.operation == "traffic.bun-scripts.job")
                        {
                            arc_ipc::write_response(
                                &mut stream,
                                &arc_ipc::Response::error(
                                    &request.request_id,
                                    arc_ipc::ERROR_UNSUPPORTED_CAPABILITY,
                                ),
                            )
                            .unwrap();
                        } else if unsupported {
                            // The frozen peer recognizes no new vocabulary. Lose
                            // legacy metadata ACKs without authorizing anything.
                            assert!(matches!(
                                request.operation.as_str(),
                                "job.start" | "job.complete" | "meter.report" | "meter.summary"
                            ));
                            if request.operation == "job.start"
                                && request.payload["action"] == "REGISTER"
                            {
                                assert_eq!(
                                    request.payload["classification"]["classification"],
                                    "UNKNOWN"
                                );
                                assert_eq!(
                                    request.payload["classification"]["reason"],
                                    "UNRESOLVED_COMMAND"
                                );
                            }
                        } else {
                            held.push(stream);
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5))
                    }
                    Err(e) => panic!("frozen peer accept: {e}"),
                }
            }
        });
        Self {
            stop,
            requests,
            thread: Some(handle),
        }
    }
    fn finish(mut self) -> Vec<arc_ipc::Request> {
        self.stop.store(true, Ordering::Release);
        self.thread.take().unwrap().join().unwrap();
        self.requests.lock().unwrap().clone()
    }
}
impl Drop for Peer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(handle) = self.thread.take() {
            if !thread::panicking() {
                handle.join().unwrap();
            } else {
                let _ = handle.join();
            }
        }
    }
}
pub fn old_peer_and_outage(read: Read) {
    let case = Case::new("old-peer-outage", read, false);
    let mut observations = Vec::new();
    for (name, unsupported) in [("old-peer", true), ("hung-peer", false)] {
        let path = case.fixture.root.join(format!("{name}.sock"));
        let peer = Peer::new(&path, unsupported);
        let p = project(&case, name, Kind::Test, true);
        let start = Instant::now();
        let mut run = Run::start_with(&case, &p, &Kind::Test.argv(), |cmd| {
            cmd.env(arc_core::ARC_SOCKET_PATH, &path);
        });
        eventually(|| {
            assert!(
                run.wrapper.child.try_wait().unwrap().is_none(),
                "{}",
                log(&p.join("stderr"))
            );
            !log(&p.join("bodies")).is_empty()
        });
        let preparation = start.elapsed();
        assert!(
            preparation < Duration::from_secs(3),
            "absolute 1800 ms preparation deadline was restarted: {preparation:?}"
        );
        assert_eq!(run.count("bun"), 1);
        assert_eq!(case.active()["permits"], 0);
        let requests = peer.finish();
        run.finish(0);
        validate_artifact(&case, &p, Kind::Test);
        assert!(log(&p.join("stderr")).contains(if unsupported {
            "TRACKING_UNCERTAIN"
        } else {
            "TRACKING_UNAVAILABLE"
        }));
        let ids: Vec<_> = requests
            .iter()
            .filter_map(|r| r.payload.get("job_id").and_then(Value::as_str))
            .collect();
        if let Some(first) = ids.first() {
            assert!(ids.iter().all(|id| id == first));
        }
        observations.push(json!({"case":name,"peer":"explicit protocol-1 fault fixture, not a historical production binary","firstBodyElapsedMs":preparation.as_millis(),"requests":requests,"starts":1,"outcome":0,"notice":log(&p.join("stderr"))}));
    }
    assert!(case.rows("SELECT id FROM jobs").is_empty());
    case.save(
        "old-peer-outage",
        json!({"observations":observations,"productionDbUnchanged":true,"activePermits":0}),
    );
    case.done();
}
pub fn replacement(read: Read) {
    let mut case = Case::new("peer-replacement", read, false);
    let a = project(&case, "root", Kind::Test, true);
    let b = project(&case, "waiting", Kind::Test, false);
    let mut first = Run::start(&case, &a, &Kind::Test.argv());
    let entry = first.entered(&case, false);
    let barrier = case.fixture.root.join("replace.sock");
    let listener = UnixListener::bind(&barrier).unwrap();
    let mut second = Run::start_with(&case, &b, &Kind::Test.argv(), |cmd| {
        cmd.env("ARC_PRESSURE_ACCEPTANCE_STAGE", "queued")
            .env("ARC_PRESSURE_ACCEPTANCE_BARRIER", &barrier);
    });
    let (mut release, checkpoint) = super::protocol::checkpoint(&listener);
    let id = case.job(second.wrapper.child.id());
    let before = case.reconcile(&id, "TEST", "BUN_SCRIPT_TEST");
    assert_eq!(before["diagnostic"]["admission"]["state"], "QUEUED");
    case.daemon.child.kill().unwrap();
    case.daemon.child.wait().unwrap();
    fs::remove_file(&case.fixture.socket).unwrap();
    fs::rename(
        case.fixture.root.join("runtime.json"),
        case.fixture.root.join("runtime-before-restart.json"),
    )
    .unwrap();
    let peer = Peer::new(&case.fixture.socket, true);
    release.write_all(b"x").unwrap();
    assert_eq!(second.wait().code(), Some(0));
    let requests = peer.finish();
    assert_eq!(second.count("bun"), 1);
    assert_eq!(log(&b.join("bodies")).lines().count(), 1);
    let notice = log(&b.join("stderr"));
    assert!(notice.contains("TRACKING_UNCERTAIN"));
    assert!(notice.contains(&id));
    let traffic: Vec<_> = requests
        .iter()
        .filter(|r| r.operation == "traffic.bun-scripts.job")
        .collect();
    assert!(!traffic.is_empty());
    for request in traffic {
        assert_eq!(request.payload["request"]["job_id"], id);
        assert_ne!(request.payload["request"]["action"], "ENQUEUE");
    }
    assert!(
        requests.iter().all(|r| r.operation != "job.start"),
        "replacement cannot erase committed script admission"
    );
    fs::remove_file(&case.fixture.socket).unwrap();
    case.daemon = native::OwnedChild::new(case.fixture.command().spawn().unwrap());
    case.fixture.wait_for(&mut case.daemon, "arc-daemon ready.");
    assert!(first.wrapper.child.try_wait().unwrap().is_none());
    assert_eq!(case.active()["permits"], 1);
    first.finish(0);
    let done = case.terminal(&first, "TEST", "BUN_SCRIPT_TEST", 0);
    // Fresh status applies native disappearance; it never executes the old job.
    let status = case.cli(&["job", "status", &id]);
    assert!(status.status.success());
    eventually(|| {
        let _ = case.cli(&["job", "status", &id]);
        matches!(
            case.row(&id)["state"].as_str(),
            Some("ABANDONED" | "CANCELLED")
        )
    });
    let after = case.reconcile(&id, "TEST", "BUN_SCRIPT_TEST");
    assert_eq!(case.active()["permits"], 0);
    assert_eq!(second.count("bun"), 1);
    assert_eq!(case.rows("SELECT count(*) AS n FROM permits")[0]["n"], 1);
    case.save("peer-replacement",json!({"priorEntry":entry,"queued":before,"checkpoint":checkpoint,"replacementRequests":requests,"replacementOriginalStarts":1,"priorJobIdRetained":id,"priorRunningOutcome":done,"afterProductionRecovery":after,"cliOutcome":0,"persistence":"uncertain until recovered; missing outcome is never fabricated from native exit","stderr":log(&b.join("stderr")),"activePermits":0}));
    case.done();
}

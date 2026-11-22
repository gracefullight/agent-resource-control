//! Production commit/replay checks and explicitly synthetic protocol-1 fault peers.
use super::*;
use std::{
    os::unix::net::UnixListener,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

fn payload(request: &arc_ipc::traffic::JobRequest) -> Value {
    json!(arc_ipc::governor::GovernorJobRequest::Traffic {
        request: request.clone()
    })
}
fn lose(case: &Case, request: &arc_ipc::traffic::JobRequest) -> std::os::unix::net::UnixStream {
    let mut wire = arc_ipc::Request::new(
        "lost-python-ack",
        arc_ipc::traffic::OPERATION_PYTHON_TESTS_JOB,
    );
    wire.payload = payload(request);
    let mut stream = case.fixture.connect();
    arc_ipc::write_message(&mut stream, &wire).unwrap();
    stream
}
fn replay(case: &Case, request: &arc_ipc::traffic::JobRequest) -> Value {
    case.rpc(
        arc_ipc::traffic::OPERATION_PYTHON_TESTS_JOB,
        payload(request),
    )
}
pub fn acknowledgments(read: Read) {
    use arc_ipc::{
        jobs::{JobId, Outcome},
        traffic::JobRequest,
    };
    let case = case("lost-acknowledgments", read, false);
    let id = JobId::try_from("1234567890abcdef1234567890abcdef".to_owned()).unwrap();
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let class = serde_json::from_value(json!({"classification":"TEST","locality":"HOST_LOCAL","reason":"PYTHON_PYTEST","uncertainty":"NONE"})).unwrap();
    let enqueue = JobRequest::Enqueue {
        job_id: id.clone(),
        observer: observer.clone(),
        classification: class,
        lineage_context: None,
        project: None,
        event: None,
    };
    let lost = lose(&case, &enqueue);
    eventually(|| case.active()["permits"] == 1);
    drop(lost);
    let grant = replay(&case, &enqueue);
    assert_eq!(replay(&case, &enqueue), grant);
    assert_eq!(grant["result"]["state"], "GRANTED");
    let generation = grant["result"]["permit_generation"].as_u64().unwrap();
    // The raw client is the authenticated observer. A transport proxy would
    // change peer PID and correctly fail owner authentication. Exercise the
    // real launch fence here without rewriting identities or adding a hook.
    let launch = arc_ipc::governor::GovernorLaunchRequest::Reserved {
        request: arc_ipc::policy::launch::LaunchRequest {
            job_id: id.clone(),
            observer: observer.clone(),
            permit_generation: generation,
        },
    };
    let mut gate = case.fixture.connect();
    let mut prepare =
        arc_ipc::Request::new("python-raw-launch", arc_ipc::governor::OPERATION_LAUNCH);
    prepare.payload = json!(launch);
    let release = arc_ipc::exchange(&mut gate, &prepare).unwrap();
    assert_eq!(release.status, "ok", "{release:?}");
    assert_eq!(release.result.as_ref().unwrap()["state"], "RESERVED");
    let p = projects::project(&case, "real-python-child", Kind::Pytest, true);
    let mut command = Command::new(case.tools.shims.join("python"));
    command
        .args(["-m", "pytest"])
        .current_dir(&p)
        .stdin(Stdio::null())
        .stdout(File::create(p.join("stdout")).unwrap())
        .stderr(File::create(p.join("stderr")).unwrap());
    projects::configure(&case, &p, &mut command);
    let wrapper = native::OwnedChild::new(command.spawn().unwrap());
    let child = ProcessIdentity::capture(wrapper.child.id()).unwrap();
    let mut started = arc_ipc::Request::new(
        "python-lost-start-ack",
        arc_ipc::governor::OPERATION_LAUNCH_ACK,
    );
    started.payload = json!(arc_ipc::governor::GovernorLaunchAck::Reserved {
        ack: arc_ipc::policy::launch::LaunchAck::Started {
            job_id: id.clone(),
            observer: observer.clone(),
            permit_generation: generation,
            child: child.clone(),
        },
    });
    arc_ipc::write_message(&mut gate, &started).unwrap();
    // MSG_PEEK establishes that the real daemon produced its response without
    // consuming/decoding the ACK. Discard it before any client read/retry.
    use std::os::fd::AsRawFd;
    eventually(|| {
        let mut byte = 0_u8;
        let n = unsafe {
            libc::recv(
                gate.as_raw_fd(),
                (&mut byte as *mut u8).cast(),
                1,
                libc::MSG_PEEK | libc::MSG_DONTWAIT,
            )
        };
        if n < 0 {
            assert_eq!(
                std::io::Error::last_os_error().kind(),
                std::io::ErrorKind::WouldBlock
            );
        }
        n > 0
    });
    drop(gate);
    let mut run = Run {
        wrapper,
        identity: child.clone(),
        project: p.clone(),
        fifo: None,
    };
    eventually(|| log(&p.join("bodies")).lines().count() == 1);
    assert_eq!(probe_live(&child), IdentityStatus::VerifiedMatch);
    let attach = JobRequest::Attach {
        job_id: id.clone(),
        observer: observer.clone(),
        child: child.clone(),
        permit_generation: generation,
    };
    let lost = lose(&case, &attach);
    eventually(|| case.row(id.as_str())["state"] == "RUNNING");
    drop(lost);
    assert_eq!(
        replay(&case, &attach),
        json!({"state":"TRAFFIC","result":{"state":"RUNNING"}})
    );
    let running = case.reconcile(id.as_str(), "TEST", "PYTHON_PYTEST");
    run.finish(0);
    assert_eq!(probe_live(&child), IdentityStatus::Missing);
    let complete = JobRequest::Complete {
        job_id: id.clone(),
        observer,
        permit_generation: generation,
        outcome: Outcome::Exited { code: 0 },
    };
    let lost = lose(&case, &complete);
    eventually(|| case.active()["permits"] == 0);
    drop(lost);
    let finished = replay(&case, &complete);
    assert_eq!(replay(&case, &complete), finished);
    assert_eq!(
        finished["result"]["outcome"],
        json!({"kind":"EXITED","code":0})
    );
    let terminal = case.reconcile(id.as_str(), "TEST", "PYTHON_PYTEST");
    let meter_cli = case.cli(&["meter", "summary", "--job", id.as_str(), "--json"]);
    assert!(meter_cli.status.success());
    let meter: Value = serde_json::from_slice(&meter_cli.stdout).unwrap();
    assert_eq!(
        meter,
        (case.read)(&case.fixture.socket, "meter_summary", json!({"job_id":id}))
    );
    assert_eq!(
        meter["accounting"],
        json!({"state":"unknown","reason":"unavailable"})
    );
    let legacy = case.rpc(
        "job.status",
        json!(arc_ipc::jobs::StatusRequest::ById { job_id: id.clone() }),
    );
    let projection = json!({"classification":"UNKNOWN","locality":"UNKNOWN","reason":"UNRESOLVED_COMMAND","uncertainty":"OBSERVE"});
    assert_eq!(legacy["job"]["classification"], projection);
    let old_diagnostic = case.rpc("traffic.diagnostics.job", json!({"job_id":id}));
    assert_eq!(old_diagnostic["classification"], projection);
    assert!(old_diagnostic["admission"].is_null());
    assert_eq!(
        case.row(id.as_str())["classification_reason"],
        "PYTHON_PYTEST",
        "legacy projections must never change stored facts"
    );
    assert_eq!(log(&p.join("launches")).lines().count(), 1);
    assert_eq!(log(&p.join("bodies")).lines().count(), 1);
    let history = permit_history(&case, 1, 1);
    case.save("lost-acknowledgments", json!({"scope":"real daemon authenticated raw client intentionally does not consume enqueue/launch-start/attach/complete replies; raw client is not CLI wait4 accounting","grant":grant,"launch":release,"lostStartRequest":started,"running":running,"completed":terminal,"meter":meter,"legacyLifecycle":legacy,"legacyDiagnostics":old_diagnostic,"starts":1,"bodyCount":1,"permits":history}));
    case.done();
}

struct FaultPeer {
    stop: Arc<AtomicBool>,
    requests: Arc<Mutex<Vec<arc_ipc::Request>>>,
    handle: Option<thread::JoinHandle<()>>,
}
impl FaultPeer {
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
                        let mut stream = shared::protocol::bounded_stream(stream);
                        let request: arc_ipc::Request = arc_ipc::read_message(&mut stream).unwrap();
                        assert_eq!(request.protocol_version, 1);
                        seen.lock().unwrap().push(request.clone());
                        if unsupported && request.operation == "traffic.python-tests.capabilities" {
                            assert!(request.payload.is_null());
                            arc_ipc::write_response(
                                &mut stream,
                                &arc_ipc::Response::error(
                                    &request.request_id,
                                    arc_ipc::ERROR_UNSUPPORTED_CAPABILITY,
                                ),
                            )
                            .unwrap();
                        } else if unsupported {
                            assert!(
                                matches!(
                                    request.operation.as_str(),
                                    "job.start" | "job.complete" | "meter.report" | "meter.summary"
                                ),
                                "new vocabulary reached a legacy peer: {request:?}"
                            );
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
                    Err(e) => panic!("synthetic Python peer accept: {e}"),
                }
            }
        });
        Self {
            stop,
            requests,
            handle: Some(handle),
        }
    }
    fn finish(mut self) -> Vec<arc_ipc::Request> {
        self.stop.store(true, Ordering::Release);
        self.handle.take().unwrap().join().unwrap();
        self.requests.lock().unwrap().clone()
    }
}
impl Drop for FaultPeer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(handle) = self.handle.take() {
            if thread::panicking() {
                let _ = handle.join();
            } else {
                handle.join().unwrap();
            }
        }
    }
}
pub fn compatibility_and_outage(read: Read) {
    let case = case("compatibility-outage", read, false);
    let mut observations = Vec::new();
    for (name, unsupported) in [("unsupported-capability", true), ("hung-peer", false)] {
        let socket = case.fixture.root.join(format!("{name}.sock"));
        let peer = FaultPeer::new(&socket, unsupported);
        let p = projects::project(&case, name, Kind::Pytest, true);
        let start_at = Instant::now();
        let mut run = start_with(&case, &p, &Kind::Pytest.argv(), |cmd| {
            cmd.env(arc_core::ARC_SOCKET_PATH, &socket);
        });
        eventually(|| {
            assert!(
                run.wrapper.child.try_wait().unwrap().is_none(),
                "{}",
                log(&p.join("stderr"))
            );
            log(&p.join("bodies")).lines().count() == 1
        });
        let body_at = start_at.elapsed();
        let requests = peer.finish();
        run.finish(0);
        assert_eq!(log(&p.join("launches")).lines().count(), 1);
        assert_eq!(log(&p.join("bodies")).lines().count(), 1);
        let notice = log(&p.join("stderr"));
        assert!(
            notice.contains(if unsupported {
                "TRACKING_UNCERTAIN"
            } else {
                "TRACKING_UNAVAILABLE"
            }),
            "{notice}"
        );
        let ids: Vec<_> = requests
            .iter()
            .filter_map(|r| r.payload.get("job_id").and_then(Value::as_str))
            .collect();
        if let Some(first) = ids.first() {
            assert!(ids.iter().all(|id| id == first));
        }
        observations.push(json!({"case":name,"syntheticPeer":true,"protocol":1,"requests":requests,"firstBodyElapsedMs":body_at.as_millis(),"starts":1,"nativeExit":0,"notice":notice}));
    }
    let p = projects::project(&case, "absent-socket", Kind::Unittest, false);
    let mut run = start_with(&case, &p, &Kind::Unittest.argv(), |cmd| {
        cmd.env(
            arc_core::ARC_SOCKET_PATH,
            case.fixture.root.join("missing.sock"),
        );
    });
    run.finish(0);
    assert_eq!(log(&p.join("launches")).lines().count(), 1);
    assert_eq!(log(&p.join("bodies")).lines().count(), 1);
    assert!(log(&p.join("stderr")).contains("TRACKING_UNAVAILABLE"));
    assert!(case.rows("SELECT id FROM jobs").is_empty());
    assert_eq!(case.active()["permits"], 0);
    case.save("compatibility-outage", json!({"scenarios":observations,"absentSocketExit":0,"productionDatabaseUnchanged":true,"deadlineProof":"task-2: exec::transport::preparation_tests::fallback_routes_keep_the_exact_original_deadline_even_when_already_expired covers Python with injected time; native elapsed above is observational, not a wall-clock deadline substitute"}));
    case.done();
}

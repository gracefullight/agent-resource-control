use super::*;
use std::{
    os::unix::net::{UnixListener, UnixStream},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
};

// Executed old-client fixture based on the accepted protocol-1 status shapes
// and closed classification enum; source provenance is in diagnostics/frozen.rs.
const FROZEN_OLD_CLIENT: &str = r#"
import json, socket, struct, sys
path, jid, expected = sys.argv[1:]
def readn(s, n):
    out = b''
    while len(out) < n:
        data = s.recv(n-len(out))
        if not data: raise EOFError()
        out += data
    return out
def call(op, payload):
    s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM); s.connect(path)
    req = {'protocol_version':1,'request_id':'old-client','operation':op,'payload':payload}
    data = json.dumps(req).encode(); s.sendall(struct.pack('>I',len(data))+data)
    n = struct.unpack('>I',readn(s,4))[0]; response = json.loads(readn(s,n)); s.close()
    assert set(response) == {'protocol_version','request_id','status','result'}
    assert response['protocol_version'] == 1 and response['request_id'] == 'old-client' and response['status'] == 'ok'
    return response['result']
t = call('traffic.job', {'action':'STATUS','job_id':jid})
fields = {'QUEUED':{'state','sequence','queued_at_ms'}, 'RUNNING':{'state','queue_wait_ms'}, 'COMPLETED':{'state','queue_wait_ms','outcome'}}
assert t['state'] == expected and set(t) == fields[expected]
if expected == 'COMPLETED': assert t['outcome'] == {'kind':'EXITED','code':0}
r = call('job.status', {'query':'BY_ID','job_id':jid})
assert set(r) == {'kind','job'} and r['kind'] == 'FOUND'
j = r['job']
assert set(j) == {'job_id','phase','classification','observer','observer_verified_at_ms','child','child_verified_at_ms','observation','reason','outcome','registered_at_ms','launch_claimed_at_ms','completed_at_ms','updated_at_ms','revision'}
assert j['job_id'] == jid
assert j['classification'] == {'classification':'UNKNOWN','locality':'UNKNOWN','reason':'UNRESOLVED_COMMAND','uncertainty':'OBSERVE'}
"#;

fn assert_old_bun_status(f: &Fixture, id: &arc_ipc::jobs::JobId, state: &str) {
    use arc_ipc::{
        jobs::{StatusRequest, StatusResult},
        traffic::{JobRequest, StatusResult as TrafficStatus},
    };
    let mut request = arc_ipc::Request::new("old-lifecycle", arc_ipc::jobs::OPERATION_JOB_STATUS);
    request.payload = serde_json::to_value(StatusRequest::ById { job_id: id.clone() }).unwrap();
    let response = arc_ipc::exchange(&mut f.connect(), &request).unwrap();
    assert_eq!(response.status, arc_ipc::STATUS_OK);
    let value = response.result.unwrap();
    let decoded: StatusResult = serde_json::from_value(value.clone()).unwrap();
    assert!(decoded.is_valid());
    assert!(!value.to_string().contains("BUN_TEST"));
    let reply = traffic(f, &JobRequest::Status { job_id: id.clone() });
    let value = reply.result.unwrap();
    assert_eq!(value["state"], state);
    let decoded: TrafficStatus = serde_json::from_value(value.clone()).unwrap();
    assert!(decoded.is_valid());
    assert!(!value.to_string().contains("lineage_context"));
    let old = Command::new("python3")
        .args(["-c", FROZEN_OLD_CLIENT])
        .arg(&f.socket)
        .arg(id.as_str())
        .arg(state)
        .output()
        .unwrap();
    assert!(
        old.status.success(),
        "old-client fixture: {}",
        String::from_utf8_lossy(&old.stderr)
    );
}

fn independent_holder(f: &Fixture, class: &str) -> (arc_ipc::jobs::JobId, OwnedChild, PathBuf) {
    let ready = f.root.join("holder-ready");
    let release = f.root.join("holder-release");
    let holder = OwnedChild::new(
        Command::new(binary("arc-exec-fixture"))
            .env("ARC_FIXTURE_HOLD_UNATTACHED", "1")
            .env("HOLDER_CLASS", class)
            .env("HOLDER_READY", &ready)
            .env("HOLDER_RELEASE", &release)
            .env(arc_core::ARC_SOCKET_PATH, &f.socket)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::from(
                File::create(f.root.join("holder-stderr")).unwrap(),
            ))
            .spawn()
            .unwrap(),
    );
    eventually(|| ready.exists());
    let id = arc_ipc::jobs::JobId::try_from(fs::read_to_string(ready).unwrap()).unwrap();
    (id, holder, release)
}

#[test]
fn legacy_status_shapes_decode_bun_while_queued_running_and_completed() {
    use arc_ipc::jobs::JobId;
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let (_holder_id, mut holder, release) = independent_holder(&f, "TEST");
    let (project, mut bun) = real_bun_fixture(&f, "bun_legacy_status");
    eventually(|| entry(&f, bun.child.id()).is_some_and(|e| e.0 == "QUEUED"));
    let bun_id = JobId::try_from(
        database(&f)
            .query_row(
                "SELECT job_id FROM job_lifecycle WHERE observer_pid=?1",
                [bun.child.id()],
                |r| r.get::<_, String>(0),
            )
            .unwrap(),
    )
    .unwrap();
    assert_old_bun_status(&f, &bun_id, "QUEUED");
    fs::write(release, "release").unwrap();
    finish(&mut holder);
    eventually(|| {
        project.join("started").exists()
            && entry(&f, bun.child.id()).is_some_and(|e| e.0 == "RUNNING")
    });
    assert_old_bun_status(&f, &bun_id, "RUNNING");
    fs::write(project.join("release"), "release").unwrap();
    finish(&mut bun);
    assert_old_bun_status(&f, &bun_id, "COMPLETED");
    assert_eq!(invocations(&project), 1);
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn ambiguous_bun_replacement_keeps_witness_and_never_registers_a_second_job() {
    use std::sync::Mutex;
    let _serial = support::serial();
    for mode in ["lost", "busy", "queued", "different-store"] {
        let f = Fixture::new();
        let listener = UnixListener::bind(&f.socket).unwrap();
        listener.set_nonblocking(true).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let captured = Arc::new(Mutex::new(Vec::<arc_ipc::Request>::new()));
        let worker_stop = stop.clone();
        let worker_captured = captured.clone();
        let server = thread::spawn(move || {
            let mut enqueue_count = 0;
            while !worker_stop.load(Ordering::SeqCst) {
                let (mut stream, _) = match listener.accept() {
                    Ok(pair) => pair,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(1));
                        continue;
                    }
                    Err(e) => panic!("fixture listener: {e}"),
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let request: arc_ipc::Request = arc_ipc::read_message(&mut stream).unwrap();
                worker_captured.lock().unwrap().push(request.clone());
                let response = if request.operation == arc_ipc::traffic::OPERATION_BUN_CAPABILITIES
                {
                    arc_ipc::Response::ok(
                        &request.request_id,
                        serde_json::json!({"capability":"traffic.bun.v1"}),
                    )
                } else {
                    assert_eq!(request.operation, arc_ipc::traffic::OPERATION_BUN_JOB);
                    match request.payload["action"].as_str().unwrap() {
                        "ENQUEUE" => {
                            enqueue_count += 1;
                            if enqueue_count == 1 && matches!(mode, "lost" | "different-store") {
                                continue;
                            }
                            if enqueue_count == 1 && mode == "busy" {
                                arc_ipc::Response::error(
                                    &request.request_id,
                                    arc_ipc::ERROR_DB_BUSY,
                                )
                            } else if enqueue_count == 1 && mode == "queued" {
                                arc_ipc::Response::ok(
                                    &request.request_id,
                                    serde_json::json!({"state":"QUEUED","sequence":1,"queued_at_ms":1,"reason":"CLASS_WINDOW"}),
                                )
                            } else {
                                arc_ipc::Response::error(
                                    &request.request_id,
                                    arc_ipc::ERROR_UNSUPPORTED_CAPABILITY,
                                )
                            }
                        }
                        "FALLBACK" if mode == "different-store" => arc_ipc::Response::ok(
                            &request.request_id,
                            serde_json::json!({"state":"ABANDONED"}),
                        ),
                        "POLL" | "FALLBACK" | "CANCEL" => arc_ipc::Response::error(
                            &request.request_id,
                            arc_ipc::ERROR_UNSUPPORTED_CAPABILITY,
                        ),
                        other => panic!("unexpected fixture action {other}"),
                    }
                };
                let _ = arc_ipc::write_response(&mut stream, &response);
            }
        });
        symlink(binary("arc-exec-fixture"), f.root.join("bun")).unwrap();
        let marker = f.root.join("uncertain-marker");
        let output = Command::new(binary("arcctl"))
            .args(["exec", "--", "bun", "test"])
            .env(arc_core::ARC_SOCKET_PATH, &f.socket)
            .env("PATH", &f.root)
            .env("ARC_FIXTURE_MODE", "echo")
            .env("ARC_FIXTURE_MARKER", &marker)
            .env_remove("ARC_TRAFFIC_LINEAGE")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        stop.store(true, Ordering::SeqCst);
        server.join().unwrap();
        assert_eq!(
            output.status.code(),
            Some(17),
            "{mode}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stderr).contains("TRACKING_UNCERTAIN"));
        assert_eq!(fs::read_to_string(marker).unwrap().lines().count(), 1);
        let requests = captured.lock().unwrap();
        let enqueues: Vec<_> = requests
            .iter()
            .filter(|r| r.payload["action"] == "ENQUEUE")
            .collect();
        assert!(!enqueues.is_empty());
        let id = enqueues[0].payload["job_id"].as_str().unwrap();
        assert!(enqueues.iter().all(|r| r.payload["job_id"] == id));
        assert!(requests.iter().any(|r| r.payload["action"] == "FALLBACK"));
        assert!(requests
            .iter()
            .all(|r| r.operation != "job.start" && r.operation != "traffic.job"));
        assert!(f.root.join(format!(".traffic-witness-{id}")).exists());
    }
}

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
        jobs::JobId,
        traffic::{JobRequest, StatusResult},
    };
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let (id, mut holder, release) = independent_holder(&f, "BUILD");
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
    fs::write(release, "release").unwrap();
    finish(&mut holder);
    assert_eq!(active(&f).1, 0);
    daemon.stop(libc::SIGTERM);
    f.report();
}

pub(super) fn real_bun_fixture(f: &Fixture, name: &str) -> (PathBuf, BuildChild) {
    let project = f.root.join(name);
    fs::create_dir(&project).unwrap();
    fs::write(
        project.join("native.test.ts"),
        r#"
import { test } from "bun:test";
import { appendFileSync, existsSync } from "node:fs";
test("barrier", async () => {
  appendFileSync(process.env.TEST_MARKER!, `${process.pid}\n`);
  while (!existsSync(process.env.TEST_RELEASE!)) await Bun.sleep(10);
}, 60000);
"#,
    )
    .unwrap();
    let child = Command::new(binary("arcctl"))
        .args(["exec", "--", "bun", "test", "--timeout=60000"])
        .current_dir(&project)
        .env(arc_core::ARC_SOCKET_PATH, &f.socket)
        .env("TEST_MARKER", project.join("started"))
        .env("TEST_RELEASE", project.join("release"))
        .env(
            "ARC_TRAFFIC_LINEAGE",
            if name == "replacement_second" {
                "a".repeat(64)
            } else {
                String::new()
            },
        )
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(File::create(project.join("stderr")).unwrap()))
        .spawn()
        .unwrap();
    (project, BuildChild::new(OwnedChild::new(child), f, true))
}

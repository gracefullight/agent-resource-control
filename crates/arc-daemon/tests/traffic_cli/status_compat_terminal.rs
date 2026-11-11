use super::*;
use std::{
    os::unix::net::{UnixListener, UnixStream},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
};

fn seed_lifecycle_records(f: &Fixture, outcomes: &[Option<arc_ipc::jobs::Outcome>]) -> Vec<String> {
    use arc_ipc::jobs::{CompleteRequest, JobId, ProcessIdentity, StartRequest};
    let mut db = arc_storage::open_and_migrate(&f.database).unwrap();
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let classification = serde_json::from_value(serde_json::json!({
        "classification":"UNKNOWN", "locality":"UNKNOWN",
        "reason":"UNRESOLVED_COMMAND", "uncertainty":"OBSERVE"
    }))
    .unwrap();
    let mut ids = Vec::new();
    for (index, outcome) in outcomes.iter().enumerate() {
        let id = JobId::try_from(format!("{:032x}", index + 1)).unwrap();
        let registered_at = 100 + index as u64;
        for request in [
            StartRequest::Register {
                job_id: id.clone(),
                observer: observer.clone(),
                classification,
            },
            StartRequest::Claim {
                job_id: id.clone(),
                observer: observer.clone(),
            },
        ] {
            let evidence = arc_storage::jobs::StartEvidence::gather(&request);
            arc_storage::jobs::start(&mut db, &request, &evidence, registered_at).unwrap();
        }
        if let Some(outcome) = outcome {
            arc_storage::jobs::complete(
                &mut db,
                &CompleteRequest {
                    job_id: id.clone(),
                    observer: observer.clone(),
                    outcome: outcome.clone(),
                },
                200 + index as u64,
            )
            .unwrap();
        }
        ids.push(id.as_str().to_owned());
    }
    ids
}

#[test]
fn legacy_busy_unknowns_do_not_hide_later_known_recent_outcomes() {
    use arc_ipc::jobs::Outcome;
    let _serial = support::serial();
    let f = Fixture::new();
    let ids = seed_lifecycle_records(
        &f,
        &[
            Some(Outcome::Exited { code: 0 }),
            Some(Outcome::Exited { code: 17 }),
            None,
            Some(Outcome::Exited { code: 17 }),
        ],
    );
    let db = database(&f);
    for id in [&ids[0], &ids[1], &ids[3]] {
        db.execute("UPDATE jobs SET state='COMPLETED' WHERE id=?1", [id])
            .unwrap();
    }
    db.execute("UPDATE jobs SET exit_code=NULL WHERE id=?1", [&ids[3]])
        .unwrap();
    drop(db);
    let mut daemon = f.spawn("test");
    let path = f.root.join("legacy-busy.sock");
    let listener = UnixListener::bind(&path).unwrap();
    listener.set_nonblocking(true).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let lookups = Arc::new(Mutex::new(Vec::new()));
    let server_stop = stop.clone();
    let server_lookups = lookups.clone();
    let unknown = ids[3].clone();
    let backend = f.socket.clone();
    let server = thread::spawn(move || {
        while !server_stop.load(Ordering::SeqCst) {
            let (mut stream, _) = match listener.accept() {
                Ok(pair) => pair,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(1));
                    continue;
                }
                Err(e) => panic!("legacy status proxy: {e}"),
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let request: arc_ipc::Request = arc_ipc::read_message(&mut stream).unwrap();
            let response = if request.operation == arc_ipc::traffic::OPERATION_TRAFFIC_JOB {
                let id = request.payload["job_id"].as_str().unwrap();
                server_lookups.lock().unwrap().push(format!("traffic:{id}"));
                arc_ipc::Response::error(&request.request_id, arc_ipc::ERROR_DB_BUSY)
            } else {
                assert_eq!(request.operation, arc_ipc::jobs::OPERATION_JOB_STATUS);
                if request.payload["query"] == "BY_ID" {
                    let id = request.payload["job_id"].as_str().unwrap();
                    server_lookups.lock().unwrap().push(format!("job:{id}"));
                    if id == unknown.as_str() {
                        // Accepted-HEAD daemons reject this historical row as DB_BUSY.
                        arc_ipc::Response::error(&request.request_id, arc_ipc::ERROR_DB_BUSY)
                    } else {
                        arc_ipc::exchange(&mut UnixStream::connect(&backend).unwrap(), &request)
                            .unwrap()
                    }
                } else {
                    assert_eq!(request.payload["query"], "RECENT");
                    let mut response =
                        arc_ipc::exchange(&mut UnixStream::connect(&backend).unwrap(), &request)
                            .unwrap();
                    let jobs = response.result.as_mut().unwrap()["jobs"]
                        .as_array_mut()
                        .unwrap();
                    let row = jobs
                        .iter_mut()
                        .find(|row| row["job_id"].as_str() == Some(unknown.as_str()))
                        .unwrap();
                    // Accepted-HEAD recent summaries report this row as COMPLETED.
                    row["phase"] = serde_json::json!("COMPLETED");
                    response
                }
            };
            let _ = arc_ipc::write_response(&mut stream, &response);
        }
    });
    let output = Command::new(binary("arcctl"))
        .args(["job", "status"])
        .env(arc_core::ARC_SOCKET_PATH, &path)
        .output()
        .unwrap();
    stop.store(true, Ordering::SeqCst);
    server.join().unwrap();
    assert!(output.status.success(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    for (id, state) in ids
        .iter()
        .zip(["succeeded", "failed", "unknown", "outcome unknown"])
    {
        let block = text
            .split("Job: ")
            .find(|block| block.starts_with(id.as_str()))
            .unwrap_or_else(|| panic!("missing {id}: {text}"));
        assert!(block.contains(&format!("State: {state}\n")), "{block}");
    }
    assert_eq!(
        *lookups.lock().unwrap(),
        vec![
            format!("job:{}", ids[3]),
            format!("traffic:{}", ids[2]),
            format!("job:{}", ids[1]),
            format!("job:{}", ids[0]),
        ]
    );
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn busy_traffic_preserves_known_signal_and_spawn_failure_without_hiding_later_rows() {
    use arc_ipc::jobs::{Outcome, SpawnReason};
    let _serial = support::serial();
    let f = Fixture::new();
    let ids = seed_lifecycle_records(
        &f,
        &[
            Some(Outcome::Exited { code: 0 }),
            Some(Outcome::Exited { code: 17 }),
            Some(Outcome::Signaled {
                signal: 15,
                core_dumped: false,
            }),
            Some(Outcome::SpawnFailed {
                reason: SpawnReason::NotFound,
                os_code: Some(2),
            }),
        ],
    );
    let mut daemon = f.spawn("test");
    let path = f.root.join("terminal-busy.sock");
    let listener = UnixListener::bind(&path).unwrap();
    listener.set_nonblocking(true).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let lookups = Arc::new(Mutex::new(Vec::new()));
    let server_stop = stop.clone();
    let server_lookups = lookups.clone();
    let backend = f.socket.clone();
    let server = thread::spawn(move || {
        while !server_stop.load(Ordering::SeqCst) {
            let (mut stream, _) = match listener.accept() {
                Ok(pair) => pair,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(1));
                    continue;
                }
                Err(e) => panic!("terminal status proxy: {e}"),
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let request: arc_ipc::Request = arc_ipc::read_message(&mut stream).unwrap();
            let response = if request.operation == arc_ipc::traffic::OPERATION_TRAFFIC_JOB {
                let id = request.payload["job_id"].as_str().unwrap();
                let mut seen = server_lookups.lock().unwrap();
                let first = !seen.iter().any(|prior| prior == id);
                seen.push(id.to_owned());
                if first {
                    arc_ipc::Response::error(&request.request_id, arc_ipc::ERROR_DB_BUSY)
                } else {
                    arc_ipc::exchange(&mut UnixStream::connect(&backend).unwrap(), &request)
                        .unwrap()
                }
            } else {
                assert_eq!(request.operation, arc_ipc::jobs::OPERATION_JOB_STATUS);
                arc_ipc::exchange(&mut UnixStream::connect(&backend).unwrap(), &request).unwrap()
            };
            let _ = arc_ipc::write_response(&mut stream, &response);
        }
    });
    let output = Command::new(binary("arcctl"))
        .args(["job", "status"])
        .env(arc_core::ARC_SOCKET_PATH, &path)
        .output()
        .unwrap();
    stop.store(true, Ordering::SeqCst);
    server.join().unwrap();
    assert!(output.status.success(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    for (id, state) in ids.iter().zip([
        "succeeded",
        "failed",
        "terminated by signal",
        "spawn failed",
    ]) {
        let block = text
            .split("Job: ")
            .find(|block| block.starts_with(id.as_str()))
            .unwrap_or_else(|| panic!("missing {id}: {text}"));
        assert!(block.contains(&format!("State: {state}\n")), "{block}");
    }
    assert_eq!(
        *lookups.lock().unwrap(),
        vec![ids[3].clone(), ids[2].clone()]
    );
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn unavailable_traffic_does_not_hide_recorded_terminal_outcomes() {
    use arc_ipc::jobs::{Outcome, SpawnReason};
    let _serial = support::serial();
    let f = Fixture::new();
    let ids = seed_lifecycle_records(
        &f,
        &[
            Some(Outcome::Exited { code: 0 }),
            Some(Outcome::Exited { code: 17 }),
            Some(Outcome::Signaled {
                signal: 15,
                core_dumped: false,
            }),
            Some(Outcome::SpawnFailed {
                reason: SpawnReason::NotFound,
                os_code: Some(2),
            }),
            None,
        ],
    );
    let mut daemon = f.spawn("test");
    let path = f.root.join("traffic-unavailable.sock");
    let listener = UnixListener::bind(&path).unwrap();
    listener.set_nonblocking(true).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let failures = Arc::new(AtomicUsize::new(0));
    let server_stop = stop.clone();
    let server_failures = failures.clone();
    let backend = f.socket.clone();
    let server = thread::spawn(move || {
        while !server_stop.load(Ordering::SeqCst) {
            let (mut stream, _) = match listener.accept() {
                Ok(pair) => pair,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(1));
                    continue;
                }
                Err(e) => panic!("unavailable Traffic proxy: {e}"),
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let request: arc_ipc::Request = arc_ipc::read_message(&mut stream).unwrap();
            let response = if request.operation == arc_ipc::traffic::OPERATION_TRAFFIC_JOB {
                server_failures.fetch_add(1, Ordering::SeqCst);
                arc_ipc::Response::error(&request.request_id, arc_ipc::ERROR_PROTOCOL_INCOMPATIBLE)
            } else {
                assert_eq!(request.operation, arc_ipc::jobs::OPERATION_JOB_STATUS);
                arc_ipc::exchange(&mut UnixStream::connect(&backend).unwrap(), &request).unwrap()
            };
            let _ = arc_ipc::write_response(&mut stream, &response);
        }
    });
    let outputs: Vec<_> = ids
        .iter()
        .map(|id| {
            Command::new(binary("arcctl"))
                .args(["job", "status"])
                .arg(id)
                .env(arc_core::ARC_SOCKET_PATH, &path)
                .output()
                .unwrap()
        })
        .collect();
    stop.store(true, Ordering::SeqCst);
    server.join().unwrap();
    assert_eq!(failures.load(Ordering::SeqCst), ids.len());
    for (output, (state, detail)) in outputs.iter().zip([
        ("succeeded", Some("Exit code: 0")),
        ("failed", Some("Exit code: 17")),
        ("terminated by signal", Some("Signal: 15")),
        ("spawn failed", Some("Reason: SPAWN_FAILED")),
        ("unknown", None),
    ]) {
        assert!(output.status.success(), "{output:?}");
        let text = String::from_utf8_lossy(&output.stdout);
        assert!(text.contains(&format!("State: {state}\n")), "{text}");
        assert!(text.contains("Scheduling: status unavailable"), "{text}");
        if let Some(detail) = detail {
            assert!(text.contains(detail), "{text}");
        } else {
            assert!(!text.contains("Exit code:"), "{text}");
        }
    }
    daemon.stop(libc::SIGTERM);
    f.report();
}

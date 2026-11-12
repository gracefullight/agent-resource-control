//! Real CLI, real SQLite worker, and deterministic dropped Traffic replies.
use super::*;
use crate::{
    db_worker::{DbConfig, DbWorker},
    limits::RuntimeLimits,
    runtime_metrics::RuntimeMetrics,
};
use arc_ipc::{
    traffic::{
        JobRequest, OPERATION_BUN_CAPABILITIES, OPERATION_BUN_JOB, OPERATION_TRAFFIC_CAPABILITIES,
        OPERATION_TRAFFIC_JOB,
    },
    Request, Response,
};
use std::{
    fs,
    num::NonZeroUsize,
    os::unix::{
        fs::{symlink, DirBuilderExt},
        net::UnixListener,
    },
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

#[derive(Clone, Copy)]
enum Boundary {
    Enqueue,
    Attach,
    Complete,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn real_cli_executes_once_when_replies_are_lost_before_and_after_commits() {
    for program in ["cargo", "bun"] {
        for boundary in [Boundary::Enqueue, Boundary::Attach, Boundary::Complete] {
            for after_commit in [false, true] {
                exercise(program, boundary, after_commit).await;
            }
        }
    }
}

async fn exercise(program: &'static str, boundary: Boundary, after_commit: bool) {
    let root = arc_core::runtime_root().unwrap().join(format!(
        "af-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
    let database = root.join("db");
    let socket = root.join("sock");
    let connection = arc_storage::open_and_migrate(&database).unwrap();
    let limits = RuntimeLimits::default();
    let mut worker = DbWorker::start(
        connection,
        DbConfig {
            queue_capacity: NonZeroUsize::new(8).unwrap(),
            busy_timeout: Duration::from_millis(30),
            execution_timeout: Duration::from_secs(1),
        },
        Arc::new(RuntimeMetrics::new(&limits)),
    )
    .unwrap();
    let client = worker.client();
    let listener = UnixListener::bind(&socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let injected = Arc::new(AtomicBool::new(false));
    let attempts = Arc::new(AtomicUsize::new(0));
    let server_stop = stop.clone();
    let server_injected = injected.clone();
    let server_attempts = attempts.clone();
    let runtime = tokio::runtime::Handle::current();
    let server = thread::spawn(move || {
        while !server_stop.load(Ordering::SeqCst) {
            let (mut stream, _) = match listener.accept() {
                Ok(pair) => pair,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(1));
                    continue;
                }
                Err(error) => panic!("fixture accept: {error}"),
            };
            stream.set_nonblocking(false).unwrap();
            let peer = crate::platform::peer_pid(&stream).unwrap();
            let request: Request = arc_ipc::read_message(&mut stream).unwrap();
            let (probe, operation, capability) = if program == "bun" {
                (
                    OPERATION_BUN_CAPABILITIES,
                    OPERATION_BUN_JOB,
                    arc_ipc::traffic::Capabilities::bun_supported(),
                )
            } else {
                (
                    OPERATION_TRAFFIC_CAPABILITIES,
                    OPERATION_TRAFFIC_JOB,
                    arc_ipc::traffic::Capabilities::supported(),
                )
            };
            if request.operation == probe {
                arc_ipc::write_response(
                    &mut stream,
                    &Response::ok(
                        request.request_id,
                        serde_json::to_value(capability).unwrap(),
                    ),
                )
                .unwrap();
                continue;
            }
            // This frozen Traffic peer predates the additive Meter operation.
            // Keep all dropped-reply/one-execution assertions below intact.
            if matches!(request.operation.as_str(), "meter.report" | "meter.summary") {
                arc_ipc::write_response(
                    &mut stream,
                    &Response::error(request.request_id, arc_ipc::ERROR_UNSUPPORTED_CAPABILITY),
                )
                .unwrap();
                continue;
            }
            assert_eq!(request.operation, operation);
            let job = arc_ipc::traffic::decode_job_for(operation, &request.payload).unwrap();
            let selected = matches!(
                (&boundary, &job),
                (Boundary::Enqueue, JobRequest::Enqueue { .. })
                    | (Boundary::Attach, JobRequest::Attach { .. })
                    | (Boundary::Complete, JobRequest::Complete { .. })
            );
            if selected {
                server_attempts.fetch_add(1, Ordering::SeqCst);
            }
            let fault = selected && !server_injected.swap(true, Ordering::SeqCst);
            if fault && !after_commit {
                continue;
            }
            let result = runtime.block_on(handle(
                client.clone(),
                job,
                peer,
                Instant::now() + Duration::from_secs(2),
            ));
            if fault && after_commit {
                assert!(result.is_ok(), "commit must precede the dropped reply");
                continue;
            }
            let response = match result {
                Ok(value) => Response::ok(request.request_id, value),
                Err(code) => Response::error(request.request_id, code),
            };
            let _ = arc_ipc::write_response(&mut stream, &response);
        }
    });
    let bin = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    symlink(bin.join("arc-exec-fixture"), root.join(program)).unwrap();
    let marker = root.join("calls");
    let mut cli = Command::new(bin.join("arcctl"))
        .args([
            "exec",
            "--",
            program,
            if program == "bun" { "test" } else { "build" },
        ])
        .env(arc_core::ARC_SOCKET_PATH, &socket)
        .env("PATH", &root)
        .env("ARC_FIXTURE_MODE", "echo")
        .env("ARC_FIXTURE_MARKER", &marker)
        .env_remove("ARC_TRAFFIC_LINEAGE")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = cli.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = cli.kill();
            let _ = cli.wait();
            panic!("CLI fault case timed out");
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    };
    stop.store(true, Ordering::SeqCst);
    server.join().unwrap();
    assert!(injected.load(Ordering::SeqCst));
    assert!(
        attempts.load(Ordering::SeqCst) >= 2,
        "CLI did not retry the lost reply"
    );
    assert_eq!(status.code(), Some(17));
    assert_eq!(fs::read_to_string(marker).unwrap().lines().count(), 1);
    let db = rusqlite::Connection::open(&database).unwrap();
    let counts: (i64, i64, i64) = db.query_row(
        "SELECT (SELECT count(*) FROM permits), (SELECT count(*) FROM permits WHERE state='ACTIVE'), (SELECT count(*) FROM job_lifecycle WHERE outcome_kind='EXITED')",
        [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    ).unwrap();
    assert_eq!(counts, (1, 0, 1));
    drop(db);
    worker
        .shutdown(tokio::time::Instant::now() + Duration::from_secs(2))
        .await
        .unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lost_grant_ack_then_cancel_recovers_unspawned_reservation_after_reopen() {
    for program in ["cargo", "bun"] {
        cancel_after_lost_grant(program).await;
    }
}

async fn cancel_after_lost_grant(program: &'static str) {
    let root = arc_core::runtime_root().unwrap().join(format!(
        "ac-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
    let database = root.join("db");
    let socket = root.join("sock");
    let connection = arc_storage::open_and_migrate(&database).unwrap();
    let limits = RuntimeLimits::default();
    let mut worker = DbWorker::start(
        connection,
        DbConfig {
            queue_capacity: NonZeroUsize::new(8).unwrap(),
            busy_timeout: Duration::from_millis(30),
            execution_timeout: Duration::from_secs(1),
        },
        Arc::new(RuntimeMetrics::new(&limits)),
    )
    .unwrap();
    let client = worker.client();
    let listener = UnixListener::bind(&socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    let runtime = tokio::runtime::Handle::current();
    let server = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let (mut stream, _) = match listener.accept() {
                Ok(pair) => pair,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "grant fixture timed out");
                    thread::sleep(Duration::from_millis(1));
                    continue;
                }
                Err(error) => panic!("grant fixture accept: {error}"),
            };
            stream.set_nonblocking(false).unwrap();
            let peer = crate::platform::peer_pid(&stream).unwrap();
            let request: Request = arc_ipc::read_message(&mut stream).unwrap();
            let (probe, operation, capability) = if program == "bun" {
                (
                    OPERATION_BUN_CAPABILITIES,
                    OPERATION_BUN_JOB,
                    arc_ipc::traffic::Capabilities::bun_supported(),
                )
            } else {
                (
                    OPERATION_TRAFFIC_CAPABILITIES,
                    OPERATION_TRAFFIC_JOB,
                    arc_ipc::traffic::Capabilities::supported(),
                )
            };
            if request.operation == probe {
                arc_ipc::write_response(
                    &mut stream,
                    &Response::ok(
                        request.request_id,
                        serde_json::to_value(capability).unwrap(),
                    ),
                )
                .unwrap();
                continue;
            }
            assert_eq!(request.operation, operation);
            let job = arc_ipc::traffic::decode_job_for(operation, &request.payload).unwrap();
            assert!(matches!(job, JobRequest::Enqueue { .. }));
            let reply = runtime
                .block_on(handle(
                    client.clone(),
                    job,
                    peer,
                    Instant::now() + Duration::from_secs(2),
                ))
                .unwrap();
            assert_eq!(reply["state"], "GRANTED");
            // The grant committed; signal cancellation and drop its reply, then
            // close the server so the cancellation acknowledgement is also lost.
            assert_eq!(unsafe { libc::kill(peer as i32, libc::SIGTERM) }, 0);
            break;
        }
    });
    let bin = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    symlink(bin.join("arc-exec-fixture"), root.join(program)).unwrap();
    let marker = root.join("calls");
    let mut cli = Command::new(bin.join("arcctl"))
        .args([
            "exec",
            "--",
            program,
            if program == "bun" { "test" } else { "build" },
        ])
        .env(arc_core::ARC_SOCKET_PATH, &socket)
        .env("PATH", &root)
        .env("ARC_FIXTURE_MODE", "echo")
        .env("ARC_FIXTURE_MARKER", &marker)
        .env_remove("ARC_TRAFFIC_LINEAGE")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = cli.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = cli.kill();
            let _ = cli.wait();
            panic!("cancel fixture timed out");
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    };
    server.join().unwrap();
    assert!(!status.success());
    assert!(!marker.exists(), "cancelled CLI must not spawn the target");
    worker
        .shutdown(tokio::time::Instant::now() + Duration::from_secs(2))
        .await
        .unwrap();

    // Reopen the durable database as on restart. A no-spawn receipt may release
    // only the matching dead owner's unattached grant.
    let mut db = arc_storage::open_and_migrate(&database).unwrap();
    let candidates = traffic::recovery_candidates(&db, 0, 64).unwrap();
    assert_eq!(candidates.len(), 1);
    let candidate = &candidates[0];
    let receipt = super::witness::read_at(&socket, candidate).expect("no-spawn witness");
    assert!(matches!(receipt.proof(), super::witness::Proof::Unspawned));
    let evidence = traffic::UnspawnedEvidence::gather(&candidate.job, receipt.key(), jobs::now());
    drop(receipt);
    super::recovery::recover_at_start_on(&mut db, &socket).unwrap();
    assert!(!traffic::recover_unspawned(&mut db, &evidence).unwrap());
    assert_eq!(
        traffic::status(&db, &candidate.job.job_id).unwrap(),
        arc_ipc::traffic::StatusResult::Cancelled {}
    );
    assert!(!root
        .join(format!(
            ".traffic-witness-{}",
            candidate.job.job_id.as_str()
        ))
        .exists());
    let (total, active): (i64, i64) = db
        .query_row(
            "SELECT count(*),coalesce(sum(state='ACTIVE'),0) FROM permits",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!((total, active), (1, 0));
    drop(db);
    fs::remove_dir_all(root).unwrap();
}

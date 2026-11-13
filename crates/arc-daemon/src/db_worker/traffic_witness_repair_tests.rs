//! Exercise lost acknowledgements against a real CLI and durable SQLite worker.
use super::*;
use crate::{
    db_worker::{DbConfig, DbWorker},
    limits::RuntimeLimits,
    runtime_metrics::RuntimeMetrics,
};
use arc_ipc::{
    jobs::{JobId, Outcome, SpawnReason},
    traffic::{JobRequest, JobResult, OPERATION_TRAFFIC_CAPABILITIES, OPERATION_TRAFFIC_JOB},
    Request, Response,
};
use std::{
    fs,
    num::NonZeroUsize,
    os::unix::{fs::DirBuilderExt, net::UnixListener},
    process::{Command, Stdio},
    sync::Arc,
    thread,
    time::Duration,
};

#[derive(Clone, Copy)]
enum FailurePoint {
    BeforeSpawn,
    SpawnFailure,
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn owner_killed_after_grant_before_spawn_releases_from_initial_witness() {
    exercise(FailurePoint::BeforeSpawn).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failed_spawn_with_lost_completion_is_recovered_without_replay() {
    exercise(FailurePoint::SpawnFailure).await;
}

async fn exercise(point: FailurePoint) {
    let root = arc_core::runtime_root().unwrap().join(format!(
        "aw-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
    let database = root.join("db");
    let socket = root.join("sock");
    let limits = RuntimeLimits::default();
    let mut worker = DbWorker::start(
        arc_storage::open_and_migrate(&database).unwrap(),
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
    let runtime = tokio::runtime::Handle::current();
    let server = thread::spawn(move || {
        loop {
            let (mut stream, _) = listener.accept().unwrap();
            let peer = crate::platform::peer_pid(&stream).unwrap();
            let request: Request = arc_ipc::read_message(&mut stream).unwrap();
            if request.operation == arc_ipc::governor::OPERATION_CAPABILITIES {
                arc_ipc::write_response(
                    &mut stream,
                    &Response::error(request.request_id, arc_ipc::ERROR_UNSUPPORTED_CAPABILITY),
                )
                .unwrap();
                continue;
            }
            if super::launch_fixture::dispatch(&runtime, &client, &stream, &request) {
                continue;
            }
            if request.operation == OPERATION_TRAFFIC_CAPABILITIES {
                arc_ipc::write_response(
                    &mut stream,
                    &Response::ok(
                        &request.request_id,
                        serde_json::to_value(arc_ipc::traffic::Capabilities::supported()).unwrap(),
                    ),
                )
                .unwrap();
                continue;
            }
            assert_eq!(request.operation, OPERATION_TRAFFIC_JOB);
            let job = arc_ipc::traffic::decode_job(&request.payload).unwrap();
            if matches!(
                (point, &job),
                (FailurePoint::SpawnFailure, JobRequest::Complete { .. })
            ) {
                break; // Completion was sent but deliberately not committed or acknowledged.
            }
            let result = runtime
                .block_on(handle(
                    client.clone(),
                    job.clone(),
                    peer,
                    Instant::now() + Duration::from_secs(2),
                ))
                .unwrap();
            if matches!(
                (point, &job),
                (FailurePoint::BeforeSpawn, JobRequest::Enqueue { .. })
            ) {
                assert_eq!(result["state"], "GRANTED");
                assert_eq!(unsafe { libc::kill(peer as i32, libc::SIGKILL) }, 0);
                break;
            }
            arc_ipc::write_response(&mut stream, &Response::ok(&request.request_id, result))
                .unwrap();
        }
    });
    let bin = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let marker = root.join("should-not-exist");
    let mut cli = Command::new(bin.join("arcctl"))
        .args(["exec", "--", "cargo", "build"])
        .env(arc_core::ARC_SOCKET_PATH, &socket)
        .env("PATH", &root)
        .env("ARC_FIXTURE_MARKER", &marker)
        .env_remove("ARC_TRAFFIC_LINEAGE")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if cli.try_wait().unwrap().is_some() {
            break;
        }
        if Instant::now() >= deadline {
            let _ = cli.kill();
            let _ = cli.wait();
            panic!("witness fixture timed out");
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    server.join().unwrap();
    assert!(!marker.exists());
    worker
        .shutdown(tokio::time::Instant::now() + Duration::from_secs(2))
        .await
        .unwrap();
    let mut db = arc_storage::open_and_migrate(&database).unwrap();
    let candidates = traffic::recovery_candidates(&db, 0, 64).unwrap();
    assert_eq!(candidates.len(), 1);
    let candidate = &candidates[0];
    let receipt = super::witness::read_at(&socket, candidate).expect("durable owner-bound witness");
    match (point, receipt.proof()) {
        (FailurePoint::BeforeSpawn, super::witness::Proof::Unspawned) => (),
        (
            FailurePoint::SpawnFailure,
            super::witness::Proof::SpawnFailed(Outcome::SpawnFailed {
                reason: SpawnReason::NotFound,
                ..
            }),
        ) => (),
        _ => panic!("unexpected witness outcome"),
    }
    drop(receipt);
    let path = root.join(format!(
        ".traffic-witness-{}",
        candidate.job.job_id.as_str()
    ));
    let current = fs::read(&path).unwrap();
    assert_eq!(&current[104..], candidate.job.job_id.as_str().as_bytes());
    let mut wrong_job = current.clone();
    wrong_job[104] = if wrong_job[104] == b'a' { b'b' } else { b'a' };
    fs::write(&path, &wrong_job).unwrap();
    assert!(super::witness::read_at(&socket, candidate).is_none());
    if matches!(point, FailurePoint::BeforeSpawn) {
        let mut old = vec![0u8; 88];
        old[..8].copy_from_slice(b"ARCNOS01");
        old[24..].copy_from_slice(&current[40..104]);
        fs::write(&path, &old).unwrap();
        assert!(matches!(
            super::witness::read_at(&socket, candidate).unwrap().proof(),
            super::witness::Proof::Unspawned
        ));
        let mut version_two = current[..104].to_vec();
        version_two[..8].copy_from_slice(b"ARCWIT02");
        fs::write(&path, &version_two).unwrap();
        assert!(matches!(
            super::witness::read_at(&socket, candidate).unwrap().proof(),
            super::witness::Proof::Unspawned
        ));
    }
    fs::write(&path, current).unwrap();
    super::recovery::recover_at_start_on(&mut db, &socket).unwrap();
    let result = traffic::status(&db, &candidate.job.job_id).unwrap();
    match point {
        FailurePoint::BeforeSpawn => assert!(matches!(
            result,
            arc_ipc::traffic::StatusResult::Cancelled {}
        )),
        FailurePoint::SpawnFailure => assert!(matches!(
            result,
            arc_ipc::traffic::StatusResult::Completed {
                outcome: Outcome::SpawnFailed {
                    reason: SpawnReason::NotFound,
                    ..
                },
                ..
            }
        )),
    }
    let (total, active): (i64, i64) = db
        .query_row(
            "SELECT count(*),coalesce(sum(state='ACTIVE'),0) FROM permits",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!((total, active), (1, 0));
    let owner = ProcessIdentity::capture(std::process::id()).unwrap();
    let id = JobId::try_from("ee23456789abcdef0123456789abcdef".to_owned()).unwrap();
    let classification = serde_json::from_value(serde_json::json!({"classification":"BUILD","locality":"HOST_LOCAL","reason":"CARGO_BUILD","uncertainty":"NONE"})).unwrap();
    let request = JobRequest::Enqueue {
        job_id: id,
        observer: owner.clone(),
        classification,
        lineage_context: None,
    };
    let context = LineageContext::try_from("a".repeat(64)).unwrap();
    let next = traffic::enqueue(
        &mut db,
        &request,
        &traffic::OwnerEvidence::gather(&owner),
        &arc_traffic::Policy::default(),
        &context,
        None,
        jobs::now(),
    )
    .unwrap();
    assert!(matches!(next, JobResult::Granted { .. }));
    drop(db);
    fs::remove_dir_all(root).unwrap();
}

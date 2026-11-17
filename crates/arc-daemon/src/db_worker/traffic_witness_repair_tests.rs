//! Exercise lost acknowledgements against a real CLI and durable SQLite worker.
use super::*;
use crate::native_fixture::OwnedChild;
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
    fs::{self, File, OpenOptions},
    io,
    num::NonZeroUsize,
    os::unix::{
        fs::{DirBuilderExt, PermissionsExt},
        net::UnixListener,
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Arc, OnceLock},
    thread,
    time::Duration,
};

#[derive(Clone, Copy)]
enum FailurePoint {
    BeforeSpawn,
    SpawnFailure,
}

// Cargo can relink target/debug/arcctl while another test binary is running.
// Open it once, then hard-link the immutable copy into each case's own root.
static CLI_FIXTURE: OnceLock<PathBuf> = OnceLock::new();
extern "C" fn remove_cli_fixture() {
    if let Some(path) = CLI_FIXTURE.get() {
        if let Some(directory) = path.parent() {
            if let Err(error) = fs::remove_dir_all(directory) {
                if error.kind() != io::ErrorKind::NotFound {
                    eprintln!(
                        "cannot remove owned CLI fixture {}: {error}",
                        directory.display()
                    );
                    unsafe { libc::_exit(1) };
                }
            }
        }
    }
}

fn immutable_cli() -> &'static Path {
    CLI_FIXTURE
        .get_or_init(|| {
            let binary = std::env::current_exe()
                .unwrap()
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("arcctl");
            let directory = arc_core::runtime_root().unwrap().join(format!(
                "arc-witness-cli-{}-{}",
                std::process::id(),
                arc_core::process_identity::random_job_id().unwrap()
            ));
            fs::DirBuilder::new()
                .mode(0o700)
                .create(&directory)
                .unwrap();
            let destination = directory.join("arcctl");
            snapshot_cli(&binary, &destination, || {});
            assert_eq!(unsafe { libc::atexit(remove_cli_fixture) }, 0);
            destination
        })
        .as_path()
}

fn snapshot_cli(source_path: &Path, destination: &Path, mut missing: impl FnMut()) {
    let end = Instant::now() + Duration::from_secs(60);
    let mut source = loop {
        match File::open(source_path) {
            Ok(source) => break source,
            Err(error) if error.kind() == io::ErrorKind::NotFound && Instant::now() < end => {
                missing();
                thread::yield_now();
            }
            Err(error) => panic!(
                "required arcctl fixture {} unavailable: {error}",
                source_path.display()
            ),
        }
    };
    let mut target = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .unwrap();
    io::copy(&mut source, &mut target).unwrap();
    fs::set_permissions(destination, source.metadata().unwrap().permissions()).unwrap();
}

#[test]
fn cli_fixture_waits_for_publication_and_survives_replacement() {
    use std::sync::mpsc;
    let root = arc_core::runtime_root().unwrap().join(format!(
        "arc-witness-copy-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
    let source = root.join("arcctl");
    let destination = root.join("copy");
    let (ready, received) = mpsc::sync_channel(1);
    let copy_source = source.clone();
    let copy_destination = destination.clone();
    let worker = thread::spawn(move || {
        let mut notified = false;
        snapshot_cli(&copy_source, &copy_destination, || {
            if !notified {
                ready.send(()).unwrap();
                notified = true;
            }
        });
    });
    received
        .recv_timeout(Duration::from_secs(60))
        .expect("CLI copy never observed missing source");
    let pending = root.join("pending");
    fs::write(&pending, b"original").unwrap();
    fs::set_permissions(&pending, fs::Permissions::from_mode(0o700)).unwrap();
    fs::rename(&pending, &source).unwrap();
    worker.join().unwrap();
    fs::write(&pending, b"replacement").unwrap();
    fs::rename(&pending, &source).unwrap();
    assert_eq!(fs::read(&destination).unwrap(), b"original");
    assert_eq!(
        fs::metadata(&destination).unwrap().permissions().mode() & 0o777,
        0o700
    );
    fs::remove_dir_all(root).unwrap();
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
    let cli_binary = root.join("arcctl");
    fs::hard_link(immutable_cli(), &cli_binary).unwrap();
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
    let marker = root.join("should-not-exist");
    let mut cli = OwnedChild::new(
        Command::new(cli_binary)
            .args(["exec", "--", "cargo", "build"])
            .env(arc_core::ARC_SOCKET_PATH, &socket)
            .env("PATH", &root)
            .env("ARC_FIXTURE_MARKER", &marker)
            .env_remove("ARC_TRAFFIC_LINEAGE")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    tokio::task::block_in_place(|| cli.wait_with_output())
        .expect("witness fixture CLI never exited or could not be reaped");
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
    // A concurrent fork can inherit a locked descriptor even when it is
    // close-on-exec. Give each synthetic historical record its own inode so
    // an unrelated child cannot retain the preceding fixture's read lock.
    let replace = |bytes: &[u8]| {
        let next = root.join("witness-replacement");
        fs::write(&next, bytes).unwrap();
        fs::set_permissions(&next, fs::Permissions::from_mode(0o600)).unwrap();
        fs::rename(next, &path).unwrap();
    };
    let mut wrong_job = current.clone();
    wrong_job[104] = if wrong_job[104] == b'a' { b'b' } else { b'a' };
    replace(&wrong_job);
    assert!(super::witness::read_at(&socket, candidate).is_none());
    if matches!(point, FailurePoint::BeforeSpawn) {
        let mut old = vec![0u8; 88];
        old[..8].copy_from_slice(b"ARCNOS01");
        old[24..].copy_from_slice(&current[40..104]);
        replace(&old);
        assert!(matches!(
            super::witness::read_at(&socket, candidate).unwrap().proof(),
            super::witness::Proof::Unspawned
        ));
        let mut version_two = current[..104].to_vec();
        version_two[..8].copy_from_slice(b"ARCWIT02");
        replace(&version_two);
        assert!(matches!(
            super::witness::read_at(&socket, candidate).unwrap().proof(),
            super::witness::Proof::Unspawned
        ));
    }
    replace(&current);
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

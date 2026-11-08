use super::{call, handle, Operation, Value};
use crate::{
    db_worker::{start_with_test_hooks, DbConfig, DbError, WorkerTestHooks},
    limits::RuntimeLimits,
    runtime_metrics::{Gauge, RuntimeMetrics},
};
use arc_ipc::jobs::{JobId, LifecycleRequest, ProcessIdentity, StartRequest, StatusRequest};
use std::{
    num::NonZeroUsize,
    sync::{mpsc, Arc},
    time::{Duration, Instant},
};
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lifecycle_cancellation_before_commit_releases_shared_capacity_and_never_launches() {
    let root = arc_core::runtime_root().unwrap().join(format!(
        "arc-job-worker-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    let db = root.join("arc.sqlite3");
    let connection = arc_storage::open_and_migrate(&db).unwrap();
    let limits = RuntimeLimits::default();
    let metrics = Arc::new(RuntimeMetrics::new(&limits));
    let (started, observed) = mpsc::sync_channel(1);
    let (_release, receiver) = mpsc::sync_channel(1);
    let mut worker = start_with_test_hooks(
        connection,
        DbConfig {
            queue_capacity: NonZeroUsize::new(2).unwrap(),
            busy_timeout: Duration::from_millis(30),
            execution_timeout: Duration::from_secs(1),
        },
        metrics.clone(),
        WorkerTestHooks {
            started,
            release: receiver,
        },
    )
    .unwrap();
    let client = worker.client();
    let deadline = Instant::now() + Duration::from_secs(3);
    let request = StartRequest::Register {
        job_id: JobId::try_from("00000000000000000000000000000001".to_string()).unwrap(),
        observer: ProcessIdentity::capture(std::process::id()).unwrap(),
        classification: arc_ipc::Classification {
            classification: arc_ipc::JobClass::Unknown,
            locality: arc_ipc::Locality::Unknown,
            reason: arc_ipc::Reason::UnresolvedCommand,
            uncertainty: arc_ipc::Uncertainty::Observe,
        },
    };
    let evidence = arc_storage::jobs::StartEvidence::gather(&request);
    let ticket = client
        .try_job(
            deadline,
            Operation::Start(request.clone(), evidence, super::now()),
        )
        .unwrap();
    observed.recv_timeout(Duration::from_secs(1)).unwrap();
    drop(ticket);
    metrics
        .wait_for(|s| s.gauge(Gauge::DbOutstanding).current == 0)
        .await;
    let Value::Get(result) = call(&client, Operation::Get(request.job_id().clone()), deadline)
        .await
        .unwrap()
    else {
        panic!()
    };
    assert!(result.unwrap().is_none());
    let result = handle(
        client.clone(),
        LifecycleRequest::Start(request.clone()),
        deadline,
    )
    .await
    .unwrap();
    assert_eq!(result["disposition"], "CREATED");
    assert_eq!(
        handle(
            client.clone(),
            LifecycleRequest::Start(request.clone()),
            deadline
        )
        .await
        .unwrap()["disposition"],
        "UNCHANGED"
    );
    let status = handle(
        client.clone(),
        LifecycleRequest::Status(StatusRequest::ById {
            job_id: request.job_id().clone(),
        }),
        deadline,
    )
    .await
    .unwrap();
    assert_eq!(status["job"]["phase"], "REGISTERED");
    worker.begin_shutdown();
    assert!(matches!(
        client.try_job(deadline, Operation::Recent),
        Err(DbError::Stopping)
    ));
    worker
        .shutdown(tokio::time::Instant::now() + Duration::from_secs(1))
        .await
        .unwrap();
    for gauge in [Gauge::DbQueued, Gauge::DbExecuting, Gauge::DbOutstanding] {
        assert_eq!(metrics.snapshot().gauge(gauge).current, 0);
    }
    let connection = arc_storage::open_and_migrate(&db).unwrap();
    assert_eq!(
        connection
            .query_row("SELECT count(*) FROM job_lifecycle", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    drop(connection);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn worker_records_native_observation_time_instead_of_queue_execution_time() {
    let root = arc_core::runtime_root().unwrap().join(format!(
        "arc-job-clock-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    let mut connection = arc_storage::open_and_migrate(root.join("arc.sqlite3")).unwrap();
    let request=StartRequest::Register{job_id:JobId::try_from("00000000000000000000000000000002".to_string()).unwrap(),observer:ProcessIdentity::capture(std::process::id()).unwrap(),classification:serde_json::from_value(serde_json::json!({"classification":"UNKNOWN","locality":"UNKNOWN","reason":"UNRESOLVED_COMMAND","uncertainty":"OBSERVE"})).unwrap()};
    let evidence = arc_storage::jobs::StartEvidence::gather(&request);
    // Fixed caller-clock fixture: changing worker execution to stamp current
    // wall time would falsely move the prior native observation by one second.
    let captured = super::now() - 1000;
    let Value::Start(result) = super::execute(
        &mut connection,
        Operation::Start(request, evidence, captured),
    ) else {
        panic!()
    };
    let job = result.unwrap().job;
    assert_eq!(job.observer_verified_at_ms, captured);
    assert_eq!(job.observation.observed_at_ms, captured);
    assert_eq!(job.registered_at_ms, captured);
    connection.close().unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

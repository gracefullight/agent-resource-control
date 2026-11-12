use super::*;
use arc_ipc::{
    policy::launch::LaunchAck,
    traffic::{JobRequest, JobResult},
};
use arc_policy::{DecisionReason, ObservationAvailability, PolicyMode, PolicyReason};
use std::{
    sync::{mpsc, Arc},
    time::Duration,
};
#[path = "policy_fixture.rs"]
mod fixture;
use fixture::Fixture;
#[path = "policy_clock_tests.rs"]
mod clocks;
#[path = "policy_repair_tests.rs"]
mod repairs;
fn generation(result: JobResult) -> u64 {
    match result {
        JobResult::Granted {
            permit_generation, ..
        } => permit_generation,
        _ => panic!("expected grant {result:?}"),
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn production_writer_admission_launch_and_reads_use_published_pressure() {
    let f = Fixture::new(None).await;
    let first = generation(f.enqueue(1).await);
    f.level(1).await;
    let request = LaunchRequest {
        job_id: f.id(1),
        observer: f.owner.clone(),
        permit_generation: first,
    };
    assert!(matches!(
        f.policy(Input::Launch(request.clone())).await,
        Value::Launch {
            result: LaunchResult::Held { .. },
            fence: None
        }
    ));
    assert!(matches!(f.enqueue(2).await, JobResult::Queued { .. }));
    f.traffic(JobRequest::Complete {
        job_id: f.id(1),
        observer: f.owner.clone(),
        permit_generation: first,
        outcome: arc_ipc::jobs::Outcome::Exited { code: 17 },
    })
    .await;
    for _ in 0..10 {
        assert!(matches!(f.poll(2).await, JobResult::Queued { .. }));
    }
    match f.policy(Input::Job(f.id(2))).await {
        Value::Job(read) => {
            assert_eq!(read.effective.mode, PolicyMode::MemoryHold);
            assert_eq!(
                read.last_decision.unwrap().reason,
                DecisionReason::MemoryPressure
            );
        }
        _ => panic!("expected bounded job read"),
    }
    let c =
        rusqlite::Connection::open_with_flags(f.path(), rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    assert_eq!(
        c.query_row("SELECT count(*) FROM permits", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        arc_storage::jobs::get(&c, &f.id(1))
            .unwrap()
            .unwrap()
            .outcome,
        Some(arc_ipc::jobs::Outcome::Exited { code: 17 })
    );
    drop(c);
    f.level(0).await;
    let second = generation(f.poll(2).await);
    let request = LaunchRequest {
        job_id: f.id(2),
        observer: f.owner.clone(),
        permit_generation: second,
    };
    let ready = f.policy(Input::Launch(request.clone())).await;
    assert!(matches!(
        ready,
        Value::Launch {
            result: LaunchResult::Ready { .. },
            fence: Some(_)
        }
    ));
    drop(ready);
    assert!(matches!(
        handle(
            f.client.clone(),
            Input::Launch(request),
            f.owner.pid,
            Instant::now() + Duration::from_secs(2)
        )
        .await,
        Err("INVALID_JOB_TRANSITION")
    ));
    f.shutdown().await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn published_failure_selects_l0_and_peer_mismatch_is_rejected() {
    let f = Fixture::new(None).await;
    f.level(1).await;
    assert!(matches!(f.enqueue(1).await, JobResult::Queued { .. }));
    f.level(2).await;
    match f.policy(Input::Current).await {
        Value::Current(effective) => {
            assert_eq!(effective.mode, PolicyMode::L0);
            assert_eq!(effective.reason, PolicyReason::ObservationFailed);
            assert_eq!(
                effective.observation.availability,
                ObservationAvailability::Failed
            );
            assert_eq!(effective.observation.pressure, None);
        }
        _ => panic!("expected current policy"),
    }
    assert!(matches!(f.poll(1).await, JobResult::Granted { .. }));
    let request = LaunchRequest {
        job_id: f.id(1),
        observer: f.owner.clone(),
        permit_generation: 1,
    };
    assert!(matches!(
        handle(
            f.client.clone(),
            Input::Launch(request),
            f.owner.pid + 1,
            Instant::now() + Duration::from_secs(2)
        )
        .await,
        Err(arc_ipc::ERROR_INVALID_REQUEST)
    ));
    f.shutdown().await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn writer_backlog_reads_the_new_publication_and_native_sampling_continues() {
    let (started, entered) = mpsc::sync_channel(1);
    let (release, allowed) = mpsc::sync_channel(1);
    let f = Fixture::new(Some(crate::db_worker::WorkerTestHooks {
        started,
        release: allowed,
    }))
    .await;
    let blocker = f
        .client
        .try_doctor(Instant::now() + Duration::from_secs(10))
        .unwrap();
    entered.recv_timeout(Duration::from_secs(2)).unwrap();
    let client = f.client.clone();
    let owner = f.owner.clone();
    let id = f.id(1);
    let pending = tokio::spawn(async move {
        traffic::handle(client,JobRequest::Enqueue {job_id:id,observer:owner.clone(),classification:serde_json::from_value(serde_json::json!({"classification":"BUILD","locality":"HOST_LOCAL","reason":"CARGO_BUILD","uncertainty":"NONE"})).unwrap(),lineage_context:None},owner.pid,Instant::now()+Duration::from_secs(10)).await.unwrap()
    });
    f.level(1).await;
    release.send(()).unwrap();
    blocker.receive().await.unwrap();
    let result: JobResult = serde_json::from_value(pending.await.unwrap()).unwrap();
    assert!(matches!(result, JobResult::Queued { .. }));
    match f.policy(Input::Current).await {
        Value::Current(policy) => assert_eq!(policy.mode, PolicyMode::MemoryHold),
        _ => panic!("expected policy"),
    }
    f.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn same_connection_launch_ack_releases_fence_and_policy_reads_remain_compatible() {
    let f = Fixture::new(None).await;
    let generation = generation(f.enqueue(1).await);
    let (server, client) = tokio::net::UnixStream::pair().unwrap();
    let limits = crate::limits::RuntimeLimits::default();
    let metrics = Arc::new(crate::runtime_metrics::RuntimeMetrics::new(&limits));
    let requests = crate::admission::Requests::new(&limits, Arc::clone(&metrics)).unwrap();
    let db = f.client.clone();
    let handler: crate::connection::policy_dispatch::Handler =
        Arc::new(move |input, peer, deadline| Box::pin(handle(db.clone(), input, peer, deadline)));
    let task = tokio::spawn(crate::connection::handle_with_policy(
        server,
        limits,
        requests,
        metrics,
        |_| async { panic!("no diagnostic query") },
        |_, _| async { Err(arc_ipc::ERROR_UNSUPPORTED_CAPABILITY) },
        (
            |_, _, _| async { Err(arc_ipc::ERROR_UNSUPPORTED_CAPABILITY) },
            Some(f.meter.reader()),
            None,
        ),
        Some(handler),
    ));
    let launch = LaunchRequest {
        job_id: f.id(1),
        observer: f.owner.clone(),
        permit_generation: generation,
    };
    let mut request =
        arc_ipc::Request::new("release", arc_ipc::policy::launch::OPERATION_TRAFFIC_LAUNCH);
    request.payload = serde_json::json!(launch);
    let response = crate::transport_test_support::request(&client, &request).await;
    assert!(matches!(
        arc_ipc::policy::launch::decode_launch_result(response.result.as_ref().unwrap()).unwrap(),
        LaunchResult::Ready { .. }
    ));
    request = arc_ipc::Request::new("ack", arc_ipc::policy::launch::OPERATION_TRAFFIC_LAUNCH_ACK);
    request.payload = serde_json::json!(LaunchAck::Unspawned {
        job_id: f.id(1),
        observer: f.owner.clone(),
        permit_generation: generation
    });
    let ack = crate::transport_test_support::request(&client, &request).await;
    assert_eq!(ack.status, arc_ipc::STATUS_OK);
    let request = arc_ipc::policy::current_request("current");
    let response = crate::transport_test_support::request(&client, &request).await;
    assert!(arc_ipc::policy::interpret_current(&request, &response)
        .unwrap()
        .is_valid());
    drop(client);
    task.await.unwrap();
    f.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn disconnect_releases_publication_fence_and_durable_marker_prevents_release_replay() {
    let mut f = Fixture::new(None).await;
    let generation = generation(f.enqueue(1).await);
    let (server, client) = tokio::net::UnixStream::pair().unwrap();
    let limits = crate::limits::RuntimeLimits::default();
    let metrics = Arc::new(crate::runtime_metrics::RuntimeMetrics::new(&limits));
    let requests = crate::admission::Requests::new(&limits, Arc::clone(&metrics)).unwrap();
    let db = f.client.clone();
    let handler: crate::connection::policy_dispatch::Handler =
        Arc::new(move |input, peer, deadline| Box::pin(handle(db.clone(), input, peer, deadline)));
    let task = tokio::spawn(crate::connection::handle_with_policy(
        server,
        limits,
        requests,
        metrics,
        |_| async { panic!("no diagnostic query") },
        |_, _| async { Err(arc_ipc::ERROR_UNSUPPORTED_CAPABILITY) },
        (
            |_, _, _| async { Err(arc_ipc::ERROR_UNSUPPORTED_CAPABILITY) },
            Some(f.meter.reader()),
            None,
        ),
        Some(handler),
    ));
    let launch = LaunchRequest {
        job_id: f.id(1),
        observer: f.owner.clone(),
        permit_generation: generation,
    };
    let mut request =
        arc_ipc::Request::new("lost", arc_ipc::policy::launch::OPERATION_TRAFFIC_LAUNCH);
    request.payload = serde_json::json!(launch);
    let response = crate::transport_test_support::request(&client, &request).await;
    assert!(matches!(
        arc_ipc::policy::launch::decode_launch_result(response.result.as_ref().unwrap()).unwrap(),
        LaunchResult::Ready { .. }
    ));
    drop(client);
    tokio::time::timeout(Duration::from_secs(1), task)
        .await
        .unwrap()
        .unwrap();
    // A new worker/connection observes the same committed one-shot marker.
    f.worker
        .shutdown(tokio::time::Instant::now() + Duration::from_secs(2))
        .await
        .unwrap();
    let c =
        rusqlite::Connection::open_with_flags(f.path(), rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    let count: u32 = c.query_row("SELECT count(*) FROM audit_events WHERE event_type='TRAFFIC_LAUNCH_FENCE' AND entity_id=?1", [f.id(1).as_str()], |r| r.get(0)).unwrap();
    assert_eq!(count, 1);
    drop(c);
    let connection = arc_storage::open_and_migrate(f.path()).unwrap();
    let other = crate::db_worker::DbWorker::start(
        connection,
        crate::db_worker::DbConfig {
            queue_capacity: std::num::NonZeroUsize::new(8).unwrap(),
            busy_timeout: Duration::from_millis(50),
            execution_timeout: Duration::from_secs(1),
        },
        Arc::new(crate::runtime_metrics::RuntimeMetrics::new(
            &crate::limits::RuntimeLimits::default(),
        )),
    )
    .unwrap();
    other.client().bind_observation(f.meter.reader());
    assert!(matches!(
        handle(
            other.client(),
            Input::Launch(launch),
            f.owner.pid,
            Instant::now() + Duration::from_secs(2)
        )
        .await,
        Err("INVALID_JOB_TRANSITION")
    ));
    f.worker = other;
    f.client = f.worker.client();
    f.level(1).await;
    assert!(matches!(f.enqueue(2).await, JobResult::Queued { .. }));
    f.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn critical_preserves_native_verified_nested_reservation_but_not_a_claim_string() {
    struct Child(std::process::Child);
    impl Drop for Child {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }
    let f = Fixture::new(None).await;
    let root = f.enqueue(1).await;
    let JobResult::Granted {
        permit_generation,
        lineage_context,
        ..
    } = root
    else {
        panic!("expected root");
    };
    let child = Child(
        std::process::Command::new("/bin/sleep")
            .arg("30")
            .spawn()
            .unwrap(),
    );
    let nested_owner = arc_core::process_identity::ProcessIdentity::capture(child.0.id()).unwrap();
    assert!(traffic::is_descendant(&nested_owner, &f.owner));
    f.level(1).await;
    let classification=serde_json::from_value(serde_json::json!({"classification":"BUILD","locality":"HOST_LOCAL","reason":"CARGO_BUILD","uncertainty":"NONE"})).unwrap();
    let request = JobRequest::Enqueue {
        job_id: f.id(2),
        observer: nested_owner.clone(),
        classification,
        lineage_context: Some(lineage_context.clone()),
    };
    let nested: JobResult = serde_json::from_value(
        traffic::handle(
            f.client.clone(),
            request,
            nested_owner.pid,
            Instant::now() + Duration::from_secs(3),
        )
        .await
        .unwrap(),
    )
    .unwrap();
    assert!(matches!(nested, JobResult::Reused { .. }));
    let release = handle(
        f.client.clone(),
        Input::Launch(LaunchRequest {
            job_id: f.id(2),
            observer: nested_owner.clone(),
            permit_generation,
        }),
        nested_owner.pid,
        Instant::now() + Duration::from_secs(3),
    )
    .await
    .unwrap();
    assert!(matches!(
        release,
        Value::Launch {
            result: LaunchResult::Reused { .. },
            fence: None
        }
    ));
    // The root observer is not its own descendant. A supplied secret alone
    // cannot turn another root job into verified nested execution.
    let forged = JobRequest::Enqueue {
        job_id: f.id(3),
        observer: f.owner.clone(),
        classification,
        lineage_context: Some(lineage_context),
    };
    assert!(matches!(f.traffic(forged).await, JobResult::Queued { .. }));
    let c =
        rusqlite::Connection::open_with_flags(f.path(), rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    assert_eq!(
        c.query_row("SELECT count(*) FROM permits", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        1
    );
    drop(c);
    drop(child);
    f.shutdown().await;
}

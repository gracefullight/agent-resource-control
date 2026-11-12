use super::*;
use crate::db_worker::{DbConfig, DbWorker};

async fn restart(f: &mut Fixture, bind: bool) {
    f.worker = DbWorker::start(
        arc_storage::open_and_migrate(f.path()).unwrap(),
        DbConfig {
            queue_capacity: std::num::NonZeroUsize::new(8).unwrap(),
            busy_timeout: Duration::from_millis(50),
            execution_timeout: Duration::from_secs(2),
        },
        Arc::new(crate::runtime_metrics::RuntimeMetrics::new(
            &crate::limits::RuntimeLimits::default(),
        )),
    )
    .unwrap();
    f.client = f.worker.client();
    if bind {
        f.client.bind_observation(f.meter.reader());
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn queued_recovery_status_and_cancellation_survive_utc_backstep_across_restart() {
    let mut f = Fixture::new(None).await;
    f.level(1).await;
    f.worker
        .shutdown(tokio::time::Instant::now() + Duration::from_secs(2))
        .await
        .unwrap();
    let future = jobs::now() + 86_400_000;
    let mut c = arc_storage::open_and_migrate(f.path()).unwrap();
    // Persist the state before a simulated one-day UTC backstep. The real
    // daemon's subsequent SystemTime is below this stored lifecycle timestamp.
    for n in [1, 2] {
        let result = repository::enqueue_observed(
            &mut c,
            &JobRequest::Enqueue {
                job_id: f.id(n),
                observer: f.owner.clone(),
                lineage_context: None,
                classification: serde_json::from_value(serde_json::json!({
                    "classification":"BUILD","locality":"HOST_LOCAL",
                    "reason":"CARGO_BUILD","uncertainty":"NONE"}))
                .unwrap(),
            },
            &repository::OwnerEvidence::gather(&f.owner),
            &arc_traffic::Policy::default(),
            &arc_ipc::traffic::LineageContext::try_from(format!("{n:064x}")).unwrap(),
            None,
            future,
            &f.meter.reader(),
        )
        .unwrap();
        assert!(matches!(result, JobResult::Queued { .. }));
    }
    drop(c);
    restart(&mut f, true).await;
    f.level(0).await;
    let generation = generation(f.poll(1).await);
    let request = JobRequest::StatusV2 { job_id: f.id(1) };
    let status = traffic::handle(
        f.client.clone(),
        request,
        f.owner.pid,
        Instant::now() + Duration::from_secs(2),
    )
    .await
    .unwrap();
    assert!(matches!(
        serde_json::from_value::<arc_ipc::traffic::StatusResult>(status).unwrap(),
        arc_ipc::traffic::StatusResult::Unconfirmed {}
    ));
    assert!(matches!(
        f.traffic(JobRequest::Cancel {
            job_id: f.id(2),
            observer: f.owner.clone(),
            permit_generation: None,
        })
        .await,
        JobResult::Cancelled {}
    ));
    f.traffic(JobRequest::Complete {
        job_id: f.id(1),
        observer: f.owner.clone(),
        permit_generation: generation,
        outcome: arc_ipc::jobs::Outcome::Exited { code: 17 },
    })
    .await;
    let c =
        Connection::open_with_flags(f.path(), rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
    let job = arc_storage::jobs::get(&c, &f.id(1)).unwrap().unwrap();
    assert!(job.is_valid());
    assert_eq!(job.completed_at_ms, Some(future));
    assert_eq!(
        job.outcome,
        Some(arc_ipc::jobs::Outcome::Exited { code: 17 })
    );
    drop(c);
    f.shutdown().await;
}

#[tokio::test]
async fn daemon_without_bound_sampler_reports_missing_l0_and_can_release_once() {
    let mut f = Fixture::new(None).await;
    f.worker
        .shutdown(tokio::time::Instant::now() + Duration::from_secs(2))
        .await
        .unwrap();
    restart(&mut f, false).await;
    let generation = generation(f.enqueue(1).await);
    let release = f
        .policy(Input::Launch(LaunchRequest {
            job_id: f.id(1),
            observer: f.owner.clone(),
            permit_generation: generation,
        }))
        .await;
    match &release {
        Value::Launch {
            result: LaunchResult::Ready { policy, .. },
            fence: Some(_),
        } => {
            assert_eq!(policy.mode, PolicyMode::L0);
            assert_eq!(policy.reason, PolicyReason::ObservationMissing);
            assert_eq!(policy.observation.pressure, None);
        }
        _ => panic!("missing sampler must report bounded explicit L0"),
    }
    let replacement = super::repairs::critical_sampler(&f).await;
    let reader = replacement.reader();
    let client = f.client.clone();
    let (done, completed) = mpsc::sync_channel(1);
    let binding = std::thread::spawn(move || {
        client.bind_observation(reader);
        done.send(()).unwrap();
    });
    let end = Instant::now() + Duration::from_secs(1);
    while f.client.shared.observation.0.try_lock().is_ok() {
        assert!(Instant::now() < end, "first sampler entered registry fence");
        std::thread::yield_now();
    }
    assert!(matches!(
        completed.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    drop(release);
    completed.recv_timeout(Duration::from_secs(2)).unwrap();
    binding.join().unwrap();
    match f.policy(Input::Current).await {
        Value::Current(value) => assert_eq!(value.mode, PolicyMode::MemoryHold),
        _ => panic!("expected newly bound Critical sampler"),
    }
    f.shutdown().await;
    drop(replacement);
}

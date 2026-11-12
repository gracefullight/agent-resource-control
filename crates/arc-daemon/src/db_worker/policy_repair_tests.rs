//! Synthetic pressure/clock regressions, separate from native smoke evidence.
use super::*;
use crate::db_worker::{execution::Hooks, jobs::Operation as JobOperation, DbConfig, DbWorker};
use arc_core::meter::MemoryPressure;
use arc_policy::ObservationProvider;

pub(super) async fn critical_sampler(f: &Fixture) -> arc_meter::MeterWorker {
    let sampler = f.sampler(1);
    let end = Instant::now() + Duration::from_secs(2);
    loop {
        if sampler.reader().snapshot().is_some_and(|s| {
            s.host.memory_pressure.reading.value() == Some(&MemoryPressure::Critical)
        }) {
            return sampler;
        }
        assert!(Instant::now() < end, "replacement publication deadline");
        tokio::task::yield_now().await;
    }
}
fn launch_request(f: &Fixture, permit_generation: u64) -> LaunchRequest {
    LaunchRequest {
        job_id: f.id(1),
        observer: f.owner.clone(),
        permit_generation,
    }
}
fn operation(f: &Fixture, generation: u64) -> Operation {
    Operation::Launch {
        request: launch_request(f, generation),
        owner: repository::OwnerEvidence::gather(&f.owner),
        proof: None,
    }
}
fn marker_count(c: &Connection) -> i64 {
    c.query_row(
        "SELECT count(*) FROM audit_events WHERE event_type='TRAFFIC_LAUNCH_FENCE'",
        [],
        |r| r.get(0),
    )
    .unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn replacement_during_sqlite_backlog_fences_enqueue_and_reserved_launch() {
    for launch in [false, true] {
        let mut f = Fixture::new(None).await;
        let generation = if launch {
            generation(f.enqueue(1).await)
        } else {
            0
        };
        f.worker
            .shutdown(tokio::time::Instant::now() + Duration::from_secs(2))
            .await
            .unwrap();
        let connection = arc_storage::open_and_migrate(f.path()).unwrap();
        let (blocked, observed) = mpsc::sync_channel(1);
        f.worker = DbWorker::start_inner(
            connection,
            DbConfig {
                queue_capacity: std::num::NonZeroUsize::new(8).unwrap(),
                busy_timeout: Duration::from_secs(5),
                execution_timeout: Duration::from_secs(10),
            },
            Arc::new(crate::runtime_metrics::RuntimeMetrics::new(
                &crate::limits::RuntimeLimits::default(),
            )),
            Hooks {
                lock_probe: Some(blocked),
                ..Hooks::default()
            },
        )
        .unwrap();
        f.client = f.worker.client();
        let old = f.meter.reader();
        f.client.bind_observation(old.clone());
        let replacement = critical_sampler(&f).await;
        let mut lock = arc_storage::open_and_migrate(f.path()).unwrap();
        let tx = lock
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        let ticket = if launch {
            f.client
                .try_job(
                    Instant::now() + Duration::from_secs(10),
                    JobOperation::Policy(operation(&f, generation)),
                )
                .unwrap()
        } else {
            // Native proofs are gathered before this actual admission request.
            // Reads used for reconciliation do not take the external writer lock.
            let client = f.client.clone();
            let owner = f.owner.clone();
            let id = f.id(1);
            let pending = tokio::spawn(async move {
                traffic::handle(
                    client,
                    JobRequest::Enqueue {
                        job_id: id,
                        observer: owner.clone(),
                        lineage_context: None,
                        classification: serde_json::from_value(serde_json::json!({
                        "classification":"BUILD","locality":"HOST_LOCAL",
                        "reason":"CARGO_BUILD","uncertainty":"NONE"}))
                        .unwrap(),
                    },
                    owner.pid,
                    Instant::now() + Duration::from_secs(10),
                )
                .await
            });
            observed.recv_timeout(Duration::from_secs(2)).unwrap();
            f.client.bind_observation(replacement.reader());
            tx.commit().unwrap();
            let reply: JobResult = serde_json::from_value(pending.await.unwrap().unwrap()).unwrap();
            assert!(matches!(reply, JobResult::Queued { .. }));
            assert_eq!(marker_count(&lock), 0);
            let permits: i64 = lock
                .query_row("SELECT count(*) FROM permits", [], |r| r.get(0))
                .unwrap();
            assert_eq!(permits, 0);
            drop(lock);
            f.shutdown().await;
            drop(replacement);
            continue;
        };
        observed.recv_timeout(Duration::from_secs(2)).unwrap();
        f.client.bind_observation(replacement.reader());
        old.with_observation(&mut |input| {
            assert_eq!(
                arc_policy::evaluator::evaluate(input).reason,
                PolicyReason::ObservationFailed
            );
        });
        tx.commit().unwrap();
        let value = ticket.receive_value().await.unwrap();
        assert!(matches!(value, crate::db_worker::DbValue::Lifecycle(value)
            if matches!(*value, jobs::Value::Policy(Ok(Value::Launch {
                result: LaunchResult::Held { .. }, fence: None })) )));
        assert_eq!(marker_count(&lock), 0);
        let permits: i64 = lock
            .query_row(
                "SELECT count(*) FROM permits WHERE state='ACTIVE'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(permits, 1, "hold preserves the existing reservation");
        drop(lock);
        f.shutdown().await;
        drop(replacement);
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn replacement_waits_for_outstanding_ready_then_uses_new_identity_and_critical() {
    let f = Fixture::new(None).await;
    let generation = generation(f.enqueue(1).await);
    let old = f.meter.reader();
    // Rebinding the same sampler must be idempotent, not retire it.
    f.client.bind_observation(old.clone());
    let ready = f
        .policy(Input::Launch(launch_request(&f, generation)))
        .await;
    assert!(matches!(
        ready,
        Value::Launch {
            result: LaunchResult::Ready { .. },
            fence: Some(_)
        }
    ));
    let replacement = critical_sampler(&f).await;
    let reader = replacement.reader();
    let client = f.client.clone();
    let (done, completed) = mpsc::sync_channel(1);
    let replacing = std::thread::spawn(move || {
        client.bind_observation(reader);
        done.send(()).unwrap();
    });
    let end = Instant::now() + Duration::from_secs(1);
    while f.client.shared.observation.0.try_lock().is_ok() {
        assert!(Instant::now() < end, "replacement entered registry fence");
        std::thread::yield_now();
    }
    assert!(matches!(
        completed.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    drop(ready);
    completed.recv_timeout(Duration::from_secs(2)).unwrap();
    replacing.join().unwrap();
    match f.policy(Input::Current).await {
        Value::Current(value) => assert_eq!(value.mode, PolicyMode::MemoryHold),
        _ => panic!("expected replacement policy"),
    }
    old.with_observation(&mut |input| {
        assert_eq!(
            arc_policy::evaluator::evaluate(input).reason,
            PolicyReason::ObservationFailed
        );
    });
    assert!(matches!(f.enqueue(2).await, JobResult::Queued { .. }));
    f.shutdown().await;
    drop(replacement);
}

#[tokio::test]
async fn expiry_before_release_rolls_back_or_retracts_marker_and_retry_is_exactly_once() {
    for after_commit in [false, true] {
        let mut f = Fixture::new(None).await;
        let generation = generation(f.enqueue(1).await);
        f.worker
            .shutdown(tokio::time::Instant::now() + Duration::from_secs(2))
            .await
            .unwrap();
        let mut c = arc_storage::open_and_migrate(f.path()).unwrap();
        let before = arc_storage::jobs::get(&c, &f.id(1)).unwrap().unwrap();
        let mut checks = 0;
        let result = execute_with_clock(
            &mut c,
            operation(&f, generation),
            Some(&f.client.shared.observation),
            || {
                checks += 1;
                if after_commit && checks == 1 {
                    Instant::now()
                } else {
                    Instant::now() + Duration::from_secs(10)
                }
            },
        );
        assert!(
            matches!(result, Err(E::Busy)),
            "known-undelivered rejection is retryable"
        );
        assert_eq!(marker_count(&c), 0);
        assert_eq!(
            arc_storage::jobs::get(&c, &f.id(1)).unwrap().unwrap(),
            before
        );
        let ready = execute(
            &mut c,
            operation(&f, generation),
            Some(&f.client.shared.observation),
        )
        .unwrap();
        assert!(matches!(
            ready,
            Value::Launch {
                result: LaunchResult::Ready { .. },
                fence: Some(_)
            }
        ));
        drop(ready);
        assert_eq!(marker_count(&c), 1);
        assert!(
            matches!(
                execute(
                    &mut c,
                    operation(&f, generation),
                    Some(&f.client.shared.observation)
                ),
                Err(E::InvalidTransition)
            ),
            "possible delivery cannot be retried"
        );
        drop(c);
        f.shutdown().await;
    }
}

#[tokio::test]
async fn postcommit_cleanup_failure_is_uncertainty_and_keeps_the_release_one_shot() {
    let mut f = Fixture::new(None).await;
    let generation = generation(f.enqueue(1).await);
    f.worker
        .shutdown(tokio::time::Instant::now() + Duration::from_secs(2))
        .await
        .unwrap();
    let mut c = arc_storage::open_and_migrate(f.path()).unwrap();
    c.execute_batch("CREATE TEMP TRIGGER fail_retract BEFORE DELETE ON audit_events
        WHEN OLD.event_type='TRAFFIC_LAUNCH_FENCE' BEGIN SELECT RAISE(ABORT,'injected cleanup failure'); END;").unwrap();
    let mut checks = 0;
    let result = execute_with_clock(
        &mut c,
        operation(&f, generation),
        Some(&f.client.shared.observation),
        || {
            checks += 1;
            Instant::now()
                + if checks == 1 {
                    Duration::ZERO
                } else {
                    Duration::from_secs(10)
                }
        },
    );
    assert!(matches!(result, Err(E::InvalidTransition)));
    assert_eq!(marker_count(&c), 1);
    assert!(matches!(
        execute(
            &mut c,
            operation(&f, generation),
            Some(&f.client.shared.observation)
        ),
        Err(E::InvalidTransition)
    ));
    drop(c);
    f.shutdown().await;
}

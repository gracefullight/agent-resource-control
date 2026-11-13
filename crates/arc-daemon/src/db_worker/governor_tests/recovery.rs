use super::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn governor_off_survives_writer_restart_and_original_receipt_replays() {
    let mut f = Fixture::new(None).await;
    let q = intent(0, false, 1);
    let before = set(&f, q.clone()).await;
    f.worker
        .shutdown(tokio::time::Instant::now() + Duration::from_secs(2))
        .await
        .unwrap();
    f.worker = DbWorker::start(
        arc_storage::open_and_migrate(f.path()).unwrap(),
        DbConfig {
            queue_capacity: std::num::NonZeroUsize::new(8).unwrap(),
            busy_timeout: Duration::from_millis(50),
            execution_timeout: Duration::from_secs(10),
        },
        Arc::new(crate::runtime_metrics::RuntimeMetrics::new(
            &crate::limits::RuntimeLimits::default(),
        )),
    )
    .unwrap();
    f.client = f.worker.client();
    let Value::Get(read) = f
        .governor(Input::Get(GetGovernorSwitchRequest::default()))
        .await
    else {
        panic!()
    };
    assert_eq!(&read.switch, before.switch());
    assert!(matches!(
        set(&f, q).await,
        GovernorMutationResult::Committed { replayed: true, .. }
    ));
    assert!(matches!(
        f.governor(enqueue(&f, 1)).await,
        Value::Job(GovernorJobResult::Unreserved { .. })
    ));
    f.shutdown().await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn governor_common_release_lease_orders_setter_and_grants_without_sampler() {
    let f = Fixture::new(None).await;
    // Remove the sampler to exercise the common lease independently of pressure.
    *f.client.shared.observation.0.lock().unwrap() = None;
    set(&f, intent(0, false, 1)).await;
    f.governor(enqueue(&f, 1)).await;
    let request = GovernorLaunchRequest::Unreserved {
        job_id: f.id(1),
        observer: f.owner.clone(),
    };
    let Value::Launch { result, fence } = f.governor(Input::Launch(request.clone())).await else {
        panic!()
    };
    assert!(matches!(result, GovernorLaunchResult::Unreserved { .. }));
    assert!(matches!(
        handle(
            f.client.clone(),
            Input::Set(intent(1, true, 2)),
            f.owner.pid,
            Instant::now() + Duration::from_secs(2)
        )
        .await,
        Err(arc_ipc::ERROR_DB_BUSY)
    ));
    assert!(matches!(
        handle(
            f.client.clone(),
            enqueue(&f, 2),
            f.owner.pid,
            Instant::now() + Duration::from_secs(2)
        )
        .await,
        Err(arc_ipc::ERROR_DB_BUSY)
    ));
    drop(fence);
    f.governor(Input::Unspawned { request, result }).await;
    assert!(matches!(
        set(&f, intent(1, true, 2)).await,
        GovernorMutationResult::Committed { .. }
    ));
    assert!(matches!(
        poll(&f, 1).await,
        GovernorJobResult::Traffic {
            result: arc_ipc::traffic::JobResult::Granted { .. }
        }
    ));
    f.shutdown().await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn governor_reserved_conversion_and_native_launch_order_are_serialized() {
    let f = Fixture::new(None).await;
    let Value::Job(GovernorJobResult::Traffic {
        result: arc_ipc::traffic::JobResult::Granted {
            permit_generation, ..
        },
    }) = f.governor(enqueue(&f, 1)).await
    else {
        panic!()
    };
    set(&f, intent(0, false, 1)).await;
    assert_eq!(counts(&f), (1, 0));
    let request = GovernorLaunchRequest::Reserved {
        request: arc_ipc::policy::launch::LaunchRequest {
            job_id: f.id(1),
            observer: f.owner.clone(),
            permit_generation,
        },
    };
    let Value::Launch {
        result: result @ GovernorLaunchResult::Unreserved { .. },
        fence,
    } = f.governor(Input::Launch(request.clone())).await
    else {
        panic!("pause won reserved race")
    };
    drop(fence);
    assert!(matches!(
        f.governor(Input::Launch(request.clone())).await,
        Value::Launch {
            result: GovernorLaunchResult::Unconfirmed {},
            ..
        }
    ));
    f.governor(Input::Unspawned { request, result }).await;
    set(&f, intent(1, true, 2)).await;
    assert!(matches!(
        f.governor(Input::Launch(GovernorLaunchRequest::Unreserved {
            job_id: f.id(1),
            observer: f.owner.clone()
        }))
        .await,
        Value::Launch {
            result: GovernorLaunchResult::Retry {},
            ..
        }
    ));
    f.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn governor_reserved_release_winning_pause_retains_permit_and_never_releases_twice() {
    let f = Fixture::new(None).await;
    let Value::Job(GovernorJobResult::Traffic {
        result: arc_ipc::traffic::JobResult::Granted {
            permit_generation, ..
        },
    }) = f.governor(enqueue(&f, 1)).await
    else {
        panic!()
    };
    let request = GovernorLaunchRequest::Reserved {
        request: arc_ipc::policy::launch::LaunchRequest {
            job_id: f.id(1),
            observer: f.owner.clone(),
            permit_generation,
        },
    };
    let Value::Launch {
        result: GovernorLaunchResult::Reserved { result },
        fence,
    } = f.governor(Input::Launch(request.clone())).await
    else {
        panic!()
    };
    assert!(matches!(
        *result,
        arc_ipc::policy::launch::LaunchResult::Ready { .. }
    ));
    assert!(matches!(
        handle(
            f.client.clone(),
            Input::Set(intent(0, false, 1)),
            f.owner.pid,
            Instant::now() + Duration::from_secs(2)
        )
        .await,
        Err(arc_ipc::ERROR_DB_BUSY)
    ));
    drop(fence);
    set(&f, intent(0, false, 1)).await;
    assert_eq!(counts(&f), (1, 1));
    assert!(
        !execution::disposition(&ro(&f), &f.id(1))
            .unwrap()
            .unwrap()
            .unreserved
    );
    assert!(matches!(
        f.governor(Input::Launch(request)).await,
        Value::Launch {
            result: GovernorLaunchResult::Unconfirmed {},
            ..
        }
    ));
    let child = Child::spawn();
    f.traffic(JobRequest::Attach {
        job_id: f.id(1),
        observer: f.owner.clone(),
        child: child.identity(),
        permit_generation,
    })
    .await;
    drop(child);
    let done = JobRequest::Complete {
        job_id: f.id(1),
        observer: f.owner.clone(),
        permit_generation,
        outcome: Outcome::Signaled {
            signal: 9,
            core_dumped: false,
        },
    };
    f.traffic(done.clone()).await;
    f.traffic(done).await;
    assert_eq!(counts(&f), (1, 0));
    f.shutdown().await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn governor_postcommit_deadline_is_uncertain_but_receipt_proves_commit() {
    let mut f = Fixture::new(None).await;
    f.worker
        .shutdown(tokio::time::Instant::now() + Duration::from_secs(2))
        .await
        .unwrap();
    let (hook, entered, release) = gate();
    f.worker = DbWorker::start_inner(
        arc_storage::open_and_migrate(f.path()).unwrap(),
        DbConfig {
            queue_capacity: std::num::NonZeroUsize::new(8).unwrap(),
            busy_timeout: Duration::from_millis(50),
            execution_timeout: Duration::from_secs(10),
        },
        Arc::new(crate::runtime_metrics::RuntimeMetrics::new(
            &crate::limits::RuntimeLimits::default(),
        )),
        crate::db_worker::execution::Hooks {
            after_operation: Some(hook),
            ..Default::default()
        },
    )
    .unwrap();
    f.client = f.worker.client();
    let q = intent(0, false, 1);
    let pending = f
        .client
        .try_job(
            Instant::now() + Duration::from_millis(150),
            jobs::Operation::Governor(Operation::Set(q.clone())),
        )
        .unwrap();
    entered.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(matches!(
        pending.receive_value().await,
        Err(crate::db_worker::DbError::Deadline)
    ));
    assert!(!state::get_switch(&ro(&f)).unwrap().enabled);
    release.send(()).unwrap();
    let Value::Get(read) = f
        .governor(Input::Get(GetGovernorSwitchRequest {
            key: GovernorSwitchKey::GovernorEnabled,
            mutation_id: Some(q.mutation_id),
        }))
        .await
    else {
        panic!()
    };
    assert_eq!(read.switch.revision, 1);
    assert!(read.mutation_result.is_some());
    f.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn governor_cancellation_after_switch_update_before_commit_rolls_back_every_fact() {
    let mut f = Fixture::new(None).await;
    f.worker
        .shutdown(tokio::time::Instant::now() + Duration::from_secs(2))
        .await
        .unwrap();
    let mut connection = arc_storage::open_and_migrate(f.path()).unwrap();
    let (entered, observed) = mpsc::sync_channel(1);
    let (release, allowed) = mpsc::sync_channel(1);
    let mut allowed = Some(allowed);
    connection
        .authorizer(Some(move |context: rusqlite::hooks::AuthContext<'_>| {
            if matches!(
                context.action,
                rusqlite::hooks::AuthAction::Insert {
                    table_name: "governor_mutation_receipts"
                }
            ) {
                if let Some(allowed) = allowed.take() {
                    entered.send(()).unwrap();
                    allowed.recv_timeout(Duration::from_secs(3)).unwrap();
                }
            }
            rusqlite::hooks::Authorization::Allow
        }))
        .unwrap();
    f.worker = DbWorker::start(
        connection,
        DbConfig {
            queue_capacity: std::num::NonZeroUsize::new(8).unwrap(),
            busy_timeout: Duration::from_millis(50),
            execution_timeout: Duration::from_secs(10),
        },
        Arc::new(crate::runtime_metrics::RuntimeMetrics::new(
            &crate::limits::RuntimeLimits::default(),
        )),
    )
    .unwrap();
    f.client = f.worker.client();
    let q = intent(0, false, 1);
    let pending = f
        .client
        .try_job(
            Instant::now() + Duration::from_secs(10),
            jobs::Operation::Governor(Operation::Set(q.clone())),
        )
        .unwrap();
    observed.recv_timeout(Duration::from_secs(2)).unwrap();
    // The switch UPDATE ran; reader sees only committed prior ON state.
    assert!(state::get_switch(&ro(&f)).unwrap().enabled);
    drop(pending);
    release.send(()).unwrap();
    let Value::Get(read) = f
        .governor(Input::Get(GetGovernorSwitchRequest {
            key: GovernorSwitchKey::GovernorEnabled,
            mutation_id: Some(q.mutation_id),
        }))
        .await
    else {
        panic!()
    };
    assert!(read.switch.enabled);
    assert!(read.mutation_result.is_none());
    assert_eq!(read.switch.revision, 0);
    f.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn governor_backlogged_pause_resume_and_enqueue_use_fresh_critical_after_writer_admission() {
    let (hook, entered, release) = gate();
    let f = Fixture::new(Some(hook)).await;
    let before = f.meter.reader().snapshot().unwrap();
    let end = Instant::now() + Duration::from_secs(10);
    let blocker = f.client.try_doctor(end).unwrap();
    entered.recv_timeout(Duration::from_secs(2)).unwrap();
    let pause = f
        .client
        .try_job(
            end,
            jobs::Operation::Governor(Operation::Set(intent(0, false, 71))),
        )
        .unwrap();
    let resume = f
        .client
        .try_job(
            end,
            jobs::Operation::Governor(Operation::Set(intent(1, true, 72))),
        )
        .unwrap();
    let Input::Job(GovernorJobRequest::Traffic { request }) = enqueue(&f, 1) else {
        unreachable!()
    };
    let pending = f
        .client
        .try_job(
            end,
            jobs::Operation::Governor(Operation::Enqueue {
                request,
                owner: repository::OwnerEvidence::gather(&f.owner),
                context: traffic::context().unwrap(),
                proof: None,
            }),
        )
        .unwrap();
    // These tickets entered while the cached observation was Normal. The
    // sampler must continue while SQLite's sole writer is backlogged.
    f.level(1).await;
    let fresh = f.meter.reader().snapshot().unwrap();
    assert!(fresh.sequence > before.sequence);
    assert_eq!(state::get_switch(&ro(&f)).unwrap().revision, 0);
    release.send(()).unwrap();
    blocker.receive().await.unwrap();
    for ticket in [pause, resume] {
        let DbValue::Lifecycle(reply) = ticket.receive_value().await.unwrap() else {
            panic!()
        };
        assert!(matches!(
            *reply,
            jobs::Value::Governor(Ok(Value::Set(GovernorMutationResult::Committed {
                changed: true,
                ..
            })))
        ));
    }
    let DbValue::Lifecycle(reply) = pending.receive_value().await.unwrap() else {
        panic!()
    };
    assert!(matches!(
        *reply,
        jobs::Value::Governor(Ok(Value::Job(GovernorJobResult::Traffic {
            result: arc_ipc::traffic::JobResult::Queued { .. }
        })))
    ));
    let switch = state::get_switch(&ro(&f)).unwrap();
    assert!(switch.enabled);
    assert_eq!(switch.revision, 2);
    assert_eq!(counts(&f), (0, 0));
    // Admission itself, independent of any later read, recorded fresh Critical.
    let reason: String = ro(&f)
        .query_row(
            "SELECT reason_code FROM policy_decisions WHERE job_id=?1 ORDER BY rowid DESC LIMIT 1",
            [f.id(1).as_str()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(reason, "memory_pressure");
    f.level(0).await;
    assert!(matches!(
        poll(&f, 1).await,
        GovernorJobResult::Traffic {
            result: arc_ipc::traffic::JobResult::Granted { .. }
        }
    ));
    f.traffic(JobRequest::Cancel {
        job_id: f.id(1),
        observer: f.owner.clone(),
        permit_generation: None,
    })
    .await;
    assert_eq!(counts(&f), (1, 0));
    eprintln!(
        "governor-backlog-observation={}",
        serde_json::json!({"case":"writer-queued-pause-resume-enqueue","switch":switch,"normalSequenceBeforeWait":before.sequence,"criticalSequenceBeforeRelease":fresh.sequence,"admissionDecision":reason,"permitsWhileCritical":[0,0],"permitsAfterRecoveryAndCancel":counts(&f),"children":0,"source":"explicit injected pressure; real bounded writer tickets"})
    );
    f.shutdown().await;
}

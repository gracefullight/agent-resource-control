use super::super::policy::tests::fixture::Fixture;
use super::*;
use crate::db_worker::{DbConfig, DbValue, DbWorker, WorkerTestHooks};
use arc_ipc::jobs::Outcome;
use std::sync::{mpsc, Arc};
#[path = "governor_repair_tests.rs"]
mod repairs;
trait GovernorFixture {
    async fn governor(&self, input: Input) -> Value;
}
impl GovernorFixture for Fixture {
    async fn governor(&self, input: Input) -> Value {
        handle(
            self.client.clone(),
            input,
            self.owner.pid,
            Instant::now() + Duration::from_secs(5),
        )
        .await
        .unwrap()
    }
}

fn intent(base: u64, enabled: bool, n: u32) -> SetGovernorSwitchRequest {
    SetGovernorSwitchRequest {
        key: GovernorSwitchKey::GovernorEnabled,
        enabled,
        expected_revision: base,
        mutation_id: format!("{base}-{n:032x}"),
    }
}
async fn set(f: &Fixture, q: SetGovernorSwitchRequest) -> GovernorMutationResult {
    match f.governor(Input::Set(q)).await {
        Value::Set(r) => r,
        _ => panic!("setter reply"),
    }
}
fn enqueue(f: &Fixture, n: u32) -> Input {
    Input::Job(GovernorJobRequest::Traffic{request:JobRequest::Enqueue{job_id:f.id(n),observer:f.owner.clone(),classification:serde_json::from_value(serde_json::json!({"classification":"BUILD","locality":"HOST_LOCAL","reason":"CARGO_BUILD","uncertainty":"NONE"})).unwrap(),lineage_context:None}})
}
async fn poll(f: &Fixture, n: u32) -> GovernorJobResult {
    match f
        .governor(Input::Job(GovernorJobRequest::Traffic {
            request: JobRequest::Poll {
                job_id: f.id(n),
                observer: f.owner.clone(),
            },
        }))
        .await
    {
        Value::Job(r) => r,
        _ => panic!("poll"),
    }
}
fn ro(f: &Fixture) -> Connection {
    Connection::open_with_flags(f.path(), rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap()
}
fn counts(f: &Fixture) -> (u32, u32) {
    ro(f)
        .query_row(
            "SELECT count(*),COALESCE(sum(state='ACTIVE'),0) FROM permits",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap()
}
fn gate() -> (WorkerTestHooks, mpsc::Receiver<()>, mpsc::SyncSender<()>) {
    let (started, entered) = mpsc::sync_channel(1);
    let (release, allowed) = mpsc::sync_channel(1);
    (
        WorkerTestHooks {
            started,
            release: allowed,
        },
        entered,
        release,
    )
}
struct Child(std::process::Child);
impl Child {
    fn spawn() -> Self {
        Self(
            std::process::Command::new("/bin/sleep")
                .arg("30")
                .spawn()
                .unwrap(),
        )
    }
    fn finish(mut self) -> Outcome {
        use std::os::unix::process::ExitStatusExt;
        self.0.kill().unwrap();
        let status = self.0.wait().unwrap();
        match status.signal() {
            Some(signal) => Outcome::Signaled {
                signal: signal as u8,
                core_dumped: status.core_dumped(),
            },
            None => Outcome::Exited {
                code: status.code().unwrap() as u8,
            },
        }
    }
    fn identity(&self) -> arc_ipc::jobs::ProcessIdentity {
        arc_ipc::jobs::ProcessIdentity::capture(self.0.id()).unwrap()
    }
}
impl Drop for Child {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn governor_native_running_child_survives_queue_pause_and_critical_bypass() {
    let f = Fixture::new(None).await;
    let Value::Job(GovernorJobResult::Traffic {
        result: arc_ipc::traffic::JobResult::Granted {
            permit_generation, ..
        },
    }) = f.governor(enqueue(&f, 1)).await
    else {
        panic!("A grant")
    };
    let child_a = Child::spawn();
    let a = child_a.identity();
    f.traffic(JobRequest::Attach {
        job_id: f.id(1),
        observer: f.owner.clone(),
        child: a.clone(),
        permit_generation,
    })
    .await;
    assert!(matches!(
        f.governor(enqueue(&f, 2)).await,
        Value::Job(GovernorJobResult::Traffic {
            result: arc_ipc::traffic::JobResult::Queued { .. }
        })
    ));
    assert_eq!(counts(&f), (1, 1));
    let paused = set(&f, intent(0, false, 1)).await;
    assert_eq!(paused.switch().revision, 1);
    assert!(matches!(
        poll(&f, 2).await,
        GovernorJobResult::Unreserved { .. }
    ));
    assert_eq!(
        arc_core::process_identity::probe_live(&a),
        arc_core::process_identity::IdentityStatus::VerifiedMatch
    );
    f.level(1).await;
    assert!(matches!(
        f.governor(enqueue(&f, 3)).await,
        Value::Job(GovernorJobResult::Unreserved { .. })
    ));
    let request = GovernorLaunchRequest::Unreserved {
        job_id: f.id(2),
        observer: f.owner.clone(),
    };
    let Value::Launch {
        result: GovernorLaunchResult::Unreserved { release_id, .. },
        fence,
    } = f.governor(Input::Launch(request.clone())).await
    else {
        panic!("B release")
    };
    drop(fence);
    assert!(matches!(
        f.governor(Input::Launch(request)).await,
        Value::Launch {
            result: GovernorLaunchResult::Unconfirmed {},
            ..
        }
    ));
    let child_b = Child::spawn();
    let b = child_b.identity();
    f.governor(Input::Job(GovernorJobRequest::Attach {
        job_id: f.id(2),
        observer: f.owner.clone(),
        release_id: release_id.clone(),
        child: b.clone(),
    }))
    .await;
    assert_eq!(
        arc_core::process_identity::probe_live(&b),
        arc_core::process_identity::IdentityStatus::VerifiedMatch
    );
    assert!(matches!(
        poll(&f, 2).await,
        GovernorJobResult::Traffic {
            result: arc_ipc::traffic::JobResult::Running {}
        }
    ));
    assert_eq!(counts(&f), (1, 1));
    let outcome_a = child_a.finish();
    let outcome_b = child_b.finish();
    f.traffic(JobRequest::Complete {
        job_id: f.id(1),
        observer: f.owner.clone(),
        permit_generation,
        outcome: outcome_a.clone(),
    })
    .await;
    f.governor(Input::Job(GovernorJobRequest::Complete {
        job_id: f.id(2),
        observer: f.owner.clone(),
        release_id,
        outcome: outcome_b.clone(),
    }))
    .await;
    assert_eq!(counts(&f), (1, 0));
    assert!(matches!(
        set(&f, intent(1, true, 2)).await,
        GovernorMutationResult::Committed { changed: true, .. }
    ));
    assert!(matches!(
        f.governor(enqueue(&f, 4)).await,
        Value::Job(GovernorJobResult::Traffic {
            result: arc_ipc::traffic::JobResult::Queued { .. }
        })
    ));
    assert_eq!(counts(&f), (1, 0));
    f.level(0).await;
    assert!(matches!(
        poll(&f, 3).await,
        GovernorJobResult::Traffic {
            result: arc_ipc::traffic::JobResult::Granted { .. }
        }
    ));
    assert!(matches!(
        poll(&f, 4).await,
        GovernorJobResult::Traffic {
            result: arc_ipc::traffic::JobResult::Queued { .. }
        }
    ));
    for n in [3, 4] {
        f.traffic(JobRequest::Cancel {
            job_id: f.id(n),
            observer: f.owner.clone(),
            permit_generation: None,
        })
        .await;
    }
    assert_eq!(counts(&f), (2, 0));
    let c = ro(&f);
    let switch = state::get_switch(&c).unwrap();
    let children: u32 = c
        .query_row(
            "SELECT count(*) FROM job_lifecycle WHERE child_pid IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(children, 2);
    let release_claims: u32 = c
        .query_row(
            "SELECT count(*) FROM governor_jobs WHERE release_id IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(release_claims, 1);
    eprintln!(
        "governor-native-observation={}",
        serde_json::json!({"scope":"production writer and routing with explicit injected pressure; CLI/registered bridge acceptance is downstream","childIdentities":[a,b],"sqliteChildren":children,"oneShotClaims":release_claims,"pauseRevision":paused.switch().revision,"finalSwitch":switch,"finalPermits":{"total":counts(&f).0,"active":counts(&f).1},"actualOutcomes":[outcome_a,outcome_b]})
    );
    drop(c);
    f.shutdown().await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn governor_concurrent_setters_read_current_cas_after_backlog_and_peer_is_bound() {
    let (hook, entered, release) = gate();
    let f = Fixture::new(Some(hook)).await;
    let blocker = f
        .client
        .try_doctor(Instant::now() + Duration::from_secs(10))
        .unwrap();
    entered.recv_timeout(Duration::from_secs(2)).unwrap();
    let a = f
        .client
        .try_job(
            Instant::now() + Duration::from_secs(10),
            jobs::Operation::Governor(Operation::Set(intent(0, false, 1))),
        )
        .unwrap();
    let b = f
        .client
        .try_job(
            Instant::now() + Duration::from_secs(10),
            jobs::Operation::Governor(Operation::Set(intent(0, false, 2))),
        )
        .unwrap();
    assert!(state::get_switch(&ro(&f)).unwrap().enabled);
    release.send(()).unwrap();
    blocker.receive().await.unwrap();
    let DbValue::Lifecycle(a) = a.receive_value().await.unwrap() else {
        panic!()
    };
    let DbValue::Lifecycle(b) = b.receive_value().await.unwrap() else {
        panic!()
    };
    assert!(matches!(
        *a,
        jobs::Value::Governor(Ok(Value::Set(GovernorMutationResult::Committed { .. })))
    ));
    assert!(matches!(
        *b,
        jobs::Value::Governor(Ok(Value::Set(GovernorMutationResult::Conflict {
            reason: GovernorConflictReason::RevisionConflict,
            ..
        })))
    ));
    assert_eq!(state::get_switch(&ro(&f)).unwrap().revision, 1);
    assert!(matches!(
        handle(
            f.client.clone(),
            enqueue(&f, 1),
            f.owner.pid + 1,
            Instant::now() + Duration::from_secs(2)
        )
        .await,
        Err(arc_ipc::ERROR_INVALID_REQUEST)
    ));
    f.shutdown().await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn governor_cancelled_queued_write_has_no_effect_and_missing_receipt_is_not_a_rejection() {
    let (hook, entered, release) = gate();
    let f = Fixture::new(Some(hook)).await;
    let blocker = f
        .client
        .try_doctor(Instant::now() + Duration::from_secs(10))
        .unwrap();
    entered.recv_timeout(Duration::from_secs(2)).unwrap();
    let q = intent(0, false, 1);
    let pending = f
        .client
        .try_job(
            Instant::now() + Duration::from_secs(10),
            jobs::Operation::Governor(Operation::Set(q.clone())),
        )
        .unwrap();
    drop(pending);
    release.send(()).unwrap();
    blocker.receive().await.unwrap();
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
    assert_eq!(read.switch.revision, 0);
    assert!(read.mutation_result.is_none());
    f.shutdown().await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn governor_commit_ack_is_not_published_early_and_postcommit_cancellation_reads_back_receipt()
{
    let mut f = Fixture::new(None).await;
    f.worker
        .shutdown(tokio::time::Instant::now() + Duration::from_secs(2))
        .await
        .unwrap();
    let (hook, entered, release) = gate();
    let worker = DbWorker::start_inner(
        arc_storage::open_and_migrate(f.path()).unwrap(),
        DbConfig {
            queue_capacity: std::num::NonZeroUsize::new(8).unwrap(),
            busy_timeout: Duration::from_millis(50),
            execution_timeout: Duration::from_secs(10),
        },
        Arc::new(crate::runtime_metrics::RuntimeMetrics::new(
            &crate::limits::RuntimeLimits::default(),
        )),
        super::super::execution::Hooks {
            after_operation: Some(hook),
            ..Default::default()
        },
    )
    .unwrap();
    f.worker = worker;
    f.client = f.worker.client();
    let q = intent(0, false, 1);
    let mut pending = f
        .client
        .try_job(
            Instant::now() + Duration::from_secs(10),
            jobs::Operation::Governor(Operation::Set(q.clone())),
        )
        .unwrap();
    entered.recv_timeout(Duration::from_secs(2)).unwrap();
    // Operation returned after COMMIT but the writer hasn't sent an ACK.
    assert!(!state::get_switch(&ro(&f)).unwrap().enabled);
    assert!(matches!(
        pending.reply.try_recv(),
        Err(tokio::sync::oneshot::error::TryRecvError::Empty)
    ));
    drop(pending);
    release.send(()).unwrap();
    let Value::Get(read) = f
        .governor(Input::Get(GetGovernorSwitchRequest {
            key: GovernorSwitchKey::GovernorEnabled,
            mutation_id: Some(q.mutation_id.clone()),
        }))
        .await
    else {
        panic!()
    };
    assert_eq!(read.switch.revision, 1);
    assert!(matches!(
        read.mutation_result,
        Some(GovernorMutationResult::Committed { replayed: true, .. })
    ));
    assert!(matches!(
        set(&f, q).await,
        GovernorMutationResult::Committed {
            changed: true,
            replayed: true,
            ..
        }
    ));
    f.shutdown().await;
}
#[path = "governor_tests/recovery.rs"]
mod recovery;

#[path = "governor_transport_tests.rs"]
mod transport;

#[path = "governor_tests/scripts.rs"]
mod scripts;

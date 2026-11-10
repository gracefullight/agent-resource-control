use super::*;
use crate::limits::RuntimeLimits;
use std::{
    fs,
    path::PathBuf,
    sync::{atomic::AtomicU64, mpsc as sync_mpsc},
};

struct Database(PathBuf);

impl Database {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "arc-worker-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root.join("arc.sqlite3"))
    }

    fn open(&self) -> Connection {
        arc_storage::open_and_migrate(&self.0).unwrap()
    }

    fn reopen(&self) {
        let connection = self.open();
        assert_eq!(
            connection
                .query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
                .unwrap(),
            "ok"
        );
        assert_eq!(
            connection
                .query_row("SELECT count(*) FROM schema_migrations", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            3
        );
        connection.close().unwrap();
    }
}

impl Drop for Database {
    fn drop(&mut self) {
        fs::remove_dir_all(self.0.parent().unwrap()).unwrap();
    }
}

fn config() -> DbConfig {
    DbConfig {
        queue_capacity: NonZeroUsize::new(2).unwrap(),
        busy_timeout: Duration::from_millis(30),
        execution_timeout: Duration::from_secs(1),
    }
}

fn metrics(config: &DbConfig) -> Arc<RuntimeMetrics> {
    let limits = RuntimeLimits {
        db_queue: config.queue_capacity,
        db_busy_timeout: config.busy_timeout,
        db_execution_timeout: config.execution_timeout,
        ..RuntimeLimits::default()
    };
    limits.validate().unwrap();
    Arc::new(RuntimeMetrics::new(&limits))
}

fn assert_bounds(metrics: &RuntimeMetrics) {
    let snapshot = metrics.snapshot();
    for (gauge, cap) in [
        (Gauge::DbQueued, snapshot.db_queue_cap),
        (Gauge::DbExecuting, 1),
        (Gauge::DbOutstanding, snapshot.db_queue_cap + 1),
    ] {
        let value = snapshot.gauge(gauge);
        assert!(value.current <= cap, "{gauge:?} current: {value:?}");
        assert!(value.high_water <= cap, "{gauge:?} high water: {value:?}");
    }
}
fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(3)
}

fn gate() -> (
    WorkerTestHooks,
    sync_mpsc::Receiver<()>,
    sync_mpsc::SyncSender<()>,
) {
    let (started, observed) = sync_mpsc::sync_channel(1);
    let (release, wait) = sync_mpsc::sync_channel(1);
    (
        WorkerTestHooks {
            started,
            release: wait,
        },
        observed,
        release,
    )
}

async fn stop(worker: &mut DbWorker, database: &Database) {
    let began = Instant::now();
    let exit = worker
        .shutdown(tokio::time::Instant::now() + Duration::from_millis(1_500))
        .await
        .unwrap();
    assert!(exit.worker_joined && exit.connection_closed);
    assert!(began.elapsed() < Duration::from_millis(1_500));
    assert!(worker.thread.is_none());
    let metrics = &worker.client.metrics;
    assert_bounds(metrics);
    let snapshot = metrics.snapshot();
    for gauge in Gauge::ALL {
        assert_eq!(snapshot.gauge(gauge).current, 0, "{gauge:?} retained");
    }
    assert_eq!(snapshot.tracked_tasks.current, 0);
    assert_eq!(snapshot.counter(Counter::WorkerStarted), 1);
    assert_eq!(snapshot.counter(Counter::WorkerJoined), 1);
    database.reopen();
    eprintln!("worker-accounting: {}", snapshot.json());
    eprintln!(
        "worker-lifecycle: joined=true closed=true reopen_integrity=ok elapsed_us={}",
        began.elapsed().as_micros()
    );
}

async fn recover(client: &DbClient) -> DoctorReport {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            match client.try_doctor(deadline()) {
                Ok(ticket) => return ticket.receive().await.unwrap(),
                Err(DbError::Busy) => tokio::task::yield_now().await,
                Err(error) => panic!("recovery admission: {error}"),
            }
        }
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn finite_queue_rejects_100000_attempts_and_recovers_after_cancellation() {
    let database = Database::new();
    let (hooks, started, _release) = gate();
    let metrics = metrics(&config());
    let mut worker =
        start_with_test_hooks(database.open(), config(), Arc::clone(&metrics), hooks).unwrap();
    let client = worker.client();
    let active = client.try_doctor(deadline()).unwrap();
    started.recv_timeout(Duration::from_secs(1)).unwrap();
    let queued_one = client.try_doctor(deadline()).unwrap();
    let queued_two = client.try_doctor(deadline()).unwrap();
    let began = Instant::now();
    for _ in 0..100_000 {
        assert!(matches!(client.try_doctor(deadline()), Err(DbError::Busy)));
    }
    assert_bounds(&metrics);
    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.gauge(Gauge::DbQueued).current, 2);
    assert_eq!(snapshot.gauge(Gauge::DbExecuting).current, 1);
    assert_eq!(snapshot.gauge(Gauge::DbOutstanding).current, 3);
    assert_eq!(snapshot.counter(Counter::DbRejected), 100_000);
    eprintln!(
        "worker-load: attempts=100000 rejected=100000 queued=2 executing=1 elapsed_us={}",
        began.elapsed().as_micros()
    );
    assert!(
        began.elapsed() < Duration::from_secs(2),
        "full admission must not wait"
    );
    drop(queued_one);
    drop(queued_two);
    // Cancellation doesn't free an occupied queue slot behind executing SQL.
    assert!(matches!(client.try_doctor(deadline()), Err(DbError::Busy)));
    drop(active);
    assert_eq!(recover(&client).await.integrity_check, "ok");
    stop(&mut worker, &database).await;
    assert!(matches!(
        client.try_doctor(deadline()),
        Err(DbError::Stopping)
    ));
}

#[tokio::test]
async fn stopped_worker_discards_full_queue_with_live_client_clones() {
    let database = Database::new();
    let (hooks, started, _release) = gate();
    let mut worker =
        start_with_test_hooks(database.open(), config(), metrics(&config()), hooks).unwrap();
    let client = worker.client();
    let active = client.try_doctor(deadline()).unwrap();
    started.recv_timeout(Duration::from_secs(1)).unwrap();
    let one = client.try_doctor(deadline()).unwrap();
    let two = client.try_doctor(deadline()).unwrap();
    stop(&mut worker, &database).await;
    for ticket in [active, one, two] {
        assert!(matches!(ticket.receive().await, Err(DbError::Stopping)));
    }
}

#[tokio::test]
async fn queued_deadline_is_absolute_and_expired_job_is_not_started() {
    let database = Database::new();
    let (hooks, started, release) = gate();
    let mut worker =
        start_with_test_hooks(database.open(), config(), metrics(&config()), hooks).unwrap();
    let client = worker.client();
    let active = client.try_doctor(deadline()).unwrap();
    started.recv_timeout(Duration::from_secs(1)).unwrap();
    let expires = Instant::now() + Duration::from_millis(30);
    let expired = client.try_doctor(expires).unwrap();
    tokio::time::sleep_until(tokio::time::Instant::from_std(expires)).await;
    assert!(matches!(expired.receive().await, Err(DbError::Deadline)));
    release.send(()).unwrap();
    assert_eq!(active.receive().await.unwrap().integrity_check, "ok");
    assert_eq!(recover(&client).await.schema_migrations, 3);
    assert!(matches!(
        client.try_doctor(Instant::now()),
        Err(DbError::Deadline)
    ));
    stop(&mut worker, &database).await;
}

#[tokio::test]
async fn active_sql_is_interrupted_and_worker_joins() {
    let database = Database::new();
    let (progress, observed) = sync_mpsc::sync_channel(1);
    let hooks = execution::Hooks {
        expensive: Some(progress),
        ..Default::default()
    };
    let mut worker =
        DbWorker::start_inner(database.open(), config(), metrics(&config()), hooks).unwrap();
    let ticket = worker.client().try_doctor(deadline()).unwrap();
    // Notification comes from SQLite's VM progress callback, not before SQL.
    observed.recv_timeout(Duration::from_secs(1)).unwrap();
    stop(&mut worker, &database).await;
    assert!(matches!(ticket.receive().await, Err(DbError::Stopping)));
}

#[tokio::test]
async fn dropping_ticket_interrupts_active_sql_without_stopping_worker() {
    let database = Database::new();
    let (progress, observed) = sync_mpsc::sync_channel(1);
    let hooks = execution::Hooks {
        expensive: Some(progress),
        ..Default::default()
    };
    let mut worker =
        DbWorker::start_inner(database.open(), config(), metrics(&config()), hooks).unwrap();
    let ticket = worker.client().try_doctor(deadline()).unwrap();
    observed.recv_timeout(Duration::from_secs(1)).unwrap();
    drop(ticket);
    assert_eq!(recover(&worker.client()).await.integrity_check, "ok");
    stop(&mut worker, &database).await;
}

#[tokio::test]
async fn execution_deadline_interrupts_sql_before_response_deadline() {
    let database = Database::new();
    let (progress, observed) = sync_mpsc::sync_channel(1);
    let hooks = execution::Hooks {
        expensive: Some(progress),
        ..Default::default()
    };
    let mut limits = config();
    limits.execution_timeout = Duration::from_millis(80);
    let metrics = metrics(&limits);
    let mut worker = DbWorker::start_inner(database.open(), limits, metrics, hooks).unwrap();
    let ticket = worker.client().try_doctor(deadline()).unwrap();
    observed.recv_timeout(Duration::from_secs(1)).unwrap();
    assert!(matches!(ticket.receive().await, Err(DbError::Deadline)));
    assert_eq!(recover(&worker.client()).await.integrity_check, "ok");
    stop(&mut worker, &database).await;
}

#[tokio::test]
async fn cancellation_before_statement_and_early_interrupt_cannot_escape_sticky_checks() {
    let database = Database::new();
    let (gate, started, release) = gate();
    let (progress, observed) = sync_mpsc::sync_channel(1);
    let hooks = execution::Hooks {
        gate: Some(gate),
        expensive: Some(progress),
        ..Default::default()
    };
    let mut worker =
        DbWorker::start_inner(database.open(), config(), metrics(&config()), hooks).unwrap();
    let ticket = worker.client().try_doctor(deadline()).unwrap();
    started.recv_timeout(Duration::from_secs(1)).unwrap();
    worker.interrupt.interrupt(); // No SQL is active; this signal alone is insufficient.
    drop(ticket);
    release.try_send(()).unwrap();
    assert_eq!(recover(&worker.client()).await.integrity_check, "ok");
    assert!(matches!(
        observed.try_recv(),
        Err(sync_mpsc::TryRecvError::Disconnected)
    ));
    stop(&mut worker, &database).await;
}

#[tokio::test]
async fn enqueue_between_empty_check_and_park_is_not_lost() {
    let database = Database::new();
    let (park, started, release) = gate();
    let hooks = execution::Hooks {
        park: Some(park),
        ..Default::default()
    };
    let mut worker =
        DbWorker::start_inner(database.open(), config(), metrics(&config()), hooks).unwrap();
    started.recv_timeout(Duration::from_secs(1)).unwrap();
    let ticket = worker.client().try_doctor(deadline()).unwrap();
    release.send(()).unwrap();
    assert_eq!(ticket.receive().await.unwrap().integrity_check, "ok");
    stop(&mut worker, &database).await;
}

#[tokio::test]
async fn stop_between_empty_check_and_park_is_not_lost() {
    let database = Database::new();
    let (park, started, _release) = gate();
    let hooks = execution::Hooks {
        park: Some(park),
        ..Default::default()
    };
    let mut worker =
        DbWorker::start_inner(database.open(), config(), metrics(&config()), hooks).unwrap();
    let _live_client = worker.client();
    started.recv_timeout(Duration::from_secs(1)).unwrap();
    worker.begin_shutdown();
    stop(&mut worker, &database).await;
}

#[tokio::test]
async fn lock_wait_is_bounded_and_shutdown_cancels_actual_busy_callback() {
    for shutdown_in_lock in [false, true] {
        let database = Database::new();
        let connection = database.open();
        let blocker = database.open();
        blocker.execute_batch("BEGIN IMMEDIATE").unwrap();
        let (lock_probe, observed) = sync_mpsc::sync_channel(1);
        let hooks = execution::Hooks {
            lock_probe: Some(lock_probe),
            ..Default::default()
        };
        let mut worker =
            DbWorker::start_inner(connection, config(), metrics(&config()), hooks).unwrap();
        let ticket = worker.client().try_doctor(deadline()).unwrap();
        observed.recv_timeout(Duration::from_secs(1)).unwrap();
        if shutdown_in_lock {
            worker.begin_shutdown();
        }
        let result = ticket.receive().await;
        if shutdown_in_lock {
            assert!(matches!(result, Err(DbError::Stopping)));
        } else {
            assert!(matches!(result, Err(DbError::Busy)));
        }
        blocker.execute_batch("ROLLBACK").unwrap();
        blocker.close().unwrap();
        stop(&mut worker, &database).await;
    }
}

#[tokio::test]
async fn repeated_idle_workers_close_and_join_with_live_clients() {
    let database = Database::new();
    for _ in 0..30 {
        let mut worker = DbWorker::start(database.open(), config(), metrics(&config())).unwrap();
        let client = worker.client();
        assert_eq!(recover(&client).await.integrity_check, "ok");
        stop(&mut worker, &database).await;
    }
}

#[tokio::test]
async fn shutdown_timeout_retains_handle_until_native_thread_really_exits() {
    let database = Database::new();
    let (exit, closed, release) = gate();
    let hooks = execution::Hooks {
        exit: Some(exit),
        ..Default::default()
    };
    let mut worker =
        DbWorker::start_inner(database.open(), config(), metrics(&config()), hooks).unwrap();
    worker.begin_shutdown();
    closed.recv_timeout(Duration::from_secs(1)).unwrap();
    // SQLite has closed but the native thread is still inside its bounded gate.
    assert!(matches!(
        worker.shutdown(tokio::time::Instant::now()).await,
        Err(DbError::ShutdownDeadline)
    ));
    assert!(!worker.thread.as_ref().unwrap().is_finished());
    release.send(()).unwrap();
    stop(&mut worker, &database).await;
}

#[tokio::test]
async fn concurrent_cancelled_admission_preserves_queue_and_outstanding_bounds() {
    let database = Database::new();
    let mut worker = DbWorker::start(database.open(), config(), metrics(&config())).unwrap();
    std::thread::scope(|scope| {
        for _ in 0..8 {
            let client = worker.client();
            scope.spawn(move || {
                for _ in 0..5_000 {
                    match client.try_doctor(deadline()) {
                        Ok(ticket) => drop(ticket),
                        Err(DbError::Busy) => (),
                        Err(error) => panic!("unexpected producer failure: {error}"),
                    }
                }
            });
        }
    });
    assert_eq!(recover(&worker.client()).await.integrity_check, "ok");
    stop(&mut worker, &database).await;
}

#[path = "schema_tests.rs"]
mod schema_tests;

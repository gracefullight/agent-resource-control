use super::*;
use crate::{
    db_worker::{self, DbConfig, DbError, WorkerTestHooks},
    limits::RuntimeLimits,
    runtime_metrics::{Counter, Gauge, RuntimeMetrics},
};
use std::{
    num::NonZeroUsize,
    sync::{mpsc, Arc},
    time::Duration,
};

/// The scan's read holds a queue slot and is counted in DbOutstanding like
/// every other admitted operation; a foreground request beside it is counted
/// too, and only a refused foreground request counts as rejected.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn background_attribution_read_is_counted_and_bounded_beside_foreground_requests() {
    let root = std::env::temp_dir().join(format!(
        "arc-attribution-worker-{}-{}",
        std::process::id(),
        arc_core::process_identity::random_job_id().unwrap()
    ));
    std::fs::create_dir(&root).unwrap();
    struct Cleanup(std::path::PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(root.clone());
    let path = root.join("arc.sqlite3");
    let connection = arc_storage::open_and_migrate(&path).unwrap();
    let limits = RuntimeLimits {
        db_queue: NonZeroUsize::new(1).unwrap(),
        ..RuntimeLimits::default()
    };
    let metrics = Arc::new(RuntimeMetrics::new(&limits));
    let (started, observed) = mpsc::sync_channel(1);
    let (release, wait) = mpsc::sync_channel(1);
    let mut worker = db_worker::start_with_test_hooks(
        connection,
        DbConfig {
            queue_capacity: limits.db_queue,
            busy_timeout: Duration::from_millis(100),
            execution_timeout: Duration::from_secs(60),
        },
        Arc::clone(&metrics),
        WorkerTestHooks {
            started,
            release: wait,
        },
    )
    .unwrap();
    let client = worker.client();
    let end = Instant::now() + Duration::from_secs(60);
    let background = tokio::spawn({
        let client = client.clone();
        async move { managed(&client, end).await }
    });
    observed.recv_timeout(Duration::from_secs(60)).unwrap();
    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.gauge(Gauge::DbExecuting).current, 1);
    assert_eq!(
        snapshot.gauge(Gauge::DbOutstanding).current,
        1,
        "the executing background read is outstanding database work"
    );

    let foreground = client.try_job(end, jobs::Operation::Recent).unwrap();
    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.gauge(Gauge::DbQueued).current, 1);
    assert_eq!(snapshot.gauge(Gauge::DbExecuting).current, 1);
    assert_eq!(snapshot.gauge(Gauge::DbOutstanding).current, 2);
    // The queue is full: a further scan read is refused without counting as
    // a rejected request; a refused foreground request is counted.
    assert_eq!(managed(&client, end).await, None);
    assert_eq!(metrics.snapshot().counter(Counter::DbRejected), 0);
    assert_eq!(metrics.snapshot().gauge(Gauge::DbOutstanding).current, 2);
    assert!(matches!(
        client.try_job(end, jobs::Operation::Recent),
        Err(DbError::Busy)
    ));
    assert_eq!(metrics.snapshot().counter(Counter::DbRejected), 1);

    release.send(()).unwrap();
    assert_eq!(
        background.await.unwrap(),
        Some(Vec::new()),
        "no RUNNING job"
    );
    assert!(foreground.receive_value().await.is_ok());
    // The worker drops each job's accounting just after replying.
    tokio::time::timeout(
        Duration::from_secs(60),
        metrics.wait_for(|s| s.gauge(Gauge::DbOutstanding).current == 0),
    )
    .await
    .expect("outstanding work drains");
    assert_eq!(metrics.snapshot().gauge(Gauge::DbOutstanding).high_water, 2);
    worker
        .shutdown(tokio::time::Instant::now() + Duration::from_secs(60))
        .await
        .unwrap();
}

//! Independent 100,000-attempt proofs at each real admission boundary.
use crate::{
    admission::{Handlers, Requests},
    db_worker,
    limits::RuntimeLimits,
    runtime::{receipt, Fixture},
    runtime_metrics::{Counter, Gauge, RuntimeMetrics, Snapshot},
};
use std::{
    sync::{mpsc, Arc},
    time::{Duration, Instant},
};

fn assert_bounds(snapshot: &Snapshot) {
    for (gauge, cap) in [
        (Gauge::Connections, snapshot.connection_cap),
        (Gauge::Requests, snapshot.request_cap),
        (Gauge::DbQueued, snapshot.db_queue_cap),
        (Gauge::DbExecuting, 1),
        (Gauge::DbOutstanding, snapshot.db_queue_cap + 1),
        (Gauge::RejectionWork, 1),
        (
            Gauge::RequestBodyBytes,
            snapshot.request_cap * arc_ipc::MAX_FRAME_BYTES as usize,
        ),
        (
            Gauge::ResponseBytes,
            snapshot.request_cap * arc_ipc::MAX_FRAME_BYTES as usize,
        ),
    ] {
        let observed = snapshot.gauge(gauge);
        assert!(
            observed.current <= cap && observed.high_water <= cap,
            "{gauge:?}: {observed:?} > {cap}"
        );
    }
    assert!(snapshot.tracked_tasks.current <= snapshot.connection_cap);
    assert!(snapshot.tracked_tasks.high_water <= snapshot.connection_cap);
}

fn assert_zero(snapshot: &Snapshot) {
    assert_bounds(snapshot);
    for gauge in Gauge::ALL {
        assert_eq!(snapshot.gauge(gauge).current, 0, "{gauge:?}");
    }
    assert_eq!(snapshot.tracked_tasks.current, 0);
}

#[tokio::test]
async fn hundred_thousand_connection_attempts_never_spawn_rejection_tasks() {
    let limits = RuntimeLimits::test_profile();
    let metrics = Arc::new(RuntimeMetrics::new(&limits));
    let mut handlers = Handlers::new(&limits, Arc::clone(&metrics)).unwrap();
    let mut clients = Vec::new();
    let mut admitted = 0;
    let mut rejected = 0;
    let start = Instant::now();
    for _ in 0..100_000 {
        let (server, client) = tokio::net::UnixStream::pair().unwrap();
        let accept = handlers.try_spawn(server, |_| std::future::pending::<()>());
        if accept {
            admitted += 1;
            clients.push(client);
        } else {
            rejected += 1;
        }
        assert_bounds(&metrics.snapshot());
    }
    assert_eq!(admitted, limits.connections.get());
    assert_eq!(admitted + rejected, 100_000);
    assert_eq!(handlers.len(), admitted);
    let before = metrics.snapshot();
    assert_eq!(before.counter(Counter::HandlersSpawned), admitted as u64);
    assert_eq!(before.counter(Counter::ConnectionRejected), rejected as u64);
    assert_eq!(before.gauge(Gauge::RejectionWork).high_water, 1);
    handlers
        .shutdown(tokio::time::Instant::now() + Duration::from_secs(1))
        .await
        .unwrap();
    let after = metrics.snapshot();
    assert_zero(&after);
    assert_eq!(after.counter(Counter::HandlersReaped), admitted as u64);
    receipt(
        "connection-100000",
        &serde_json::json!({"scope":"production Handlers::try_spawn with authenticated native socketpairs",
        "attempted":100000,"admitted":admitted,"rejected":rejected,"completed":0,"timed_out":0,
        "shutdown_cancelled":admitted,"elapsed_us":start.elapsed().as_micros(),"final":after.json()}),
    );
}

#[test]
fn hundred_thousand_request_attempts_have_no_waiting_admission() {
    let limits = RuntimeLimits::test_profile();
    let metrics = Arc::new(RuntimeMetrics::new(&limits));
    let requests = Requests::new(&limits, Arc::clone(&metrics)).unwrap();
    let mut held = Vec::new();
    let mut rejected = 0;
    for _ in 0..100_000 {
        match requests.try_admit() {
            Some(guard) => held.push(guard),
            None => rejected += 1,
        }
        assert_bounds(&metrics.snapshot());
    }
    let admitted = held.len();
    assert_eq!(admitted, limits.requests.get());
    assert_eq!(admitted + rejected, 100_000);
    assert_eq!(
        metrics.snapshot().counter(Counter::RequestRejected),
        rejected as u64
    );
    drop(held);
    assert_zero(&metrics.snapshot());
    let recovered = requests.try_admit().unwrap();
    drop(recovered);
    receipt(
        "request-100000",
        &serde_json::json!({"scope":"production Requests::try_admit; no network or latency inference",
        "attempted":100000,"admitted":admitted,"rejected":rejected,"completed":0,"timed_out":0,
        "released_without_dispatch":admitted,"shutdown_cancelled":0,"final":metrics.snapshot().json()}),
    );
}

#[tokio::test]
async fn hundred_thousand_database_attempts_bound_retained_cancelled_work() {
    let fixture = Fixture::new();
    let limits = RuntimeLimits::default();
    let metrics = Arc::new(RuntimeMetrics::new(&limits));
    let (started, observed) = mpsc::sync_channel(1);
    let (_release, release) = mpsc::sync_channel(1);
    let mut worker = db_worker::start_with_test_hooks(
        arc_storage::open_and_migrate(&fixture.database).unwrap(),
        db_worker::DbConfig {
            queue_capacity: limits.db_queue,
            busy_timeout: limits.db_busy_timeout,
            execution_timeout: limits.db_execution_timeout,
        },
        Arc::clone(&metrics),
        db_worker::WorkerTestHooks { started, release },
    )
    .unwrap();
    let client = worker.client();
    let deadline = Instant::now() + limits.db_response_timeout;
    let active = client.try_doctor(deadline).unwrap();
    observed.recv_timeout(Duration::from_secs(1)).unwrap();
    let mut tickets = vec![active];
    let mut rejected = 0;
    let start = Instant::now();
    for _ in 1..100_000 {
        match client.try_doctor(deadline) {
            Ok(ticket) => tickets.push(ticket),
            Err(db_worker::DbError::Busy) => rejected += 1,
            Err(error) => panic!("unexpected admission outcome: {error}"),
        }
        assert_bounds(&metrics.snapshot());
    }
    assert!(
        start.elapsed() < limits.db_execution_timeout,
        "controlled work expired during workload"
    );
    let admitted = tickets.len();
    assert_eq!(admitted, limits.db_queue.get() + 1);
    assert_eq!(admitted + rejected, 100_000);
    // Cancelling clients must not free queue/outstanding accounting early.
    let active = tickets.remove(0);
    drop(tickets);
    assert_eq!(
        metrics.snapshot().gauge(Gauge::DbQueued).current,
        limits.db_queue.get()
    );
    assert_eq!(
        metrics.snapshot().gauge(Gauge::DbOutstanding).current,
        admitted
    );
    drop(active);
    let exit = worker
        .shutdown(tokio::time::Instant::now() + Duration::from_secs(1))
        .await
        .unwrap();
    assert!(exit.worker_joined && exit.connection_closed);
    let final_snapshot = metrics.snapshot();
    assert_zero(&final_snapshot);
    assert_eq!(final_snapshot.counter(Counter::DbRejected), rejected as u64);
    let reopened = arc_storage::open_and_migrate(&fixture.database).unwrap();
    let integrity: String = reopened
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");
    reopened.close().unwrap();
    receipt(
        "db-100000",
        &serde_json::json!({"scope":"production DbClient::try_doctor with controlled real worker",
        "attempted":100000,"admitted":admitted,"rejected":rejected,"completed":0,"timed_out":0,
        "client_cancelled":admitted,"shutdown_cancelled":0,"elapsed_us":start.elapsed().as_micros(),
        "reopen_integrity":"ok","final":final_snapshot.json()}),
    );
}

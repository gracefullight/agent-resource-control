//! The new assessment keeps cancellation, deadline, one-worker and join guarantees.
use super::*;

#[tokio::test]
async fn schema_assessment_cancellation_deadline_and_shutdown_keep_worker_owned() {
    for mode in ["cancel", "deadline", "shutdown"] {
        let database = Database::new();
        let mut connection = database.open();
        let (started, observed) = sync_mpsc::sync_channel(1);
        let (release, receiver) = sync_mpsc::sync_channel(1);
        let mut once = Some(receiver);
        connection
            .authorizer(Some(move |context: rusqlite::hooks::AuthContext<'_>| {
                if matches!(
                    context.action,
                    rusqlite::hooks::AuthAction::Pragma {
                        pragma_name: "user_version",
                        ..
                    }
                ) {
                    if let Some(receiver) = once.take() {
                        started.send(()).unwrap();
                        receiver
                            .recv_timeout(Duration::from_secs(2))
                            .expect("test releases schema gate");
                    }
                }
                rusqlite::hooks::Authorization::Allow
            }))
            .unwrap();
        let mut limits = config();
        limits.execution_timeout = Duration::from_millis(100);
        let metrics = metrics(&limits);
        let mut worker = DbWorker::start(connection, limits, Arc::clone(&metrics)).unwrap();
        let client = worker.client();
        let request_deadline = Instant::now() + Duration::from_millis(100);
        let ticket = client
            .try_doctor(if mode == "deadline" {
                request_deadline
            } else {
                deadline()
            })
            .unwrap();
        observed.recv_timeout(Duration::from_secs(1)).unwrap();
        match mode {
            "shutdown" => {
                worker.begin_shutdown();
                release.send(()).unwrap();
                assert!(matches!(ticket.receive().await, Err(DbError::Stopping)));
            }
            "deadline" => {
                // Wait on the admitted request's absolute deadline while assessment is active.
                tokio::time::sleep_until(tokio::time::Instant::from_std(request_deadline)).await;
                release.send(()).unwrap();
                assert!(matches!(ticket.receive().await, Err(DbError::Deadline)));
                let report = recover(&client).await;
                assert_eq!(
                    report.schema.unwrap().status,
                    arc_ipc::SchemaStatus::Compatible
                );
            }
            _ => {
                drop(ticket);
                release.send(()).unwrap();
                let report = recover(&client).await;
                assert_eq!(
                    report.schema.unwrap().status,
                    arc_ipc::SchemaStatus::Compatible
                );
            }
        }
        assert_bounds(&metrics);
        assert_eq!(metrics.snapshot().gauge(Gauge::DbExecuting).high_water, 1);
        stop(&mut worker, &database).await;
    }
}

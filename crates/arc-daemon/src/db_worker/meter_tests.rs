//! Meter cadence follows committed SQLite state, including daemon-internal
//! lifecycle work. These fixtures do not launch a job or a native sampler.
use super::*;

#[tokio::test]
async fn meter_activity_changes_after_committed_work_and_never_changes_its_reply() {
    let path = std::env::temp_dir().join(format!(
        "arc-meter-activity-{}-{}.sqlite3",
        std::process::id(),
        arc_core::process_identity::random_job_id().unwrap()
    ));
    let connection = arc_storage::open_and_migrate(&path).unwrap();
    let config = DbConfig {
        queue_capacity: NonZeroUsize::new(2).unwrap(),
        busy_timeout: Duration::from_millis(30),
        execution_timeout: Duration::from_secs(1),
    };
    let metrics = Arc::new(RuntimeMetrics::new(&crate::limits::RuntimeLimits::default()));
    let mut worker = DbWorker::start(connection, config, Arc::clone(&metrics)).unwrap();
    let client = worker.client();
    let fixture = arc_storage::open_and_migrate(&path).unwrap();
    assert!(!client.shared.meter.lock().unwrap().active);
    fixture.execute("INSERT INTO jobs(id,job_class,locality,enforcement,state,created_at) VALUES(?1,'UNKNOWN','UNKNOWN','OBSERVE','RUNNING',0)", ["meter-activity-fixture"]).unwrap();
    for (state, active) in [("RUNNING", true), ("COMPLETED", false)] {
        fixture
            .execute("UPDATE jobs SET state=?1", [state])
            .unwrap();
        let ticket = client
            .try_job(
                Instant::now() + Duration::from_secs(2),
                jobs::Operation::Recent,
            )
            .unwrap();
        let value = ticket.receive_value().await.unwrap();
        assert!(
            matches!(value, DbValue::Lifecycle(value) if matches!(*value, jobs::Value::Status(Ok(_))))
        );
        metrics
            .wait_for(|snapshot| snapshot.gauge(Gauge::DbOutstanding).current == 0)
            .await;
        assert_eq!(client.shared.meter.lock().unwrap().active, active);
    }
    worker
        .shutdown(tokio::time::Instant::now() + Duration::from_secs(2))
        .await
        .unwrap();
    drop(fixture);
    // Close and remove only this fixture's owned SQLite file and sidecars.
    for suffix in ["", "-wal", "-shm"] {
        let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
    }
}

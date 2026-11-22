use super::*;
use arc_ipc::jobs::JobId;

const JOB: &str = "11111111111111111111111111111111";

struct Database(std::path::PathBuf, rusqlite::Connection);
impl Database {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "arc-job-tree-read-{}",
            arc_core::process_identity::random_job_id().unwrap()
        ));
        let connection = arc_storage::open_and_migrate(root.join("arc.sqlite3")).unwrap();
        connection
            .execute_batch(&format!(
                "INSERT INTO jobs(id,job_class,locality,enforcement,state,created_at)
                   VALUES('{JOB}','BUILD','LOCAL','MANAGED','SUCCEEDED',1);
                 INSERT INTO job_meter_summaries(job_id,measurement_aware) VALUES('{JOB}',1);"
            ))
            .unwrap();
        Self(root, connection)
    }
}
impl Drop for Database {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn query(job: &str) -> Query {
    Query {
        job_id: JobId::try_from(job.to_owned()).unwrap(),
    }
}

#[test]
fn the_reply_is_the_stored_report_or_null_bound_to_the_job() {
    let mut db = Database::new();
    let job = JobId::try_from(JOB.to_owned()).unwrap();
    let reply = read(&mut db.1, query(JOB), None, 0).unwrap();
    let decoded = wire::decode_reply(&reply, &job).unwrap();
    assert_eq!(decoded.process_tree, None);
    let other = "2".repeat(32);
    let missing = read(&mut db.1, query(&other), None, 0).unwrap();
    assert!(missing["process_tree"].is_null());

    let value = |value: f64| wire::TreeValue {
        value: Some(value),
        basis: wire::TreeBasis::TreeSamples,
    };
    let bytes = |value: u64| wire::TreeValue {
        value: Some(value),
        basis: wire::TreeBasis::TreeSamples,
    };
    let report = ProcessTreeReport {
        schema_version: wire::SCHEMA_VERSION,
        job_id: JOB.into(),
        terminal: true,
        cpu_core_seconds: value(1.5),
        waited_cpu_core_seconds: wire::TreeValue {
            value: Some(0.5),
            basis: wire::TreeBasis::WaitedRusage,
        },
        root_sampled_cpu_core_seconds: value(0.25),
        unwaited_cpu_core_seconds: value(1.0),
        memory_peak_bytes: bytes(10),
        disk_read_bytes: bytes(0),
        disk_write_bytes: bytes(4096),
        completeness: wire::Completeness::Partial,
        missing_coverage: vec![wire::MissingCoverageReason::UnobservedDescendants],
        samples: 2,
        processes: 2,
    };
    db.1.execute(
        "UPDATE job_meter_summaries SET process_tree_json=?2 WHERE job_id=?1",
        rusqlite::params![JOB, serde_json::to_string(&report).unwrap()],
    )
    .unwrap();
    let reply = read(&mut db.1, query(JOB), None, 0).unwrap();
    assert_eq!(
        wire::decode_reply(&reply, &job).unwrap().process_tree,
        Some(report.clone())
    );
    // A stored report of another job is never served as this one.
    let mut foreign = report;
    foreign.job_id = other;
    db.1.execute(
        "UPDATE job_meter_summaries SET process_tree_json=?2 WHERE job_id=?1",
        rusqlite::params![JOB, serde_json::to_string(&foreign).unwrap()],
    )
    .unwrap();
    assert!(read(&mut db.1, query(JOB), None, 0).is_err());
}

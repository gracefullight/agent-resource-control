//! Writer behaviour over synthetic stored facts; the native path is the
//! `job_process_meter` integration target.
use super::*;
use arc_meter::job_tree::{Membership, ProcessReading};

const JOB: &str = "11111111111111111111111111111111";
const BOOT: &str = "00000000-0000-0000-0000-000000000001";
const REPORT: &str = include_str!("../../../../arc-core/tests/fixtures/job-meter-report.json");

struct Database(std::path::PathBuf, rusqlite::Connection);
impl Database {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "arc-resource-samples-{}",
            arc_core::process_identity::random_job_id().unwrap()
        ));
        let connection = arc_storage::open_and_migrate(root.join("arc.sqlite3")).unwrap();
        connection
            .execute_batch(&format!(
                "INSERT INTO jobs(id,job_class,locality,enforcement,state,created_at)
                   VALUES('{JOB}','BUILD','LOCAL','MANAGED','RUNNING',1);
                 INSERT INTO job_lifecycle(job_id,registered_at_ms,observer_pid,
                   observer_start_token,observer_boot_id,observer_verified_at_ms,child_pid,
                   child_start_token,child_boot_id,child_verified_at_ms,classification_reason,
                   classification_uncertainty,observation_target,observation_status,
                   observation_at_ms,observation_reason,revision,updated_at_ms,reason)
                   VALUES('{JOB}',1,100,'1001','{BOOT}',1,101,'1002','{BOOT}',1,'test','NONE',
                   'CHILD','VERIFIED_MATCH',1,'test',1,1,'test');
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

fn child() -> ProcessIdentity {
    ProcessIdentity {
        pid: 101,
        start_token: "1002".into(),
        boot_id: BOOT.into(),
    }
}
fn reading(pid: u32, membership: Membership, cpu: u64) -> ProcessReading {
    ProcessReading {
        identity: ProcessIdentity {
            pid,
            start_token: (1001 + u64::from(pid) - 100).to_string(),
            boot_id: BOOT.into(),
        },
        cpu_time_ns: cpu,
        child_cpu_time_ns: 0,
        rss_bytes: 4096,
        disk_read_bytes: 1,
        disk_write_bytes: 2,
        membership,
    }
}
fn record(child: ProcessIdentity) -> Record {
    let mut tree = arc_meter::job_tree::JobTree::new(false);
    let readings = [
        reading(101, Membership::Root, 500_000_000),
        reading(102, Membership::Detached, 1_500_000_000),
    ];
    let rows = tree.observe(10, &readings);
    Record {
        job_id: JOB.into(),
        child,
        timestamp_ns: 10,
        rows,
        totals: tree.totals(),
    }
}
fn stored(connection: &rusqlite::Connection) -> Option<ProcessTreeReport> {
    connection
        .query_row(
            "SELECT process_tree_json FROM job_meter_summaries WHERE job_id=?1",
            [JOB],
            |row| row.get::<_, Option<String>>(0),
        )
        .unwrap()
        .map(|json| serde_json::from_str(&json).unwrap())
}
fn pass(connection: &mut rusqlite::Connection, now_ms: u64) -> Pass {
    match execute(connection, Operation::Running { now_ms }) {
        Value::Running(Ok(pass)) => pass,
        _ => panic!("pass failed"),
    }
}
fn write_record(connection: &mut rusqlite::Connection, record: Record) -> bool {
    match execute(connection, Operation::Record(Box::new(record))) {
        Value::Recorded(Ok(written)) => written,
        _ => panic!("record failed"),
    }
}

#[test]
fn a_sample_is_written_only_under_the_recorded_running_child() {
    let mut db = Database::new();
    let listed = pass(&mut db.1, 1);
    assert_eq!(listed.running.len(), 1);
    assert_eq!(listed.running[0].child, child());
    assert!(!listed.running[0].resumed && !listed.awaiting_waited);

    // Same PID, another start time: a recycled identity writes nothing.
    let recycled = ProcessIdentity {
        start_token: "1003".into(),
        ..child()
    };
    assert!(!write_record(&mut db.1, record(recycled)));
    assert_eq!(stored(&db.1), None);

    assert!(write_record(&mut db.1, record(child())));
    let rows: Vec<(String, String, String, String)> =
        db.1.prepare(
            "SELECT timestamp_ns,process_identity,cpu_time_ns,attribution_confidence
             FROM resource_samples ORDER BY id",
        )
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].0, "10");
    assert_eq!(
        serde_json::from_str::<ProcessIdentity>(&rows[0].1).unwrap(),
        child()
    );
    assert_eq!((rows[0].2.as_str(), rows[0].3.as_str()), ("500000000", "A"));
    assert_eq!(
        (rows[1].2.as_str(), rows[1].3.as_str()),
        ("1500000000", "C")
    );

    let running = stored(&db.1).unwrap();
    assert!(!running.terminal);
    assert_eq!(running.waited_cpu_core_seconds.basis, TreeBasis::Unobserved);
    assert_eq!(running.cpu_core_seconds.value, Some(2.0));
    assert_eq!(running.memory_peak_bytes.value, Some(8192));
    assert_eq!(running.completeness, Completeness::Partial);
    assert!(pass(&mut db.1, 1).running[0].resumed);
}

#[test]
fn a_left_job_is_finalized_then_completed_with_its_waited_usage() {
    let mut db = Database::new();
    assert!(write_record(&mut db.1, record(child())));
    db.1.execute_batch(&format!(
        "UPDATE jobs SET state='SUCCEEDED' WHERE id='{JOB}';
         UPDATE job_meter_summaries SET terminal_at_ms=5000 WHERE job_id='{JOB}';"
    ))
    .unwrap();
    // Too old to complete: left as the running snapshot.
    let late = pass(&mut db.1, 5000 + FINALIZE_WINDOW_MS + 1);
    assert!(late.running.is_empty() && !late.awaiting_waited);
    assert!(!stored(&db.1).unwrap().terminal);

    let first = pass(&mut db.1, 6000);
    assert!(first.awaiting_waited);
    let terminal = stored(&db.1).unwrap();
    assert!(terminal.terminal);
    // Without the waited usage: the root's last sample plus the unwaited CPU.
    assert_eq!(terminal.cpu_core_seconds.value, Some(2.0));
    assert_eq!(terminal.waited_cpu_core_seconds.value, None);
    assert!(
        !pass(&mut db.1, 6000).awaiting_waited,
        "nothing new to read"
    );

    db.1.execute(
        "UPDATE job_meter_summaries SET report_id=?2,report_json=?3 WHERE job_id=?1",
        params![JOB, "2".repeat(32), REPORT],
    )
    .unwrap();
    assert!(!pass(&mut db.1, 6000).awaiting_waited);
    let completed = stored(&db.1).unwrap();
    assert_eq!(completed.waited_cpu_core_seconds.value, Some(2.0));
    assert_eq!(
        completed.waited_cpu_core_seconds.basis,
        TreeBasis::WaitedRusage
    );
    assert_eq!(completed.cpu_core_seconds.value, Some(2.0 + 1.5));
    assert_eq!(completed.unwaited_cpu_core_seconds.value, Some(1.5));
    assert_eq!(completed.missing_coverage, [Reason::UnobservedDescendants]);
    // Idempotent: a complete terminal report is not rewritten.
    let before = stored(&db.1);
    pass(&mut db.1, 6000);
    assert_eq!(stored(&db.1), before);
}

#[test]
fn unobserved_values_are_null_with_their_basis_never_zero() {
    let report = build(
        JOB,
        &Parts::from_totals(&TreeTotals::default()),
        false,
        None,
    );
    assert_eq!(report.completeness, Completeness::Unknown);
    for value in [
        &report.cpu_core_seconds,
        &report.root_sampled_cpu_core_seconds,
        &report.unwaited_cpu_core_seconds,
        &report.waited_cpu_core_seconds,
    ] {
        assert_eq!((value.value, value.basis), (None, TreeBasis::Unobserved));
    }
    assert_eq!(report.memory_peak_bytes.value, None);
    assert_eq!(report.disk_write_bytes.basis, TreeBasis::Unobserved);
    // A root-only tree with its waited usage is complete.
    let totals = TreeTotals {
        root_cpu_ns: Some(1),
        samples: 1,
        processes: 1,
        peak_rss_bytes: Some(1),
        disk_read_bytes: Some(0),
        disk_write_bytes: Some(0),
        ..TreeTotals::default()
    };
    let report = build(JOB, &Parts::from_totals(&totals), true, Some(0.5));
    assert_eq!(report.completeness, Completeness::Complete);
    assert_eq!(report.cpu_core_seconds.value, Some(0.5));
    assert_eq!(report.disk_read_bytes.value, Some(0));
}

/// Integration of lanes A and C: rows written by this writer (cumulative
/// per-process `cpu_time_ns` and `disk_write_bytes`, 19-digit wall-ns text,
/// identity JSON) roll up to their growth and are then deleted.
#[test]
fn written_samples_roll_up_to_their_growth_then_are_deleted() {
    let mut db = Database::new();
    let now_ms: u64 = 1_800_000_000_000;
    let minute = (now_ms - 73 * 60 * 60 * 1000) / 60_000 * 60_000;
    let first_ns = (minute + 1_000) * 1_000_000;
    let sweep = |cpu: [u64; 2], written: [u64; 2]| {
        let mut root = reading(101, Membership::Root, cpu[0]);
        root.disk_write_bytes = written[0];
        let mut detached = reading(102, Membership::Detached, cpu[1]);
        detached.disk_write_bytes = written[1];
        [root, detached]
    };
    let mut tree = arc_meter::job_tree::JobTree::new(false);
    for (offset, readings) in [
        (0, sweep([500_000_000, 1_500_000_000], [10, 100])),
        (
            1_000_000_000,
            sweep([1_000_000_000, 2_500_000_000], [30, 1_100]),
        ),
    ] {
        let timestamp_ns = first_ns + offset;
        let rows = tree.observe(timestamp_ns, &readings);
        assert!(write_record(
            &mut db.1,
            Record {
                job_id: JOB.into(),
                child: child(),
                timestamp_ns,
                rows,
                totals: tree.totals(),
            }
        ));
    }
    let report = arc_storage::rollup::roll_up(&mut db.1, now_ms).unwrap();
    assert_eq!(report.raw_rolled_up, 4);
    let raw: i64 =
        db.1.query_row("SELECT count(*) FROM resource_samples", [], |r| r.get(0))
            .unwrap();
    assert_eq!(raw, 0);
    for granularity in ["minute", "hour", "day"] {
        let (cpu, peak, written, jobs): (f64, i64, i64, i64) =
            db.1.query_row(
                "SELECT sum(cpu_core_seconds),max(peak_rss_bytes),sum(disk_write_bytes),
                   sum(job_count) FROM resource_rollups WHERE granularity=?1",
                [granularity],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        assert!((cpu - 1.5).abs() < 1e-9, "{granularity}: {cpu}");
        assert_eq!(
            (peak, written, jobs),
            (8192, 20 + 1_000, 1),
            "{granularity}"
        );
    }
}

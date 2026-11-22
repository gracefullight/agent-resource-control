//! The aggregation rules over a minimal schema of the columns read. The
//! production path (installed hook, built arcctl, real daemon) is
//! `arc-daemon tests/usage_summary.rs`.
use super::*;
use serde_json::json;

const HOUR: u64 = 3_600_000;
const T0: u64 = 1_767_225_600_000; // 2026-01-01T00:00:00Z, a day boundary.

fn database() -> Connection {
    let connection = Connection::open_in_memory().unwrap();
    connection
        .execute_batch(
            "CREATE TABLE jobs(id TEXT PRIMARY KEY, job_class TEXT, state TEXT, vendor TEXT,
               project_key TEXT, hook_overhead_ms INTEGER);
             CREATE TABLE job_lifecycle(job_id TEXT PRIMARY KEY, registered_at_ms INTEGER);
             CREATE TABLE job_meter_summaries(job_id TEXT PRIMARY KEY, relation TEXT,
               report_json TEXT);
             CREATE TABLE traffic_entries(job_id TEXT, state TEXT);",
        )
        .unwrap();
    connection
}

struct Job<'a> {
    id: u32,
    class: &'a str,
    state: &'a str,
    vendor: Option<&'a str>,
    project: Option<&'a str>,
    at: u64,
    overhead: Option<i64>,
    relation: &'a str,
    cpu: Option<f64>,
    run: Option<f64>,
}

fn insert(connection: &Connection, job: Job<'_>) {
    let id = format!("{:032x}", job.id);
    connection
        .execute(
            "INSERT INTO jobs VALUES(?1,?2,?3,?4,?5,?6)",
            params![
                id,
                job.class,
                job.state,
                job.vendor,
                job.project,
                job.overhead
            ],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO job_lifecycle VALUES(?1,?2)",
            params![id, job.at as i64],
        )
        .unwrap();
    let report = json!({"cpu_core_seconds":{"total":job.cpu},"run_elapsed":{"seconds":job.run}});
    connection
        .execute(
            "INSERT INTO job_meter_summaries VALUES(?1,?2,?3)",
            params![id, job.relation, report.to_string()],
        )
        .unwrap();
}

fn base(id: u32) -> Job<'static> {
    Job {
        id,
        class: "BUILD",
        state: "SUCCEEDED",
        vendor: None,
        project: None,
        at: T0,
        overhead: None,
        relation: "canonical_root",
        cpu: Some(1.5),
        run: Some(2.0),
    }
}

fn query(by: GroupBy, bucket: Bucket) -> UsageQuery {
    UsageQuery {
        by,
        from_ms: T0,
        to_ms: T0 + 2 * HOUR,
        bucket,
    }
}

#[test]
fn vendors_group_with_none_and_remote_jobs_add_nothing_to_local_totals() {
    let c = database();
    insert(
        &c,
        Job {
            vendor: Some("claude-code"),
            overhead: Some(4),
            ..base(1)
        },
    );
    insert(
        &c,
        Job {
            vendor: Some("claude-code"),
            overhead: Some(9),
            state: "FAILED",
            at: T0 + 10,
            ..base(2)
        },
    );
    // Nested in job 1: counted, never summed.
    insert(
        &c,
        Job {
            vendor: Some("claude-code"),
            relation: "included_in_root",
            cpu: Some(100.0),
            run: Some(100.0),
            at: T0 + 20,
            ..base(3)
        },
    );
    insert(
        &c,
        Job {
            at: T0 + 30,
            ..base(4)
        },
    );
    insert(
        &c,
        Job {
            class: "REMOTE_CI",
            cpu: Some(7.0),
            run: Some(3.0),
            at: T0 + 40,
            ..base(5)
        },
    );
    // Out of range on both sides.
    insert(
        &c,
        Job {
            at: T0 - 1,
            ..base(6)
        },
    );
    insert(
        &c,
        Job {
            at: T0 + 2 * HOUR,
            ..base(7)
        },
    );
    let summary = summarize(&c, &query(GroupBy::Vendor, Bucket::Hour), T0 + 5).unwrap();
    assert_eq!(summary.generated_at_ms, T0 + 5);
    assert_eq!(summary.query, query(GroupBy::Vendor, Bucket::Hour));
    assert_eq!(summary.groups.len(), 2, "{:?}", summary.groups);
    let claude = &summary.groups[0];
    assert_eq!(claude["vendor"], "claude-code");
    assert_eq!(claude["bucket_start_ms"], T0);
    assert_eq!(claude["bucket_end_ms"], T0 + HOUR);
    let local = &claude["local"];
    assert_eq!(local["job.count"]["total"], 3);
    assert_eq!(
        local["job.count"]["by_class_outcome"],
        json!([
            {"class":"BUILD","outcome":"FAILED","count":1},
            {"class":"BUILD","outcome":"SUCCEEDED","count":2},
        ])
    );
    assert_eq!(local["cpu_core_seconds"]["value"], 3.0);
    assert_eq!(local["cpu_core_seconds"]["measured_jobs"], 2);
    assert_eq!(local["job.duration_ms"]["value"], 4000.0);
    assert_eq!(
        local["arc.hook.overhead_ms"],
        json!({"sum":13,"max":9,"jobs":2})
    );
    assert_eq!(claude["remote"]["job.count"]["total"], 0);
    let none = &summary.groups[1];
    assert_eq!(none["vendor"], NONE);
    assert_eq!(none["local"]["job.count"]["total"], 1);
    assert_eq!(none["local"]["cpu_core_seconds"]["value"], 1.5);
    assert_eq!(
        none["local"]["arc.hook.overhead_ms"],
        json!({"sum":null,"max":null,"jobs":0})
    );
    let remote = &none["remote"];
    assert_eq!(remote["job.count"]["total"], 1);
    assert_eq!(remote["cpu_core_seconds"]["value"], Value::Null);
    assert_eq!(
        remote["cpu_core_seconds"]["reason"],
        REASON_NOT_METERED_LOCALLY
    );
    assert_eq!(remote["job.duration_ms"]["value"], 3000.0);
}

#[test]
fn projects_and_periods_split_by_bucket_and_unknown_values_stay_null() {
    let c = database();
    let key = "ab".repeat(32);
    insert(
        &c,
        Job {
            project: Some(&key),
            ..base(1)
        },
    );
    insert(
        &c,
        Job {
            at: T0 + HOUR + 1,
            cpu: None,
            run: None,
            ..base(2)
        },
    );
    let projects = summarize(&c, &query(GroupBy::Project, Bucket::Hour), T0).unwrap();
    let names: Vec<_> = projects
        .groups
        .iter()
        .map(|g| g["project"].clone())
        .collect();
    assert_eq!(names, [json!(key), json!(NONE)]);
    let unmeasured = &projects.groups[1]["local"];
    assert_eq!(unmeasured["cpu_core_seconds"]["value"], Value::Null);
    assert_eq!(
        unmeasured["cpu_core_seconds"]["reason"],
        REASON_NO_RECORDED_REPORT
    );
    assert_eq!(unmeasured["job.duration_ms"]["value"], Value::Null);
    let hours = summarize(&c, &query(GroupBy::Period, Bucket::Hour), T0).unwrap();
    let starts: Vec<_> = hours
        .groups
        .iter()
        .map(|g| g["bucket_start_ms"].clone())
        .collect();
    assert_eq!(starts, [json!(T0), json!(T0 + HOUR)]);
    assert!(hours.groups[0].get("vendor").is_none());
    let days = summarize(&c, &query(GroupBy::Period, Bucket::Day), T0).unwrap();
    assert_eq!(days.groups.len(), 1);
    assert_eq!(days.groups[0]["bucket_end_ms"], T0 + 24 * HOUR);
    assert_eq!(days.groups[0]["local"]["job.count"]["total"], 2);
}

#[test]
fn an_invalid_range_is_refused_and_queue_depth_counts_only_queued_entries() {
    let c = database();
    let mut empty = query(GroupBy::Vendor, Bucket::Day);
    empty.to_ms = empty.from_ms;
    assert_eq!(summarize(&c, &empty, T0), Err(JobError::InvalidRequest));
    for state in ["QUEUED", "QUEUED", "RESERVED", "RUNNING", "COMPLETED"] {
        c.execute("INSERT INTO traffic_entries VALUES('j',?1)", [state])
            .unwrap();
    }
    assert_eq!(
        queue_depth(&c, T0).unwrap(),
        SnapshotTraffic {
            queue_depth: 2,
            observed_at_ms: T0
        }
    );
}

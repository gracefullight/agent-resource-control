//! v2 grouping and quality rules over a minimal schema of the columns read.
//! The production path is `arc-daemon tests/meter_quality.rs`.
use super::*;
use arc_ipc::meter::usage::Bucket;

const HOUR: u64 = 3_600_000;
const T0: u64 = 1_767_225_600_000;

fn database() -> Connection {
    let connection = Connection::open_in_memory().unwrap();
    connection
        .execute_batch(
            "CREATE TABLE jobs(id TEXT PRIMARY KEY, job_class TEXT, state TEXT, vendor TEXT,
               project_key TEXT, hook_overhead_ms INTEGER, session_key TEXT,
               footprint_start_bytes INTEGER, footprint_start_completeness TEXT,
               footprint_end_bytes INTEGER, footprint_end_completeness TEXT);
             CREATE TABLE job_lifecycle(job_id TEXT PRIMARY KEY, registered_at_ms INTEGER);
             CREATE TABLE job_meter_summaries(job_id TEXT PRIMARY KEY, relation TEXT,
               report_json TEXT, process_tree_json TEXT);",
        )
        .unwrap();
    connection
}

/// One footprint reading: bytes and completeness.
type Reading = (i64, &'static str);

#[derive(Clone)]
struct Job {
    id: u32,
    class: &'static str,
    vendor: Option<&'static str>,
    session: Option<String>,
    relation: &'static str,
    waited: Option<f64>,
    /// (tree CPU, completeness, peak, read, written)
    tree: Option<(f64, &'static str, u64, u64, u64)>,
    footprint: Option<(Reading, Reading)>,
}

fn job(id: u32) -> Job {
    Job {
        id,
        class: "BUILD",
        vendor: Some("claude-code"),
        session: None,
        relation: "canonical_root",
        waited: Some(1.0),
        tree: None,
        footprint: None,
    }
}

fn insert(c: &Connection, job: &Job) {
    let id = format!("{:032x}", job.id);
    let (start, end) = job.footprint.unzip();
    c.execute(
        "INSERT INTO jobs VALUES(?1,?2,'SUCCEEDED',?3,NULL,NULL,?4,?5,?6,?7,?8)",
        params![
            id,
            job.class,
            job.vendor,
            job.session,
            start.map(|s| s.0),
            start.map(|s| s.1),
            end.map(|e| e.0),
            end.map(|e| e.1)
        ],
    )
    .unwrap();
    c.execute(
        "INSERT INTO job_lifecycle VALUES(?1,?2)",
        params![id, T0 as i64],
    )
    .unwrap();
    let report = job
        .waited
        .map(|w| json!({"cpu_core_seconds":{"total":w},"run_elapsed":{"seconds":1.0}}).to_string());
    let tree = job.tree.map(|(cpu, completeness, peak, read, written)| {
        json!({"cpu_core_seconds":{"value":cpu,"basis":"tree_samples"},
               "completeness":completeness,
               "memory_peak_bytes":{"value":peak,"basis":"tree_samples"},
               "disk_read_bytes":{"value":read,"basis":"tree_samples"},
               "disk_write_bytes":{"value":written,"basis":"tree_samples"}})
        .to_string()
    });
    c.execute(
        "INSERT INTO job_meter_summaries VALUES(?1,?2,?3,?4)",
        params![id, job.relation, report, tree],
    )
    .unwrap();
}

fn query(by: GroupByV2) -> UsageQueryV2 {
    UsageQueryV2 {
        by,
        from_ms: T0,
        to_ms: T0 + HOUR,
        bucket: Bucket::Hour,
    }
}

fn group<'a>(summary: &'a UsageSummaryV2, key: &str, value: &str) -> &'a Value {
    let found: Vec<_> = summary.groups.iter().filter(|g| g[key] == value).collect();
    assert_eq!(found.len(), 1, "{key}={value}: {:?}", summary.groups);
    found[0]
}

#[test]
fn sessions_group_by_digest_and_vendor_and_a_missing_session_is_none() {
    let c = database();
    let (a, b) = ("a".repeat(64), "b".repeat(64));
    for (id, vendor, session) in [
        (1, Some("claude-code"), Some(a.clone())),
        (2, Some("claude-code"), Some(a.clone())),
        (3, Some("claude-code"), Some(b.clone())),
        (4, Some("cursor"), Some(b.clone())),
        (5, None, None),
    ] {
        insert(
            &c,
            &Job {
                vendor,
                session,
                ..job(id)
            },
        );
    }
    let summary = summarize(&c, &query(GroupByV2::Session), T0).unwrap();
    let keys: Vec<_> = summary
        .groups
        .iter()
        .map(|g| {
            (
                g["session"].as_str().unwrap(),
                g["vendor"].as_str().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        keys,
        [
            (a.as_str(), "claude-code"),
            (b.as_str(), "claude-code"),
            (b.as_str(), "cursor"),
            ("none", "none")
        ]
    );
    let total = |g: &Value| g["local"]["job.count"]["total"].as_u64().unwrap();
    let counts: Vec<_> = summary.groups.iter().map(total).collect();
    assert_eq!(counts, [2, 1, 1, 1]);
    assert_eq!(summary.groups[0]["local"]["cpu_core_seconds"]["value"], 2.0);
    // The v1 groupings keep their keys.
    let by_vendor = summarize(&c, &query(GroupByV2::Vendor), T0).unwrap();
    assert!(by_vendor.groups.iter().all(|g| g.get("session").is_none()));
}

#[test]
fn every_metered_value_is_flagged_and_tree_cpu_wins_over_waited() {
    let c = database();
    // claude-code: tree-sampled complete; cursor: tree-sampled and a
    // waited-only job (partial); none: an F4 partial tree and a job included
    // in its root (not eligible); a remote job.
    let tree = |cpu, completeness| Some((cpu, completeness, 100, 10, 1000));
    insert(
        &c,
        &Job {
            tree: tree(3.0, "complete"),
            footprint: Some(((4096, "complete"), (8192, "complete"))),
            ..job(1)
        },
    );
    insert(
        &c,
        &Job {
            vendor: Some("cursor"),
            tree: tree(2.0, "complete"),
            ..job(2)
        },
    );
    insert(
        &c,
        &Job {
            vendor: Some("cursor"),
            waited: Some(0.5),
            footprint: Some(((4096, "complete"), (8192, "partial"))),
            ..job(3)
        },
    );
    insert(
        &c,
        &Job {
            vendor: None,
            tree: tree(4.0, "partial"),
            ..job(4)
        },
    );
    insert(
        &c,
        &Job {
            vendor: None,
            relation: "included_in_root",
            tree: tree(9.0, "complete"),
            ..job(5)
        },
    );
    insert(
        &c,
        &Job {
            vendor: None,
            class: "REMOTE_CI",
            ..job(6)
        },
    );
    let summary = summarize(&c, &query(GroupByV2::Vendor), T0).unwrap();
    let local = |vendor: &str| group(&summary, "vendor", vendor)["local"].clone();

    let claude = local("claude-code");
    let cpu = &claude["cpu_core_seconds"];
    assert_eq!(cpu["value"], 3.0, "tree CPU, not the waited 1.0");
    assert_eq!(cpu["availability"], "partial");
    assert_eq!(cpu["basis"], BASIS_TREE);
    assert_eq!(claude["footprint_delta_bytes"]["value"], 4096);
    assert_eq!(claude["footprint_delta_bytes"]["availability"], "complete");
    // A legacy complete tree cannot prove sampled CPU, peaks, or I/O complete.
    for key in ["memory_peak_bytes", "disk_read_bytes", "disk_write_bytes"] {
        assert_eq!(claude[key]["availability"], "partial", "{key}");
        assert!(claude[key]["value"].is_u64(), "{key}");
        assert_eq!(claude[key]["measured_jobs"], 1, "{key}");
    }

    let cursor = local("cursor");
    let cpu = &cursor["cpu_core_seconds"];
    assert_eq!(cpu["value"], 2.5);
    assert_eq!(
        cpu["availability"], "partial",
        "waited-only is a lower bound"
    );
    assert_eq!(cpu["basis"], BASIS_TREE_AND_WAITED);
    assert_eq!(
        (
            cpu["tree_sampled_jobs"].clone(),
            cpu["waited_only_jobs"].clone()
        ),
        (json!(1), json!(1))
    );
    assert_eq!(cursor["disk_write_bytes"]["value"], 1000);
    assert_eq!(cursor["disk_write_bytes"]["availability"], "partial");
    // The partial end reading gives no delta, so only a lower bound is known.
    assert_eq!(cursor["footprint_delta_bytes"]["value"], Value::Null);
    assert_eq!(
        cursor["footprint_delta_bytes"]["availability"],
        "unavailable"
    );
    assert_eq!(
        cursor["footprint_delta_bytes"]["reason"],
        REASON_NOT_MEASURED
    );

    let none = local("none");
    let cpu = &none["cpu_core_seconds"];
    assert_eq!(cpu["value"], 4.0, "the included job is its root's");
    assert_eq!(cpu["availability"], "partial", "an F4 partial tree");
    assert_eq!(cpu["eligible_jobs"], 1);
    assert_eq!(none["job.count"]["total"], 2);
    let remote = &group(&summary, "vendor", "none")["remote"];
    assert_eq!(remote["cpu_core_seconds"]["availability"], "unavailable");
    assert_eq!(
        remote["cpu_core_seconds"]["reason"],
        REASON_NOT_METERED_LOCALLY
    );
    assert_eq!(remote["job.duration_ms"]["availability"], "complete");
    // A group without local jobs is unavailable, never 0.
    let summary = summarize(&c, &query(GroupByV2::Project), T0).unwrap();
    let local = &summary.groups[0]["local"];
    assert_eq!(local["cpu_core_seconds"]["availability"], "partial");
    for metric in [
        "cpu_core_seconds",
        "job.duration_ms",
        "memory_peak_bytes",
        "disk_read_bytes",
        "disk_write_bytes",
        "footprint_delta_bytes",
    ] {
        let value = &local[metric];
        let availability = value["availability"].as_str().unwrap();
        assert!(!value["basis"].as_str().unwrap().is_empty(), "{metric}");
        assert_eq!(
            value["value"].is_null(),
            availability == "unavailable",
            "{metric}"
        );
        assert_eq!(
            value["reason"].is_null(),
            availability != "unavailable",
            "{metric}"
        );
    }
}

#[test]
fn a_local_side_without_jobs_or_measurements_is_unavailable_with_a_reason() {
    let c = database();
    insert(
        &c,
        &Job {
            class: "REMOTE_EXEC",
            ..job(1)
        },
    );
    insert(
        &c,
        &Job {
            vendor: Some("cursor"),
            waited: None,
            ..job(2)
        },
    );
    let summary = summarize(&c, &query(GroupByV2::Vendor), T0).unwrap();
    let empty = &group(&summary, "vendor", "claude-code")["local"]["cpu_core_seconds"];
    assert_eq!(empty["value"], Value::Null);
    assert_eq!(empty["reason"], REASON_NO_LOCAL_JOBS);
    let unmeasured = &group(&summary, "vendor", "cursor")["local"];
    assert_eq!(
        unmeasured["cpu_core_seconds"]["reason"],
        REASON_NO_RECORDED_REPORT
    );
    assert_eq!(unmeasured["disk_write_bytes"]["reason"], REASON_NOT_SAMPLED);
    assert_eq!(
        unmeasured["footprint_delta_bytes"]["reason"],
        REASON_NOT_MEASURED
    );
    assert_eq!(unmeasured["cpu_core_seconds"]["eligible_jobs"], 1);
}

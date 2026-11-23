//! The usage aggregates recomputed from the daemon's SQLite with one SQL
//! `GROUP BY` per query, independently of `arc_storage::usage_summary`, and
//! compared with what built `arcctl` printed. The additive rule is the same:
//! duration and CPU sum only `canonical_root` jobs, and REMOTE_CI and
//! REMOTE_EXEC jobs add no CPU and are compared against `remote`. CPU is the
//! negotiated (v2) value (cycle 35 F3): the job's sampled process-tree CPU
//! where its tree was sampled, else the waited rusage of its Meter report.
use rusqlite::{params, Connection};
use serde_json::Value;
use std::collections::BTreeMap;

pub const HOUR_MS: u64 = 3_600_000;
pub const DAY_MS: u64 = 86_400_000;

/// The F3 CPU of one job row `m` of `job_meter_summaries`: its sampled tree
/// CPU, else its waited rusage; `NULL` unless it is a `canonical_root`.
pub const JOB_CPU: &str = "CASE WHEN m.relation='canonical_root' THEN COALESCE(
      CASE WHEN json_extract(m.process_tree_json,'$.cpu_core_seconds.basis')='tree_samples'
        THEN json_extract(m.process_tree_json,'$.cpu_core_seconds.value') END,
      CASE WHEN m.report_json IS NOT NULL
        THEN json_extract(m.report_json,'$.cpu_core_seconds.total') END) END";

/// The F3 CPU of the one job `filter` selects from `jobs j`.
pub fn job_cpu(db: &Connection, filter: &str) -> Option<f64> {
    db.query_row(
        &format!(
            "SELECT {JOB_CPU} FROM jobs j JOIN job_meter_summaries m ON m.job_id=j.id \
             WHERE {filter}"
        ),
        [],
        |r| r.get(0),
    )
    .unwrap()
}

/// Wait until no stored tree report can still change: every tree is
/// terminal, and completed with the waited usage when a report exists.
pub fn settle(db: &Connection) {
    crate::journey::eventually("settled process-tree reports", || {
        db.query_row(
            "SELECT count(*) FROM job_meter_summaries m JOIN jobs j ON j.id=m.job_id
             WHERE m.process_tree_json IS NOT NULL
               AND (json_extract(m.process_tree_json,'$.terminal')=0
                 OR (m.report_json IS NOT NULL AND
                   json_extract(m.process_tree_json,'$.waited_cpu_core_seconds.basis')!='waited_rusage'))",
            [],
            |r| r.get::<_, i64>(0),
        )
        .unwrap()
            == 0
    });
}

/// One recomputed (group, bucket, remote) aggregate.
#[derive(Debug, Default, PartialEq)]
struct Expected {
    total: u64,
    by_class_outcome: BTreeMap<(String, String), u64>,
    cpu: Option<f64>,
    duration_ms: Option<f64>,
    overhead: (Option<u64>, Option<u64>, u64),
}

fn key_column(by: &str) -> &'static str {
    match by {
        "vendor" => "COALESCE(j.vendor,'none')",
        "project" => "COALESCE(j.project_key,'none')",
        "period" => "''",
        other => panic!("unknown grouping {other}"),
    }
}

fn expected(
    db: &Connection,
    by: &str,
    from: u64,
    to: u64,
    width: u64,
) -> BTreeMap<(String, u64, bool), Expected> {
    let additive = "m.relation='canonical_root' AND m.report_json IS NOT NULL";
    let remote = "j.job_class IN ('REMOTE_CI','REMOTE_EXEC')";
    let sql = format!(
        "SELECT {key},(l.registered_at_ms/?3)*?3,{remote},j.job_class,j.state,count(*),
         SUM(CASE WHEN NOT {remote} THEN {JOB_CPU} END),
         SUM(CASE WHEN {additive}
             THEN json_extract(m.report_json,'$.run_elapsed.seconds')*1000.0 END),
         SUM(j.hook_overhead_ms),MAX(j.hook_overhead_ms),count(j.hook_overhead_ms)
         FROM job_lifecycle l JOIN jobs j ON j.id=l.job_id
         LEFT JOIN job_meter_summaries m ON m.job_id=j.id
         WHERE l.registered_at_ms>=?1 AND l.registered_at_ms<?2
         GROUP BY 1,2,3,4,5",
        key = key_column(by)
    );
    let mut groups: BTreeMap<(String, u64, bool), Expected> = BTreeMap::new();
    let mut statement = db.prepare(&sql).unwrap();
    let rows = statement
        .query_map(params![from as i64, to as i64, width as i64], |r| {
            Ok((
                (
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)? as u64,
                    r.get(2)?,
                ),
                (r.get::<_, String>(3)?, r.get::<_, String>(4)?),
                r.get::<_, i64>(5)? as u64,
                (r.get::<_, Option<f64>>(6)?, r.get::<_, Option<f64>>(7)?),
                (
                    r.get::<_, Option<i64>>(8)?.map(|n| n as u64),
                    r.get::<_, Option<i64>>(9)?.map(|n| n as u64),
                    r.get::<_, i64>(10)? as u64,
                ),
            ))
        })
        .unwrap();
    let add = |a: Option<f64>, b: Option<f64>| match (a, b) {
        (None, None) => None,
        (a, b) => Some(a.unwrap_or(0.0) + b.unwrap_or(0.0)),
    };
    for row in rows {
        let (group, class_outcome, count, (cpu, duration), (sum, max, jobs)) = row.unwrap();
        let entry = groups.entry(group).or_default();
        entry.total += count;
        entry.by_class_outcome.insert(class_outcome, count);
        entry.cpu = add(entry.cpu, cpu);
        entry.duration_ms = add(entry.duration_ms, duration);
        entry.overhead = (
            match (entry.overhead.0, sum) {
                (None, None) => None,
                (a, b) => Some(a.unwrap_or(0) + b.unwrap_or(0)),
            },
            entry.overhead.1.max(max),
            entry.overhead.2 + jobs,
        );
    }
    groups
}

fn number(value: &Value) -> Option<f64> {
    (!value.is_null()).then(|| value.as_f64().unwrap())
}

#[track_caller]
pub fn same(label: &str, printed: Option<f64>, recomputed: Option<f64>) {
    match (printed, recomputed) {
        (None, None) => (),
        // Summation order and decimal JSON parsing (within a few ULP) differ only.
        (Some(a), Some(b)) => assert!((a - b).abs() <= 1e-9 * b.abs().max(1.0), "{label}: {a} {b}"),
        other => panic!("{label}: {other:?}"),
    }
}

fn printed(aggregate: &Value) -> Expected {
    let count = &aggregate["job.count"];
    Expected {
        total: count["total"].as_u64().unwrap(),
        by_class_outcome: count["by_class_outcome"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| {
                (
                    (
                        c["class"].as_str().unwrap().to_owned(),
                        c["outcome"].as_str().unwrap().to_owned(),
                    ),
                    c["count"].as_u64().unwrap(),
                )
            })
            .collect(),
        cpu: number(&aggregate["cpu_core_seconds"]["value"]),
        duration_ms: number(&aggregate["job.duration_ms"]["value"]),
        overhead: (
            aggregate["arc.hook.overhead_ms"]["sum"].as_u64(),
            aggregate["arc.hook.overhead_ms"]["max"].as_u64(),
            aggregate["arc.hook.overhead_ms"]["jobs"].as_u64().unwrap(),
        ),
    }
}

/// Every printed group equals the SQLite recomputation, and every recomputed
/// aggregate is printed. Returns the number of jobs the summary covers.
#[track_caller]
pub fn assert_matches(db: &Connection, summary: &Value) -> u64 {
    let query = &summary["query"];
    let by = query["by"].as_str().unwrap();
    let (from, to) = (
        query["from_ms"].as_u64().unwrap(),
        query["to_ms"].as_u64().unwrap(),
    );
    let width = match query["bucket"].as_str().unwrap() {
        "hour" => HOUR_MS,
        "day" => DAY_MS,
        other => panic!("{other}"),
    };
    let mut expected = expected(db, by, from, to, width);
    let mut covered = 0;
    for group in summary["groups"].as_array().unwrap() {
        let key = match by {
            "period" => {
                assert!(group.get("vendor").is_none() && group.get("project").is_none());
                String::new()
            }
            by => group[by].as_str().unwrap().to_owned(),
        };
        let bucket = group["bucket_start_ms"].as_u64().unwrap();
        assert_eq!(bucket % width, 0, "{group}");
        assert_eq!(group["bucket_end_ms"].as_u64().unwrap(), bucket + width);
        for (side, remote) in [("local", false), ("remote", true)] {
            let printed = printed(&group[side]);
            covered += printed.total;
            let recomputed = expected
                .remove(&(key.clone(), bucket, remote))
                .unwrap_or_default();
            let label = format!("{by} {key} {bucket} {side}");
            assert_eq!(printed.total, recomputed.total, "{label}");
            assert_eq!(
                printed.by_class_outcome, recomputed.by_class_outcome,
                "{label}"
            );
            assert_eq!(printed.overhead, recomputed.overhead, "{label}");
            same(
                &format!("{label} duration"),
                printed.duration_ms,
                recomputed.duration_ms,
            );
            if remote {
                let cpu = &group[side]["cpu_core_seconds"];
                assert_eq!(cpu["value"], Value::Null, "{label}");
                assert_eq!(cpu["reason"], "not_metered_locally", "{label}");
            } else {
                same(&format!("{label} cpu"), printed.cpu, recomputed.cpu);
            }
        }
    }
    assert!(expected.is_empty(), "aggregates not printed: {expected:?}");
    covered
}

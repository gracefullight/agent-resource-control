//! Usage aggregates from recorded jobs (lane B, MTR-004, MTR-008, MTR-018,
//! MET-013, MET-015) and `traffic.queue.depth` (MET-017).
//!
//! One read statement per query, so every aggregate comes from one committed
//! state. A job is in range when its registration time is in `[from, to)`;
//! its period bucket is that time floored to the UTC hour or day. Each group
//! (vendor, project or period, always split by bucket) has a `local` and a
//! `remote` aggregate:
//!
//! - `job.count`: every job, by class and stored outcome state;
//! - `job.duration_ms`: the recorded run duration (`run_elapsed`) of the job
//!   Meter report, in milliseconds;
//! - `cpu_core_seconds`: the waited rusage total of the job Meter report;
//! - `arc.hook.overhead_ms`: the hook's own measured time of hook jobs.
//!
//! Duration and CPU sum only `canonical_root` reports, so a job nested in
//! another is counted but never summed twice. REMOTE_CI and REMOTE_EXEC jobs
//! go to `remote`, whose CPU is `null` with reason `not_metered_locally`;
//! they add nothing to `local`. A job without a vendor or project groups
//! under `none` (a direct command with no agent hook), never Unattributed. An
//! unmeasured value is `null` with a reason, never 0.
use crate::jobs::{JobError, JobResult};
use arc_ipc::meter::usage::{
    Bucket, GroupBy, SnapshotTraffic, UsageQuery, UsageQueryV2, UsageSummary, UsageSummaryV2,
    MAX_USAGE_SUMMARY_BYTES, SCHEMA_VERSION,
};
use rusqlite::{params, Connection};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

/// The group of a job without a vendor or project key.
pub const NONE: &str = "none";
pub const REASON_NOT_METERED_LOCALLY: &str = "not_metered_locally";
pub const REASON_NO_RECORDED_REPORT: &str = "no_recorded_report";
pub const REMOTE_CLASSES: [&str; 2] = ["REMOTE_CI", "REMOTE_EXEC"];
const HOUR_MS: u64 = 3_600_000;
const DAY_MS: u64 = 86_400_000;

/// Recorded facts of one job; the only inputs of an aggregate.
struct JobRow {
    class: String,
    state: String,
    vendor: Option<String>,
    project: Option<String>,
    registered_at_ms: u64,
    hook_overhead_ms: Option<u64>,
    /// `canonical_root` with a stored report: eligible for sums.
    additive: bool,
    /// Recorded as the canonical root of its own usage.
    canonical_root: bool,
    cpu_core_seconds: Option<f64>,
    run_seconds: Option<f64>,
}

#[derive(Default)]
struct Aggregate {
    counts: BTreeMap<(String, String), u64>,
    duration_ms: Option<f64>,
    duration_jobs: u64,
    cpu_core_seconds: Option<f64>,
    cpu_jobs: u64,
    overhead_sum: Option<u64>,
    overhead_max: Option<u64>,
    overhead_jobs: u64,
}

impl Aggregate {
    fn add(&mut self, job: &JobRow, remote: bool) {
        *self
            .counts
            .entry((job.class.clone(), job.state.clone()))
            .or_default() += 1;
        if job.additive {
            if let Some(seconds) = job.run_seconds {
                self.duration_ms = Some(self.duration_ms.unwrap_or(0.0) + seconds * 1000.0);
                self.duration_jobs += 1;
            }
            if let (false, Some(cpu)) = (remote, job.cpu_core_seconds) {
                self.cpu_core_seconds = Some(self.cpu_core_seconds.unwrap_or(0.0) + cpu);
                self.cpu_jobs += 1;
            }
        }
        if let Some(overhead) = job.hook_overhead_ms {
            self.overhead_sum = Some(self.overhead_sum.unwrap_or(0).saturating_add(overhead));
            self.overhead_max = Some(self.overhead_max.unwrap_or(0).max(overhead));
            self.overhead_jobs += 1;
        }
    }

    fn to_value(&self, remote: bool) -> Value {
        let total: u64 = self.counts.values().sum();
        let by: Vec<Value> = self
            .counts
            .iter()
            .map(|((class, outcome), count)| {
                json!({"class": class, "outcome": outcome, "count": count})
            })
            .collect();
        let reason = |value: Option<f64>| value.is_none().then_some(REASON_NO_RECORDED_REPORT);
        let cpu = if remote {
            json!({"value": null, "measured_jobs": 0, "reason": REASON_NOT_METERED_LOCALLY})
        } else {
            json!({
                "value": self.cpu_core_seconds,
                "measured_jobs": self.cpu_jobs,
                "basis": "canonical_root_waited_rusage",
                "reason": reason(self.cpu_core_seconds),
            })
        };
        json!({
            "job.count": {"total": total, "by_class_outcome": by},
            "job.duration_ms": {
                "value": self.duration_ms,
                "measured_jobs": self.duration_jobs,
                "basis": "canonical_root_run_elapsed",
                "reason": reason(self.duration_ms),
            },
            "cpu_core_seconds": cpu,
            "arc.hook.overhead_ms": {
                "sum": self.overhead_sum,
                "max": self.overhead_max,
                "jobs": self.overhead_jobs,
            },
        })
    }
}

#[derive(Default)]
struct Group {
    local: Aggregate,
    remote: Aggregate,
}

pub fn bucket_ms(bucket: Bucket) -> u64 {
    match bucket {
        Bucket::Hour => HOUR_MS,
        Bucket::Day => DAY_MS,
    }
}

fn rows(connection: &Connection, query: &UsageQuery) -> JobResult<Vec<JobRow>> {
    let from = i64::try_from(query.from_ms).map_err(|_| JobError::InvalidRequest)?;
    let to = i64::try_from(query.to_ms).map_err(|_| JobError::InvalidRequest)?;
    // The lifecycle registration index bounds the scan to the range.
    let mut statement = connection.prepare(
        "SELECT j.job_class,j.state,j.vendor,j.project_key,l.registered_at_ms,j.hook_overhead_ms,
         COALESCE(m.relation='canonical_root' AND m.report_json IS NOT NULL,0),
         json_extract(m.report_json,'$.cpu_core_seconds.total'),
         json_extract(m.report_json,'$.run_elapsed.seconds'),
         COALESCE(m.relation='canonical_root',0)
         FROM job_lifecycle l JOIN jobs j ON j.id=l.job_id
         LEFT JOIN job_meter_summaries m ON m.job_id=j.id
         WHERE l.registered_at_ms>=?1 AND l.registered_at_ms<?2",
    )?;
    let rows = statement.query_map(params![from, to], |r| {
        Ok(JobRow {
            class: r.get(0)?,
            state: r.get(1)?,
            vendor: r.get(2)?,
            project: r.get(3)?,
            registered_at_ms: u64::try_from(r.get::<_, i64>(4)?).unwrap_or(0),
            hook_overhead_ms: r
                .get::<_, Option<i64>>(5)?
                .and_then(|n| u64::try_from(n).ok()),
            additive: r.get(6)?,
            cpu_core_seconds: r.get::<_, Option<f64>>(7)?.filter(|v| v.is_finite()),
            run_seconds: r.get::<_, Option<f64>>(8)?.filter(|v| v.is_finite()),
            canonical_root: r.get(9)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

pub fn summarize(
    connection: &Connection,
    query: &UsageQuery,
    now_ms: u64,
) -> JobResult<UsageSummary> {
    if !query.is_valid() || now_ms > arc_ipc::jobs::MAX_TIMESTAMP {
        return Err(JobError::InvalidRequest);
    }
    let width = bucket_ms(query.bucket);
    let mut groups: BTreeMap<(String, u64), Group> = BTreeMap::new();
    for job in rows(connection, query)? {
        let key = match query.by {
            GroupBy::Vendor => job.vendor.clone().unwrap_or_else(|| NONE.to_owned()),
            GroupBy::Project => job.project.clone().unwrap_or_else(|| NONE.to_owned()),
            GroupBy::Period => String::new(),
        };
        let bucket = job.registered_at_ms - job.registered_at_ms % width;
        let group = groups.entry((key, bucket)).or_default();
        if REMOTE_CLASSES.contains(&job.class.as_str()) {
            group.remote.add(&job, true);
        } else {
            group.local.add(&job, false);
        }
    }
    let mut values = Vec::with_capacity(groups.len());
    for ((key, bucket), group) in groups {
        let mut value = Map::new();
        match query.by {
            GroupBy::Vendor => {
                value.insert("vendor".into(), json!(key));
            }
            GroupBy::Project => {
                value.insert("project".into(), json!(key));
            }
            GroupBy::Period => (),
        }
        value.insert("bucket_start_ms".into(), json!(bucket));
        value.insert("bucket_end_ms".into(), json!(bucket.saturating_add(width)));
        value.insert("local".into(), group.local.to_value(false));
        value.insert("remote".into(), group.remote.to_value(true));
        values.push(Value::Object(value));
    }
    let summary = UsageSummary {
        schema_version: SCHEMA_VERSION,
        query: *query,
        generated_at_ms: now_ms,
        groups: values,
    };
    // A range too wide to answer within the bound is refused, not truncated.
    match serde_json::to_vec(&summary) {
        Ok(bytes) if bytes.len() <= MAX_USAGE_SUMMARY_BYTES => Ok(summary),
        _ => Err(JobError::InvalidRequest),
    }
}

/// The `meter.usage_summary.v2` groups: `--by session`, quality flags on
/// every value, F3 tree CPU and F4 partial completeness (`usage_summary/v2.rs`).
/// The `attribution` section is added by the dispatch, not here.
pub fn summarize_v2(
    connection: &Connection,
    query: &UsageQueryV2,
    now_ms: u64,
) -> JobResult<UsageSummaryV2> {
    summarize_v2_with_codex(connection, query, now_ms, false)
}

/// A negotiated v3 response can use the Codex vendor vocabulary.
pub fn summarize_v2_with_codex(
    connection: &Connection,
    query: &UsageQueryV2,
    now_ms: u64,
    codex: bool,
) -> JobResult<UsageSummaryV2> {
    let summary = if codex {
        v2::summarize_codex(connection, query, now_ms, true)?
    } else {
        v2::summarize(connection, query, now_ms)?
    };
    // A range too wide to answer within the bound is refused, not truncated.
    match serde_json::to_vec(&summary) {
        Ok(bytes) if bytes.len() <= MAX_USAGE_SUMMARY_BYTES => Ok(summary),
        _ => Err(JobError::InvalidRequest),
    }
}

/// `traffic.queue.depth`: Traffic entries waiting for a permit right now.
pub fn queue_depth(connection: &Connection, now_ms: u64) -> JobResult<SnapshotTraffic> {
    if now_ms > arc_ipc::jobs::MAX_TIMESTAMP {
        return Err(JobError::InvalidRequest);
    }
    let depth: i64 = connection.query_row(
        "SELECT count(*) FROM traffic_entries WHERE state='QUEUED'",
        [],
        |r| r.get(0),
    )?;
    Ok(SnapshotTraffic {
        queue_depth: u64::try_from(depth).map_err(|_| JobError::StorageFailure)?,
        observed_at_ms: now_ms,
    })
}

#[path = "usage_summary/v2.rs"]
mod v2;

#[cfg(test)]
#[path = "usage_summary/tests.rs"]
mod tests;

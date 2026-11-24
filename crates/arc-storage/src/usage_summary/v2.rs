//! The `meter.usage_summary.v2` groups (lane B: G-01 item 3, MTR-015 item 3,
//! R-005 item 3; cycle 35 F3 and F4).
//!
//! The same jobs, range, buckets and v1 keys as [`super::summarize`], with
//! `--by session` and quality flags added. Grouping by session keys a group
//! by the stored session digest (`jobs.session_key`) and the vendor; a job
//! without a digest groups under session `none`, which no digest (64 hex
//! digits) can equal. Raw session ids are never stored, so none is read.
//!
//! Every reported CPU, memory, I/O, disk-written, footprint and duration
//! value of a `local` aggregate is a flagged object (`value`,
//! `availability`, `basis`, `reason`, `measured_jobs`, `eligible_jobs`).
//! Eligible jobs are the local jobs not recorded as included in another
//! job's root (their usage is their root's); the aggregate is `complete`
//! only when every eligible job contributed a complete value, `partial` (a
//! lower bound) when some did, and `unavailable` with `value: null` and a
//! reason when none did, never a silent zero. Sums use only `canonical_root`
//! jobs, as in v1.
//!
//! - `cpu_core_seconds` (F3): the process-tree CPU of the job's samples
//!   (`process_tree_json`, basis `tree_samples`, always a lower bound) where
//!   the tree was sampled; otherwise the waited rusage of
//!   the v1 report, basis `waited_rusage`, always `partial` because it omits
//!   descendants the wrapper did not wait for. A tree report made `partial`
//!   by F4 (waited usage above the sampled root) stays partial here. The
//!   aggregate basis is `tree_samples`, `waited_rusage`, or
//!   `tree_samples_and_waited_rusage` when both contributed;
//!   `tree_sampled_jobs` and `waited_only_jobs` count them.
//! - `memory_peak_bytes`: the largest per-job sampled peak (not a sum).
//! - `disk_read_bytes`, `disk_write_bytes`: sums of the sampled cumulative
//!   I/O (Disk Written is cumulative, never a footprint).
//! - These three are sampled lower bounds and so at most `partial`.
//! - `footprint_delta_bytes`: the sum of allocated-footprint deltas (basis
//!   `allocated`), only from jobs whose start and end readings are complete.
//! - `job.duration_ms`: the v1 value with its flags.
//!
//! `remote` aggregates keep the v1 counts; their metered values are
//! unavailable with reason `not_metered_locally`.
use super::{bucket_ms, Aggregate, JobRow, NONE, REASON_NOT_METERED_LOCALLY, REMOTE_CLASSES};
use crate::jobs::{JobError, JobResult};
use arc_ipc::meter::usage::{GroupByV2, UsageQueryV2, UsageSummaryV2, SCHEMA_VERSION};
use rusqlite::{params, Connection};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

pub const REASON_NO_RECORDED_REPORT: &str = super::REASON_NO_RECORDED_REPORT;
pub const REASON_NOT_SAMPLED: &str = "not_sampled";
pub const REASON_NOT_MEASURED: &str = "not_measured";
pub const REASON_NO_LOCAL_JOBS: &str = "no_eligible_jobs";
pub const BASIS_TREE: &str = "tree_samples";
pub const BASIS_WAITED: &str = "waited_rusage";
pub const BASIS_TREE_AND_WAITED: &str = "tree_samples_and_waited_rusage";
pub const BASIS_ALLOCATED: &str = "allocated";
pub const BASIS_RUN_ELAPSED: &str = "canonical_root_run_elapsed";

/// One job's value of one metric: the value and whether it is complete.
type Contribution<T> = Option<(T, bool)>;

/// The recorded facts of one job used by v2 beyond the v1 row.
struct Row {
    base: JobRow,
    session: Option<String>,
    /// Not recorded as included in another job's root.
    eligible: bool,
    waited_cpu: Option<f64>,
    tree_cpu: Option<f64>,
    peak: Option<u64>,
    read: Option<u64>,
    written: Option<u64>,
    footprint_delta: Option<i64>,
}

/// Running state of one flagged metric over a group's eligible jobs.
#[derive(Default)]
struct Metric<T> {
    value: Option<T>,
    measured: u64,
    complete: u64,
    eligible: u64,
}

impl<T: Copy> Metric<T> {
    fn add(&mut self, eligible: bool, contribution: Contribution<T>, fold: fn(T, T) -> T) {
        if !eligible {
            return;
        }
        self.eligible += 1;
        if let Some((value, complete)) = contribution {
            self.value = Some(self.value.map_or(value, |sum| fold(sum, value)));
            self.measured += 1;
            self.complete += u64::from(complete);
        }
    }

    fn to_value(&self, basis: &str, missing: &str) -> Value
    where
        T: serde::Serialize,
    {
        let (availability, reason) = match self.value {
            None if self.eligible == 0 => ("unavailable", Some(REASON_NO_LOCAL_JOBS)),
            None => ("unavailable", Some(missing)),
            Some(_) if self.complete == self.eligible => ("complete", None),
            Some(_) => ("partial", None),
        };
        json!({
            "value": self.value,
            "availability": availability,
            "basis": basis,
            "reason": reason,
            "measured_jobs": self.measured,
            "eligible_jobs": self.eligible,
        })
    }
}

#[derive(Default)]
struct Local {
    cpu: Metric<f64>,
    tree_sampled: u64,
    waited_only: u64,
    duration: Metric<f64>,
    peak: Metric<u64>,
    read: Metric<u64>,
    written: Metric<u64>,
    footprint: Metric<i64>,
}

impl Local {
    fn add(&mut self, row: &Row) {
        // A sampled peak or I/O total is never complete: the tree report's
        // completeness cannot exclude a child that exited between samples.
        let summed = |value: Option<u64>| value.filter(|_| row.eligible_root()).map(|v| (v, false));
        // F3: the sampled tree where it exists, else the waited lower bound.
        let cpu = match (row.eligible_root(), row.tree_cpu, row.waited_cpu) {
            (true, Some(tree), _) => {
                self.tree_sampled += 1;
                Some((tree, false))
            }
            (true, None, Some(waited)) => {
                self.waited_only += 1;
                Some((waited, false))
            }
            _ => None,
        };
        self.cpu.add(row.eligible, cpu, |a, b| a + b);
        self.duration.add(row.eligible, duration(row), |a, b| a + b);
        self.peak.add(row.eligible, summed(row.peak), u64::max);
        self.read
            .add(row.eligible, summed(row.read), u64::saturating_add);
        self.written
            .add(row.eligible, summed(row.written), u64::saturating_add);
        let footprint = row
            .footprint_delta
            .filter(|_| row.eligible_root())
            .map(|d| (d, true));
        self.footprint
            .add(row.eligible, footprint, i64::saturating_add);
    }

    fn cpu_basis(&self) -> &'static str {
        match (self.tree_sampled, self.waited_only) {
            (0, w) if w > 0 => BASIS_WAITED,
            (t, w) if t > 0 && w > 0 => BASIS_TREE_AND_WAITED,
            _ => BASIS_TREE,
        }
    }

    /// The flagged values added to the v1 keys of a local aggregate.
    fn extend(&self, aggregate: &mut Map<String, Value>) {
        let mut cpu = self
            .cpu
            .to_value(self.cpu_basis(), REASON_NO_RECORDED_REPORT);
        cpu["tree_sampled_jobs"] = json!(self.tree_sampled);
        cpu["waited_only_jobs"] = json!(self.waited_only);
        aggregate.insert("cpu_core_seconds".into(), cpu);
        aggregate.insert(
            "job.duration_ms".into(),
            self.duration
                .to_value(BASIS_RUN_ELAPSED, REASON_NO_RECORDED_REPORT),
        );
        for (key, metric) in [
            ("memory_peak_bytes", &self.peak),
            ("disk_read_bytes", &self.read),
            ("disk_write_bytes", &self.written),
        ] {
            aggregate.insert(key.into(), metric.to_value(BASIS_TREE, REASON_NOT_SAMPLED));
        }
        aggregate.insert(
            "footprint_delta_bytes".into(),
            self.footprint
                .to_value(BASIS_ALLOCATED, REASON_NOT_MEASURED),
        );
    }
}

/// The recorded run duration of an additive job, in milliseconds.
fn duration(row: &Row) -> Contribution<f64> {
    row.base
        .run_seconds
        .filter(|_| row.base.additive)
        .map(|s| (s * 1000.0, true))
}

impl Row {
    /// Eligible and the canonical root of its own usage.
    fn eligible_root(&self) -> bool {
        self.eligible && self.base.canonical_root
    }
}

fn remote(aggregate: &mut Map<String, Value>, duration: &Metric<f64>) {
    let missing = |basis: &str| {
        json!({"value": null, "availability": "unavailable", "basis": basis,
               "reason": REASON_NOT_METERED_LOCALLY, "measured_jobs": 0, "eligible_jobs": 0})
    };
    aggregate.insert("cpu_core_seconds".into(), missing(BASIS_TREE));
    for key in ["memory_peak_bytes", "disk_read_bytes", "disk_write_bytes"] {
        aggregate.insert(key.into(), missing(BASIS_TREE));
    }
    aggregate.insert("footprint_delta_bytes".into(), missing(BASIS_ALLOCATED));
    aggregate.insert(
        "job.duration_ms".into(),
        duration.to_value(BASIS_RUN_ELAPSED, REASON_NO_RECORDED_REPORT),
    );
}

fn rows(connection: &Connection, query: &UsageQueryV2, codex: bool) -> JobResult<Vec<Row>> {
    let from = i64::try_from(query.from_ms).map_err(|_| JobError::InvalidRequest)?;
    let to = i64::try_from(query.to_ms).map_err(|_| JobError::InvalidRequest)?;
    let mut statement = connection.prepare(
        "SELECT j.job_class,j.state,CASE WHEN ?3 THEN COALESCE(j.vendor_codex,j.vendor) ELSE j.vendor END,j.project_key,l.registered_at_ms,j.hook_overhead_ms,
         COALESCE(m.relation='canonical_root' AND m.report_json IS NOT NULL,0),
         json_extract(m.report_json,'$.cpu_core_seconds.total'),
         json_extract(m.report_json,'$.run_elapsed.seconds'),
         j.session_key, COALESCE(m.relation,'unverified'),
         CASE WHEN json_extract(m.process_tree_json,'$.cpu_core_seconds.basis')='tree_samples'
           THEN json_extract(m.process_tree_json,'$.cpu_core_seconds.value') END,
         json_extract(m.process_tree_json,'$.memory_peak_bytes.value'),
         json_extract(m.process_tree_json,'$.disk_read_bytes.value'),
         json_extract(m.process_tree_json,'$.disk_write_bytes.value'),
         CASE WHEN j.footprint_start_completeness='complete' AND j.footprint_end_completeness='complete'
           THEN j.footprint_end_bytes-j.footprint_start_bytes END
         FROM job_lifecycle l JOIN jobs j ON j.id=l.job_id
         LEFT JOIN job_meter_summaries m ON m.job_id=j.id
         WHERE l.registered_at_ms>=?1 AND l.registered_at_ms<?2",
    )?;
    let count = |n: Option<i64>| n.and_then(|n| u64::try_from(n).ok());
    let rows = statement.query_map(params![from, to, codex], |r| {
        let relation: String = r.get(10)?;
        let waited_cpu = r.get::<_, Option<f64>>(7)?.filter(|v| v.is_finite());
        Ok(Row {
            base: JobRow {
                class: r.get(0)?,
                state: r.get(1)?,
                vendor: r.get(2)?,
                project: r.get(3)?,
                registered_at_ms: u64::try_from(r.get::<_, i64>(4)?).unwrap_or(0),
                hook_overhead_ms: count(r.get(5)?),
                additive: r.get(6)?,
                cpu_core_seconds: waited_cpu,
                run_seconds: r.get::<_, Option<f64>>(8)?.filter(|v| v.is_finite()),
                canonical_root: relation == "canonical_root",
            },
            session: r.get(9)?,
            eligible: relation != "included_in_root",
            waited_cpu,
            tree_cpu: r
                .get::<_, Option<f64>>(11)?
                .filter(|v| v.is_finite() && *v >= 0.0),
            peak: count(r.get(12)?),
            read: count(r.get(13)?),
            written: count(r.get(14)?),
            footprint_delta: r.get(15)?,
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

#[derive(Default)]
struct Group {
    local: Aggregate,
    remote: Aggregate,
    quality: Local,
    remote_duration: Metric<f64>,
}

pub fn summarize(
    connection: &Connection,
    query: &UsageQueryV2,
    now_ms: u64,
) -> JobResult<UsageSummaryV2> {
    summarize_codex(connection, query, now_ms, false)
}

pub fn summarize_codex(
    connection: &Connection,
    query: &UsageQueryV2,
    now_ms: u64,
    codex: bool,
) -> JobResult<UsageSummaryV2> {
    if !query.is_valid() || now_ms > arc_ipc::jobs::MAX_TIMESTAMP {
        return Err(JobError::InvalidRequest);
    }
    let width = bucket_ms(query.bucket);
    let mut groups: BTreeMap<(String, String, u64), Group> = BTreeMap::new();
    for row in rows(connection, query, codex)? {
        let job = &row.base;
        let vendor = || job.vendor.clone().unwrap_or_else(|| NONE.to_owned());
        let key = match query.by {
            GroupByV2::Vendor => (vendor(), String::new()),
            GroupByV2::Project => (
                job.project.clone().unwrap_or_else(|| NONE.to_owned()),
                String::new(),
            ),
            GroupByV2::Period => (String::new(), String::new()),
            GroupByV2::Session => (
                row.session.clone().unwrap_or_else(|| NONE.to_owned()),
                vendor(),
            ),
        };
        let bucket = job.registered_at_ms - job.registered_at_ms % width;
        let group = groups.entry((key.0, key.1, bucket)).or_default();
        if REMOTE_CLASSES.contains(&job.class.as_str()) {
            group.remote.add(job, true);
            group
                .remote_duration
                .add(row.eligible, duration(&row), |a, b| a + b);
        } else {
            group.local.add(job, false);
            group.quality.add(&row);
        }
    }
    let mut values = Vec::with_capacity(groups.len());
    for ((key, vendor, bucket), group) in groups {
        let mut value = Map::new();
        match query.by {
            GroupByV2::Vendor => {
                value.insert("vendor".into(), json!(key));
            }
            GroupByV2::Project => {
                value.insert("project".into(), json!(key));
            }
            GroupByV2::Period => (),
            GroupByV2::Session => {
                value.insert("session".into(), json!(key));
                value.insert("vendor".into(), json!(vendor));
            }
        }
        value.insert("bucket_start_ms".into(), json!(bucket));
        value.insert("bucket_end_ms".into(), json!(bucket.saturating_add(width)));
        let mut local = group.local.to_value(false);
        if let Some(object) = local.as_object_mut() {
            group.quality.extend(object);
        }
        let mut remote_value = group.remote.to_value(true);
        if let Some(object) = remote_value.as_object_mut() {
            remote(object, &group.remote_duration);
        }
        value.insert("local".into(), local);
        value.insert("remote".into(), remote_value);
        values.push(Value::Object(value));
    }
    Ok(UsageSummaryV2 {
        schema_version: SCHEMA_VERSION,
        query: *query,
        generated_at_ms: now_ms,
        groups: values,
        attribution: None,
    })
}

#[cfg(test)]
#[path = "v2_tests.rs"]
mod tests;

//! Job process-tree samples on the single SQLite writer (lane A, MTR-002,
//! MET-002..010, DET-012).
//!
//! `job_process_meter` reads the RUNNING managed jobs' recorded child
//! identities (pid, start token, boot id) and writes identity-revalidated
//! per-process rows into `resource_samples`: `cpu_core_percent` (100 = one
//! core, from consecutive `cpu_time_ns` deltas), RSS and disk read/write
//! bytes, with `attribution_confidence` `A` for the root and `C` for
//! process-group and ppid descendants (Estimated). `timestamp_ns` and
//! `cpu_time_ns` are decimal TEXT; `process_identity` is the identity's JSON.
//!
//! A row is written only while the job is still RUNNING under exactly that
//! child identity, in the same transaction that refreshes the job's running
//! `process_tree_json`. Once the job has left RUNNING, the next pass turns
//! that snapshot into the terminal report, with the root's waited usage from
//! the v1 report when recorded. Finalization reads only stored facts, so it
//! survives a daemon restart.
use super::jobs;
use crate::db_worker::DbClient;
use arc_core::{job_meter::MissingCoverageReason as Reason, process_identity::ProcessIdentity};
use arc_ipc::meter::job_tree::{
    Completeness, ProcessTreeReport, TreeBasis, TreeValue, SCHEMA_VERSION,
};
use arc_meter::job_tree::{SampleRow, TreeTotals};
use arc_storage::jobs::JobError;
use rusqlite::{params, OptionalExtension, TransactionBehavior};
use std::time::Instant;

/// Running jobs sampled in one pass, and terminal reports completed in one pass.
const MAX_RUNNING: i64 = 256;
const MAX_FINALIZED: i64 = 64;
/// A terminal snapshot older than this is left as it is.
const FINALIZE_WINDOW_MS: u64 = 60 * 60 * 1000;

pub(crate) struct RunningJob {
    pub job_id: String,
    pub child: ProcessIdentity,
    /// A tree snapshot already exists: earlier samples were not taken here.
    pub resumed: bool,
}

pub(crate) struct Record {
    pub job_id: String,
    pub child: ProcessIdentity,
    pub timestamp_ns: u64,
    pub rows: Vec<SampleRow>,
    pub totals: TreeTotals,
}

pub(crate) struct Pass {
    pub running: Vec<RunningJob>,
    /// A finalized report still waits for its v1 waited usage.
    pub awaiting_waited: bool,
}

pub(crate) enum Operation {
    Running { now_ms: u64 },
    Record(Box<Record>),
}

pub(crate) enum Value {
    Running(Result<Pass, JobError>),
    /// `false`: the job is no longer RUNNING under that child; nothing written.
    Recorded(Result<bool, JobError>),
}

pub(crate) fn execute(connection: &mut rusqlite::Connection, operation: Operation) -> Value {
    match operation {
        Operation::Running { now_ms } => Value::Running(running(connection, now_ms)),
        Operation::Record(record) => Value::Recorded(write(connection, &record)),
    }
}

/// The writer's own committed view of whether any job is active, refreshed
/// after each operation; reading it sends no request.
pub(crate) fn jobs_active(client: &DbClient) -> bool {
    client
        .shared
        .meter
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .active
}

pub(crate) async fn pass(client: &DbClient, now_ms: u64, end: Instant) -> Option<Pass> {
    match jobs::call(
        client,
        jobs::Operation::ResourceSamples(Operation::Running { now_ms }),
        end,
    )
    .await
    {
        Ok(jobs::Value::ResourceSamples(Value::Running(Ok(pass)))) => Some(pass),
        _ => None,
    }
}

pub(crate) async fn record(client: &DbClient, record: Record, end: Instant) -> bool {
    matches!(
        jobs::call(
            client,
            jobs::Operation::ResourceSamples(Operation::Record(Box::new(record))),
            end,
        )
        .await,
        Ok(jobs::Value::ResourceSamples(Value::Recorded(Ok(true))))
    )
}

fn running(connection: &mut rusqlite::Connection, now_ms: u64) -> Result<Pass, JobError> {
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let awaiting_waited = finalize(&tx, now_ms)?;
    let running = {
        let mut statement = tx.prepare(
            "SELECT j.id,l.child_pid,l.child_start_token,l.child_boot_id,
                    s.process_tree_json IS NOT NULL
             FROM jobs j JOIN job_lifecycle l ON l.job_id=j.id
             LEFT JOIN job_meter_summaries s ON s.job_id=j.id
             WHERE j.state='RUNNING' AND l.child_pid IS NOT NULL
             ORDER BY j.id LIMIT ?1",
        )?;
        let rows = statement.query_map([MAX_RUNNING], |row| {
            Ok(RunningJob {
                job_id: row.get(0)?,
                child: ProcessIdentity {
                    pid: row.get(1)?,
                    start_token: row.get(2)?,
                    boot_id: row.get(3)?,
                },
                resumed: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .filter(|job| job.child.is_valid())
            .collect()
    };
    tx.commit()?;
    Ok(Pass {
        running,
        awaiting_waited,
    })
}

/// Complete the snapshots of jobs that left RUNNING. Returns whether one is
/// still waiting for the waited usage of its v1 report.
fn finalize(tx: &rusqlite::Transaction<'_>, now_ms: u64) -> Result<bool, JobError> {
    let pending: Vec<(String, String, Option<String>)> = {
        let mut statement = tx.prepare(
            "SELECT s.job_id,s.process_tree_json,s.report_json
             FROM job_meter_summaries s JOIN jobs j ON j.id=s.job_id
             WHERE s.expired_at_ms IS NULL AND s.terminal_at_ms IS NOT NULL
               AND s.terminal_at_ms>=?1 AND s.process_tree_json IS NOT NULL
               AND j.state NOT IN ('REGISTERED','LAUNCHING','RUNNING')
               AND (json_extract(s.process_tree_json,'$.terminal')=0
                 OR (json_extract(s.process_tree_json,'$.waited_cpu_core_seconds.basis')='unobserved'
                   AND json_extract(s.report_json,'$.cpu_core_seconds.total') IS NOT NULL))
             ORDER BY s.terminal_at_ms,s.job_id LIMIT ?2",
        )?;
        let since = now_ms.saturating_sub(FINALIZE_WINDOW_MS) as i64;
        let rows = statement.query_map(params![since, MAX_FINALIZED], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?;
        rows.collect::<Result<_, _>>()?
    };
    let mut awaiting = false;
    for (job_id, stored, report) in pending {
        let Ok(stored) = serde_json::from_str::<ProcessTreeReport>(&stored) else {
            continue;
        };
        let waited = report.and_then(|json| waited_seconds(&json));
        awaiting |= waited.is_none();
        let parts = Parts::from_report(&stored);
        store(tx, &job_id, &build(&job_id, &parts, true, waited))?;
    }
    Ok(awaiting)
}

fn waited_seconds(json: &str) -> Option<f64> {
    let value = serde_json::from_str(json).ok()?;
    let report = arc_ipc::meter::decode_report(&value).ok()?;
    report
        .cpu_core_seconds
        .total
        .filter(|v| v.is_finite() && *v >= 0.0)
}

fn write(connection: &mut rusqlite::Connection, record: &Record) -> Result<bool, JobError> {
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let current = tx
        .query_row(
            "SELECT 1 FROM jobs j JOIN job_lifecycle l ON l.job_id=j.id
             WHERE j.id=?1 AND j.state='RUNNING' AND l.child_pid=?2
               AND l.child_start_token=?3 AND l.child_boot_id=?4",
            params![
                record.job_id,
                record.child.pid,
                record.child.start_token,
                record.child.boot_id
            ],
            |_| Ok(()),
        )
        .optional()?
        .is_some();
    if !current {
        return Ok(false);
    }
    {
        let mut insert = tx.prepare(
            "INSERT INTO resource_samples(timestamp_ns,job_id,process_identity,cpu_time_ns,
               rss_bytes,disk_read_bytes,disk_write_bytes,attribution_confidence,cpu_core_percent)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        )?;
        let count = |n: u64| i64::try_from(n).ok();
        for row in &record.rows {
            let reading = &row.reading;
            let identity =
                serde_json::to_string(&reading.identity).map_err(|_| JobError::InvalidRequest)?;
            insert.execute(params![
                record.timestamp_ns.to_string(),
                record.job_id,
                identity,
                reading.cpu_time_ns.to_string(),
                count(reading.rss_bytes),
                count(reading.disk_read_bytes),
                count(reading.disk_write_bytes),
                if reading.root() { "A" } else { "C" },
                row.cpu_core_percent.filter(|p| p.is_finite() && *p >= 0.0),
            ])?;
        }
    }
    let parts = Parts::from_totals(&record.totals);
    store(
        &tx,
        &record.job_id,
        &build(&record.job_id, &parts, false, None),
    )?;
    tx.commit()?;
    Ok(true)
}

fn store(
    tx: &rusqlite::Transaction<'_>,
    job_id: &str,
    report: &ProcessTreeReport,
) -> Result<(), JobError> {
    let json = serde_json::to_string(report).map_err(|_| JobError::InvalidRequest)?;
    if json.len() > arc_ipc::meter::job_tree::MAX_JOB_TREE_BYTES {
        return Err(JobError::InvalidRequest);
    }
    tx.execute(
        "UPDATE job_meter_summaries SET process_tree_json=?2
         WHERE job_id=?1 AND expired_at_ms IS NULL",
        params![job_id, json],
    )?;
    Ok(())
}

/// The sampled facts of one tree, from live totals or from a stored snapshot.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Parts {
    pub root_sampled: Option<f64>,
    pub unwaited: Option<f64>,
    pub peak: Option<u64>,
    pub read: Option<u64>,
    pub written: Option<u64>,
    pub samples: u64,
    pub processes: u64,
    pub partial: bool,
}

impl Parts {
    pub(crate) fn from_totals(totals: &TreeTotals) -> Self {
        let seconds = |ns: u64| ns as f64 / 1e9;
        Self {
            root_sampled: totals.root_cpu_ns.map(seconds),
            unwaited: (totals.samples > 0).then(|| seconds(totals.detached_cpu_ns)),
            peak: totals.peak_rss_bytes,
            read: totals.disk_read_bytes,
            written: totals.disk_write_bytes,
            samples: totals.samples,
            processes: totals.processes,
            partial: totals.partial,
        }
    }
    fn from_report(report: &ProcessTreeReport) -> Self {
        Self {
            root_sampled: report.root_sampled_cpu_core_seconds.value,
            unwaited: report.unwaited_cpu_core_seconds.value,
            peak: report.memory_peak_bytes.value,
            read: report.disk_read_bytes.value,
            written: report.disk_write_bytes.value,
            samples: report.samples,
            processes: report.processes,
            partial: report
                .missing_coverage
                .contains(&Reason::UnobservedDescendants),
        }
    }
}

fn sampled<T>(value: Option<T>) -> TreeValue<T> {
    let basis = if value.is_some() {
        TreeBasis::TreeSamples
    } else {
        TreeBasis::Unobserved
    };
    TreeValue { value, basis }
}

/// Every value states its basis; an unobserved value is `null`, never 0.
pub(crate) fn build(
    job_id: &str,
    parts: &Parts,
    terminal: bool,
    waited: Option<f64>,
) -> ProcessTreeReport {
    let total = waited
        .or(parts.root_sampled)
        .zip(parts.unwaited)
        .map(|(base, unwaited)| base + unwaited);
    let mut missing_coverage = Vec::new();
    if parts.partial {
        missing_coverage.push(Reason::UnobservedDescendants);
    }
    if waited.is_none() {
        missing_coverage.push(Reason::NativeUsageUnavailable);
    }
    ProcessTreeReport {
        schema_version: SCHEMA_VERSION,
        job_id: job_id.to_owned(),
        terminal,
        cpu_core_seconds: sampled(total),
        waited_cpu_core_seconds: TreeValue {
            value: waited,
            basis: if waited.is_some() {
                TreeBasis::WaitedRusage
            } else {
                TreeBasis::Unobserved
            },
        },
        root_sampled_cpu_core_seconds: sampled(parts.root_sampled),
        unwaited_cpu_core_seconds: sampled(parts.unwaited),
        memory_peak_bytes: sampled(parts.peak),
        disk_read_bytes: sampled(parts.read),
        disk_write_bytes: sampled(parts.written),
        completeness: if parts.samples == 0 {
            Completeness::Unknown
        } else if missing_coverage.is_empty() {
            Completeness::Complete
        } else {
            Completeness::Partial
        },
        missing_coverage,
        samples: parts.samples,
        processes: parts.processes,
    }
}

#[cfg(test)]
#[path = "resource_samples/tests.rs"]
mod tests;

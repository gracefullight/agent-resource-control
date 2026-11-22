//! Rollup and retention of raw resource samples (lane C, MTR-014, NFR-023).
//!
//! In one IMMEDIATE transaction: raw `resource_samples` older than 72 h are
//! added to `resource_rollups` minute, hour and day buckets (CPU core-seconds,
//! peak RSS, disk writes, distinct jobs; `reclaimed_bytes` stays NULL) and
//! deleted, so each raw row contributes exactly once; minute buckets expire
//! after 30 days, hour and day buckets after one year (SRS 14.5).
//! `timestamp_ns` and `cpu_time_ns` are decimal TEXT, read with
//! CAST(... AS INTEGER); the 72 h cutoff is compared as 19-digit text so the
//! range uses `idx_resource_samples_time`.
//!
//! Values, per bucket:
//! - `cpu_core_seconds` and `disk_write_bytes`: `cpu_time_ns` and
//!   `disk_write_bytes` are cumulative per process (rusage), so each raw row
//!   contributes the growth to the next sample of the same job and process
//!   identity, attributed to the row's own bucket. That pair is counted when
//!   its earlier row is rolled up, so every interval is counted once even
//!   when its later row is still raw. A row without a successor, or without a
//!   value, contributes nothing; a bucket with no known value stays NULL.
//! - `peak_rss_bytes`: the largest sum of `rss_bytes` over the rows of one
//!   sample time (one sampling sweep) in the bucket.
//! - `job_count`: distinct jobs with a sample in the bucket. A pass covers
//!   whole minutes, so a minute bucket is counted once; an hour or day bucket
//!   that straddles two passes counts a job once per pass.
//!
//! One pass rolls at most [`MAX_RAW_BATCH`] rows (rounded to whole minutes),
//! so the writer is never held for an unbounded time; [`RollupReport::more`]
//! asks the caller for another pass.
use crate::jobs::JobResult;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

pub const RAW_RETENTION_MS: u64 = 72 * 60 * 60 * 1000;
pub const MINUTE_RETENTION_MS: u64 = 30 * 24 * 60 * 60 * 1000;
pub const HOUR_DAY_RETENTION_MS: u64 = 365 * 24 * 60 * 60 * 1000;
pub const MAX_RAW_BATCH: i64 = 20_000;
const MINUTE_MS: u64 = 60_000;
const NS_PER_MS: u64 = 1_000_000;
const GRANULARITIES: [(&str, u64); 3] = [
    ("minute", MINUTE_MS),
    ("hour", 60 * MINUTE_MS),
    ("day", 24 * 60 * MINUTE_MS),
];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RollupReport {
    pub raw_rolled_up: u64,
    pub buckets_expired: u64,
    /// Raw rows older than the cutoff remain beyond this pass's batch.
    pub more: bool,
}

fn text_ns(ms: u64) -> String {
    format!("{:019}", ms.saturating_mul(NS_PER_MS))
}

fn floor_minute_ns(text: &str) -> Option<u64> {
    let ms = text.parse::<u64>().ok()? / NS_PER_MS;
    Some(ms - ms % MINUTE_MS)
}

/// The exclusive end of this pass, as a whole minute at or before `cutoff`.
fn batch_end(connection: &Connection, cutoff: u64) -> JobResult<(u64, bool)> {
    let row = |offset: i64| -> JobResult<Option<String>> {
        Ok(connection
            .query_row(
                "SELECT timestamp_ns FROM resource_samples WHERE timestamp_ns<?1
                 AND length(timestamp_ns)=19 ORDER BY timestamp_ns LIMIT 1 OFFSET ?2",
                params![text_ns(cutoff), offset],
                |r| r.get(0),
            )
            .optional()?)
    };
    let Some(limit) = row(MAX_RAW_BATCH)? else {
        return Ok((cutoff, false));
    };
    let first = row(0)?.as_deref().and_then(floor_minute_ns).unwrap_or(0);
    let limit = floor_minute_ns(&limit).unwrap_or(cutoff);
    // A single minute holding more rows than the batch is rolled whole.
    let end = if limit > first {
        limit
    } else {
        first + MINUTE_MS
    }
    .min(cutoff);
    let more = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM resource_samples WHERE timestamp_ns>=?1
         AND timestamp_ns<?2 AND length(timestamp_ns)=19)",
        params![text_ns(end), text_ns(cutoff)],
        |r| r.get(0),
    )?;
    Ok((end, more))
}

pub fn roll_up(connection: &mut Connection, now_ms: u64) -> JobResult<RollupReport> {
    let raw_cutoff = now_ms.saturating_sub(RAW_RETENTION_MS);
    let cutoff = raw_cutoff - raw_cutoff % MINUTE_MS;
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let (end, more) = batch_end(&tx, cutoff)?;
    tx.execute_batch(
        "CREATE TEMP TABLE IF NOT EXISTS rollup_batch(
           ms INTEGER NOT NULL, ts TEXT NOT NULL, job_id TEXT, rss INTEGER,
           cpu_ns INTEGER, write_bytes INTEGER);
         DELETE FROM temp.rollup_batch;",
    )?;
    tx.execute(
        "INSERT INTO temp.rollup_batch(ms,ts,job_id,rss,cpu_ns,write_bytes)
         SELECT CAST(s.timestamp_ns AS INTEGER)/1000000, s.timestamp_ns, s.job_id, s.rss_bytes,
           max(CAST(n.cpu_time_ns AS INTEGER)-CAST(s.cpu_time_ns AS INTEGER),0),
           max(n.disk_write_bytes-s.disk_write_bytes,0)
         FROM resource_samples s LEFT JOIN resource_samples n ON n.id=(
           CASE WHEN s.process_identity IS NULL THEN NULL ELSE (
             SELECT x.id FROM resource_samples x WHERE x.job_id IS s.job_id
               AND x.process_identity=s.process_identity AND x.timestamp_ns>s.timestamp_ns
               AND length(x.timestamp_ns)=19 ORDER BY x.timestamp_ns LIMIT 1) END)
         WHERE s.timestamp_ns<?1 AND length(s.timestamp_ns)=19",
        [text_ns(end)],
    )?;
    let raw = tx.query_row("SELECT count(*) FROM temp.rollup_batch", [], |r| {
        r.get::<_, i64>(0)
    })?;
    for (granularity, width) in GRANULARITIES {
        tx.execute(
            "WITH rows AS (SELECT (ms/?2)*?2 AS bucket, ts, job_id, rss, cpu_ns, write_bytes
               FROM temp.rollup_batch),
             peaks AS (SELECT bucket, max(total) AS peak FROM
               (SELECT bucket, ts, sum(rss) AS total FROM rows GROUP BY bucket, ts) GROUP BY bucket),
             sums AS (SELECT bucket, sum(cpu_ns) AS cpu, sum(write_bytes) AS written,
               count(DISTINCT job_id) AS jobs FROM rows GROUP BY bucket)
             INSERT INTO resource_rollups(granularity,bucket_start_ms,cpu_core_seconds,
               peak_rss_bytes,disk_write_bytes,job_count,reclaimed_bytes)
             SELECT ?1, s.bucket, s.cpu/1e9, p.peak, s.written, s.jobs, NULL
               FROM sums s JOIN peaks p ON p.bucket=s.bucket WHERE true
             ON CONFLICT(granularity,bucket_start_ms) DO UPDATE SET
               cpu_core_seconds=CASE WHEN excluded.cpu_core_seconds IS NULL THEN cpu_core_seconds
                 ELSE coalesce(cpu_core_seconds,0)+excluded.cpu_core_seconds END,
               peak_rss_bytes=CASE WHEN excluded.peak_rss_bytes IS NULL THEN peak_rss_bytes
                 WHEN peak_rss_bytes IS NULL THEN excluded.peak_rss_bytes
                 ELSE max(peak_rss_bytes,excluded.peak_rss_bytes) END,
               disk_write_bytes=CASE WHEN excluded.disk_write_bytes IS NULL THEN disk_write_bytes
                 ELSE coalesce(disk_write_bytes,0)+excluded.disk_write_bytes END,
               job_count=job_count+excluded.job_count",
            params![granularity, width as i64],
        )?;
    }
    let deleted = tx.execute(
        "DELETE FROM resource_samples WHERE timestamp_ns<?1 AND length(timestamp_ns)=19",
        [text_ns(end)],
    )?;
    if deleted as i64 != raw {
        return Err(crate::jobs::JobError::StorageFailure);
    }
    tx.execute("DELETE FROM temp.rollup_batch", [])?;
    let expired = tx.execute(
        "DELETE FROM resource_rollups WHERE
           (granularity='minute' AND bucket_start_ms<?1)
           OR (granularity IN ('hour','day') AND bucket_start_ms<?2)",
        params![
            now_ms.saturating_sub(MINUTE_RETENTION_MS) as i64,
            now_ms.saturating_sub(HOUR_DAY_RETENTION_MS) as i64
        ],
    )?;
    tx.commit()?;
    Ok(RollupReport {
        raw_rolled_up: deleted as u64,
        buckets_expired: expired as u64,
        more,
    })
}

#[cfg(test)]
#[path = "rollup/tests.rs"]
mod tests;

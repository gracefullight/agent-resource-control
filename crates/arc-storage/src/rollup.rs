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
//! - `job_count`: distinct jobs with a sample in the bucket, however many
//!   passes add to it (cycle 35 F2). Each pass records the bucket's jobs in
//!   `resource_rollup_jobs` (migration 0010) and adds only jobs not yet
//!   recorded there, so an hour or day bucket that straddles two passes
//!   counts a job once. A bucket rolled before migration 0010 keeps its
//!   stored count with `job_count_completeness = unknown`; its earlier
//!   members are not known, so a job sampled on both sides of the upgrade
//!   may count once more there, and no membership is invented. Newly created
//!   buckets are `complete`. Membership rows are deleted with their bucket.
//!
//! One pass rolls at most [`MAX_RAW_BATCH`] rows. A durable partial sweep
//! records RSS across a batch boundary, so a dense sample time has the same
//! peak after restart as it would in one transaction.
use crate::jobs::JobResult;
use rusqlite::{params, Connection, TransactionBehavior};

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
    /// Older raw rows or expired buckets/members remain beyond this pass.
    pub more: bool,
}

fn text_ns(ms: u64) -> String {
    format!("{:019}", ms.saturating_mul(NS_PER_MS))
}

pub fn roll_up(connection: &mut Connection, now_ms: u64) -> JobResult<RollupReport> {
    let raw_cutoff = now_ms.saturating_sub(RAW_RETENTION_MS);
    let cutoff = raw_cutoff - raw_cutoff % MINUTE_MS;
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    tx.execute_batch(
        "CREATE TEMP TABLE IF NOT EXISTS rollup_batch(
           id INTEGER PRIMARY KEY, ms INTEGER NOT NULL, ts TEXT NOT NULL, job_id TEXT, rss INTEGER,
           cpu_ns INTEGER, write_bytes INTEGER);
         DELETE FROM temp.rollup_batch;
         CREATE TEMP TABLE IF NOT EXISTS rollup_new_jobs(bucket INTEGER NOT NULL, job_id TEXT NOT NULL);
         CREATE TEMP TABLE IF NOT EXISTS rollup_complete_sweeps(ts TEXT PRIMARY KEY, rss INTEGER NOT NULL);
         DELETE FROM temp.rollup_complete_sweeps;",
    )?;
    tx.execute(
        "INSERT INTO temp.rollup_batch(id,ms,ts,job_id,rss,cpu_ns,write_bytes)
         SELECT s.id, CAST(s.timestamp_ns AS INTEGER)/1000000, s.timestamp_ns, s.job_id, s.rss_bytes,
           max(CAST(n.cpu_time_ns AS INTEGER)-CAST(s.cpu_time_ns AS INTEGER),0),
           max(n.disk_write_bytes-s.disk_write_bytes,0)
         FROM resource_samples s LEFT JOIN resource_samples n ON n.id=(
           CASE WHEN s.process_identity IS NULL THEN NULL ELSE (
             SELECT x.id FROM resource_samples x WHERE x.job_id IS s.job_id
               AND x.process_identity=s.process_identity AND x.timestamp_ns>s.timestamp_ns
               AND length(x.timestamp_ns)=19 ORDER BY x.timestamp_ns LIMIT 1) END)
         WHERE s.id IN (SELECT id FROM resource_samples WHERE timestamp_ns<?1
           AND length(timestamp_ns)=19 ORDER BY timestamp_ns,id LIMIT ?2)",
        params![text_ns(cutoff), MAX_RAW_BATCH],
    )?;
    let raw = tx.query_row("SELECT count(*) FROM temp.rollup_batch", [], |r| {
        r.get::<_, i64>(0)
    })?;
    tx.execute(
        "INSERT INTO resource_rollup_pending_sweeps(ts,rss)
         SELECT ts,sum(rss) FROM temp.rollup_batch WHERE rss IS NOT NULL GROUP BY ts
         ON CONFLICT(ts) DO UPDATE SET rss=rss+excluded.rss",
        [],
    )?;
    for (granularity, width) in GRANULARITIES {
        // The jobs of this pass not yet counted in their bucket.
        tx.execute("DELETE FROM temp.rollup_new_jobs", [])?;
        tx.execute(
            "INSERT INTO temp.rollup_new_jobs(bucket,job_id)
             SELECT DISTINCT (b.ms/?2)*?2, b.job_id FROM temp.rollup_batch b
             WHERE b.job_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM resource_rollup_jobs m
               WHERE m.granularity=?1 AND m.bucket_start_ms=(b.ms/?2)*?2 AND m.job_id=b.job_id)",
            params![granularity, width as i64],
        )?;
        tx.execute(
            "INSERT INTO resource_rollup_jobs(granularity,bucket_start_ms,job_id)
             SELECT ?1, bucket, job_id FROM temp.rollup_new_jobs",
            [granularity],
        )?;
        tx.execute(
            "WITH rows AS (SELECT (ms/?2)*?2 AS bucket, ts, job_id, rss, cpu_ns, write_bytes
               FROM temp.rollup_batch),
             sums AS (SELECT r.bucket, sum(r.cpu_ns) AS cpu, sum(r.write_bytes) AS written,
               (SELECT count(*) FROM temp.rollup_new_jobs n WHERE n.bucket=r.bucket) AS jobs
               FROM rows r GROUP BY r.bucket)
             INSERT INTO resource_rollups(granularity,bucket_start_ms,cpu_core_seconds,
               peak_rss_bytes,disk_write_bytes,job_count,reclaimed_bytes,job_count_completeness)
             SELECT ?1, s.bucket, s.cpu/1e9, NULL, s.written, s.jobs, NULL, 'complete'
               FROM sums s WHERE true
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
        "DELETE FROM resource_samples WHERE id IN (SELECT id FROM temp.rollup_batch)",
        [],
    )?;
    if deleted as i64 != raw {
        return Err(crate::jobs::JobError::StorageFailure);
    }
    // A sweep becomes a peak only after its last raw row has been consumed.
    tx.execute(
        "INSERT INTO temp.rollup_complete_sweeps(ts,rss)
         SELECT p.ts,p.rss FROM resource_rollup_pending_sweeps p
         WHERE NOT EXISTS (SELECT 1 FROM resource_samples s WHERE s.timestamp_ns=p.ts)",
        [],
    )?;
    for (granularity, width) in GRANULARITIES {
        tx.execute(
            "UPDATE resource_rollups AS r SET peak_rss_bytes = CASE
               WHEN r.peak_rss_bytes IS NULL THEN p.peak
               ELSE max(r.peak_rss_bytes,p.peak) END
             FROM (SELECT (CAST(ts AS INTEGER)/1000000/?2)*?2 AS bucket, max(rss) AS peak
               FROM temp.rollup_complete_sweeps GROUP BY bucket) AS p
             WHERE r.granularity=?1 AND r.bucket_start_ms=p.bucket",
            params![granularity, width as i64],
        )?;
    }
    tx.execute("DELETE FROM resource_rollup_pending_sweeps WHERE ts IN (SELECT ts FROM temp.rollup_complete_sweeps)", [])?;
    tx.execute("DELETE FROM temp.rollup_batch", [])?;
    tx.execute("DELETE FROM temp.rollup_new_jobs", [])?;
    tx.execute(
        "DELETE FROM resource_rollup_jobs WHERE rowid IN (
           SELECT rowid FROM resource_rollup_jobs WHERE
             (granularity='minute' AND bucket_start_ms<?1)
             OR (granularity IN ('hour','day') AND bucket_start_ms<?2)
           LIMIT ?3)",
        params![
            now_ms.saturating_sub(MINUTE_RETENTION_MS) as i64,
            now_ms.saturating_sub(HOUR_DAY_RETENTION_MS) as i64,
            MAX_RAW_BATCH
        ],
    )?;
    let expired = tx.execute(
        "DELETE FROM resource_rollups WHERE rowid IN (
           SELECT r.rowid FROM resource_rollups r WHERE
             ((r.granularity='minute' AND r.bucket_start_ms<?1)
              OR (r.granularity IN ('hour','day') AND r.bucket_start_ms<?2))
             AND NOT EXISTS (SELECT 1 FROM resource_rollup_jobs j
               WHERE j.granularity=r.granularity AND j.bucket_start_ms=r.bucket_start_ms)
           LIMIT ?3)",
        params![
            now_ms.saturating_sub(MINUTE_RETENTION_MS) as i64,
            now_ms.saturating_sub(HOUR_DAY_RETENTION_MS) as i64,
            MAX_RAW_BATCH
        ],
    )?;
    let more = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM resource_samples WHERE timestamp_ns<?1 AND length(timestamp_ns)=19)
         OR EXISTS(SELECT 1 FROM resource_rollups WHERE
           (granularity='minute' AND bucket_start_ms<?2)
           OR (granularity IN ('hour','day') AND bucket_start_ms<?3))
         OR EXISTS(SELECT 1 FROM resource_rollup_jobs WHERE
           (granularity='minute' AND bucket_start_ms<?2)
           OR (granularity IN ('hour','day') AND bucket_start_ms<?3))",
        params![text_ns(cutoff), now_ms.saturating_sub(MINUTE_RETENTION_MS) as i64,
            now_ms.saturating_sub(HOUR_DAY_RETENTION_MS) as i64], |r| r.get(0),
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

//! The writer's operations on the interval store: a baseline, a closed
//! interval, a recorded failure and a clean stop's seal, each one immediate
//! transaction ([`IntervalStore::transaction`]).
use super::{floor_of, sql, BootClock, Interval, IntervalStore, ScanPoint, ScannerId, MAX_MS};
use crate::jobs::{JobError, JobResult};
use rusqlite::{params, Transaction};

/// A boot-clock reading as stored: its exact nanoseconds, or none when they
/// do not fit (never a capped, incomparable value).
fn boot_params(boot: &Option<BootClock>) -> (Option<&str>, Option<i64>) {
    match boot
        .as_ref()
        .and_then(|b| Some((b.boot_id.as_str(), i64::try_from(b.ns).ok()?)))
    {
        Some((id, ns)) => (Some(id), Some(ns)),
        None => (None, None),
    }
}

/// [`IntervalStore::mark_failed_from`] inside a writer's transaction.
fn record_failure(
    tx: &Transaction<'_>,
    scanner: &ScannerId,
    failure: Option<(u64, u64)>,
) -> JobResult<()> {
    tx.execute(
        "UPDATE attribution_frontier SET failed=1,sealed=0 WHERE scanner=?1",
        [scanner.as_str()],
    )?;
    if let Some((seq, low_ms)) = failure {
        if seq == 0 || low_ms > MAX_MS {
            return Err(JobError::InvalidRequest);
        }
        tx.execute(
            "UPDATE attribution_frontier SET low_ms=MIN(low_ms,?1)",
            [sql(low_ms)],
        )?;
        tx.execute(
            "INSERT OR IGNORE INTO attribution_frontier(scanner,seq,start_ms,failed,low_ms)
             VALUES(?1,?2,?3,1,?3)",
            params![scanner.as_str(), sql(seq), sql(low_ms)],
        )?;
    }
    Ok(())
}

impl IntervalStore {
    /// [`Self::begin_at`] without a boot-clock reading or unrecorded failure.
    pub fn begin(
        &self,
        scanner: &ScannerId,
        seq: u64,
        scan_start_ms: u64,
        scan_end_ms: u64,
        now_ms: u64,
    ) -> JobResult<()> {
        let start = ScanPoint::wall(scan_start_ms);
        self.begin_at(scanner, seq, &start, scan_end_ms, None, now_ms)
    }

    /// Open a chain at a baseline scan `seq` read within `[start, end_ms]`.
    /// Every frontier, and this scanner's own failure from `unrecorded_from`
    /// that nothing durable records, become one failed interval from the
    /// lowest floor (lowered by [`floor_of`] at `start`; 0 for another
    /// scanner's unsealed fence) to this scan's end (or past the latest
    /// frontier start); this scan is then the scanner's unsealed frontier.
    /// All or nothing.
    pub fn begin_at(
        &self,
        scanner: &ScannerId,
        seq: u64,
        start: &ScanPoint,
        end_ms: u64,
        unrecorded_from: Option<u64>,
        now_ms: u64,
    ) -> JobResult<()> {
        if seq == 0 || start.wall_ms > end_ms || end_ms > MAX_MS {
            return Err(JobError::InvalidRequest);
        }
        self.transaction(now_ms, |tx, frontiers| {
            let latest = frontiers.iter().map(|f| f.start_ms).max();
            let fenced: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM attribution_frontier WHERE sealed=0 AND scanner<>?1)",
                [scanner.as_str()],
                |r| r.get(0),
            )?;
            let floor = frontiers
                .iter()
                .map(|f| f.low_ms)
                .chain(floor_of(frontiers, start))
                .chain(unrecorded_from)
                .chain(fenced.then_some(0))
                .min();
            if let Some(floor) = floor {
                let from = floor.min(start.wall_ms);
                let to = end_ms.max(latest.map_or(0, |l| l + 1)).max(from + 1);
                insert(tx, scanner, seq, from, to, "failed", 0)?;
            }
            tx.execute("DELETE FROM attribution_frontier", [])?;
            let (boot_id, boot_ns) = boot_params(&start.boot);
            tx.execute(
                "INSERT INTO attribution_frontier(scanner,seq,start_ms,failed,low_ms,boot_id,boot_ns)
                 VALUES(?1,?2,?3,0,?3,?4,?5)",
                params![scanner.as_str(), sql(seq), sql(start.wall_ms), boot_id, boot_ns],
            )?;
            Ok(())
        })
    }

    /// [`Self::close_at`] without a boot-clock reading.
    pub fn close(
        &self,
        scanner: &ScannerId,
        prev_seq: u64,
        seq: u64,
        interval: &Interval,
        scan_start_ms: u64,
        now_ms: u64,
    ) -> JobResult<()> {
        let start = ScanPoint::wall(scan_start_ms);
        self.close_at(scanner, prev_seq, seq, interval, &start, now_ms)
    }

    /// Commit the measured interval closed by scan `seq`, whose baseline is
    /// this scanner's open, unfailed frontier `prev_seq`; the frontier moves
    /// to this scan (read from `start`). Anything else commits nothing.
    pub fn close_at(
        &self,
        scanner: &ScannerId,
        prev_seq: u64,
        seq: u64,
        interval: &Interval,
        start: &ScanPoint,
        now_ms: u64,
    ) -> JobResult<()> {
        let finite = |v: f64| v.is_finite() && v >= 0.0;
        let values = interval.proven.iter().chain(&interval.estimated);
        if !finite(interval.unattributed)
            || !values.map(|(_, v)| *v).all(finite)
            || interval.start_ms >= interval.end_ms
            || interval.end_ms > MAX_MS
            || prev_seq == 0
            || seq <= prev_seq
            || !(interval.start_ms..interval.end_ms).contains(&start.wall_ms)
        {
            return Err(JobError::InvalidRequest);
        }
        let rows = interval
            .proven
            .iter()
            .map(|(vendor, v)| ("proven", vendor.as_str(), *v))
            .chain(
                interval
                    .estimated
                    .iter()
                    .map(|(vendor, v)| ("estimated", vendor.as_str(), *v)),
            )
            .chain([("unattributed", "", interval.unattributed)])
            .filter(|(_, _, seconds)| *seconds > 0.0)
            .collect::<Vec<_>>();
        self.transaction(now_ms, |tx, _| {
            let (boot_id, boot_ns) = boot_params(&start.boot);
            let advanced = tx.execute(
                "UPDATE attribution_frontier
                 SET seq=?3,start_ms=?4,low_ms=?4,boot_id=?5,boot_ns=?6,sealed=0
                 WHERE scanner=?1 AND seq=?2 AND failed=0",
                params![
                    scanner.as_str(),
                    sql(prev_seq),
                    sql(seq),
                    sql(start.wall_ms),
                    boot_id,
                    boot_ns
                ],
            )?;
            if advanced != 1 {
                return Err(JobError::Conflict);
            }
            let id = insert(
                tx,
                scanner,
                seq,
                interval.start_ms,
                interval.end_ms,
                "measured",
                interval.skipped,
            )?;
            let mut add = tx.prepare(
                "INSERT INTO attribution_interval_cpu(interval_id,confidence,vendor,cpu_core_seconds)
                 VALUES(?1,?2,?3,?4)",
            )?;
            for (confidence, vendor, seconds) in &rows {
                add.execute(params![id, confidence, vendor, seconds])?;
            }
            Ok(())
        })
    }

    /// Record that this scanner's open frontier failed: the stretch from it
    /// stays failed until a baseline resolves it.
    pub fn mark_failed(&self, scanner: &ScannerId) -> JobResult<()> {
        self.mark_failed_from(scanner, None, 0)
    }

    /// [`Self::mark_failed`], with the failed pass `seq` and the floor
    /// `low_ms` of the unresolved stretch: every frontier's floor is lowered
    /// to it, and without a frontier of its own the scanner gets a failed one
    /// at that floor, so the stretch stays durable.
    pub fn mark_failed_from(
        &self,
        scanner: &ScannerId,
        failure: Option<(u64, u64)>,
        now_ms: u64,
    ) -> JobResult<()> {
        self.transaction(now_ms, |tx, _| record_failure(tx, scanner, failure))
    }

    /// A clean stop: record what the stopped scanner still knew (`failure`,
    /// as [`Self::mark_failed_from`]) and seal its frontier, so its stretch is
    /// unresolved from its recorded floor instead of from 0. Sealing never
    /// resolves the stretch; a seal that cannot be written leaves the fence.
    /// The scanner must not write again.
    pub fn seal(
        &self,
        scanner: &ScannerId,
        failure: Option<(u64, u64)>,
        now_ms: u64,
    ) -> JobResult<()> {
        self.transaction(now_ms, |tx, _| {
            if failure.is_some() {
                record_failure(tx, scanner, failure)?;
            }
            tx.execute(
                "UPDATE attribution_frontier SET sealed=1 WHERE scanner=?1",
                [scanner.as_str()],
            )?;
            Ok(())
        })
    }
}

fn insert(
    tx: &Transaction<'_>,
    scanner: &ScannerId,
    seq: u64,
    start_ms: u64,
    end_ms: u64,
    status: &str,
    skipped: u64,
) -> JobResult<i64> {
    match tx.execute(
        "INSERT INTO attribution_intervals(scanner,seq,start_ms,end_ms,status,skipped_processes)
         VALUES(?1,?2,?3,?4,?5,?6)",
        params![
            scanner.as_str(),
            sql(seq),
            sql(start_ms),
            sql(end_ms),
            status,
            skipped.min(i64::MAX as u64) as i64
        ],
    ) {
        Ok(_) => Ok(tx.last_insert_rowid()),
        // One scan of one scanner closes at most one interval.
        Err(rusqlite::Error::SqliteFailure(error, _))
            if error.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE =>
        {
            Err(JobError::Conflict)
        }
        Err(error) => Err(error.into()),
    }
}

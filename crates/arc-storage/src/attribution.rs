//! Native process-attribution aggregates (lane A: MTR-007, AT-MTR-03,
//! MET-027, NFR-012, R-005 item 2).
//!
//! The daemon's scan reads its Proven roots through [`managed_roots`] and
//! commits one aggregate row per scan interval to the lane's
//! [`IntervalStore`] (`attribution/intervals.rs`); [`section`] is the
//! attribution call site of `meter.usage_summary.v2`
//! (`usage_summary_dispatch::read`). The running scanner registers itself
//! here ([`register_scanner`]) and reports a failure it could not record
//! ([`note_unrecorded_failure`]), so a summary read of the same daemon sees
//! it before any write succeeds. The per-minute tables of migration 0010
//! (`process_attribution`, `process_attribution_scans`) are not written: a
//! minute cannot say which intervals lie inside a window, so any rows an
//! earlier build left make an overlapping window's ratio unavailable.
use crate::jobs::{JobError, JobResult};
use arc_core::process_identity::ProcessIdentity;
use arc_ipc::meter::{
    attribution::{
        AttributionSection, AttributionWindow, VendorCpu, BASIS_SCAN_DELTAS, PROVEN_WITHOUT_VENDOR,
    },
    quality::{Availability, Flagged},
};
use rusqlite::{params, Connection};
use std::{path::Path, sync::Mutex};

#[path = "attribution/intervals.rs"]
mod intervals;
pub use intervals::{
    floor_of, BootClock, Frontier, Interval, IntervalStore, LifecycleGuard, ScanPoint, ScannerId,
    MAX_INTERVALS, RETENTION_MS,
};

/// No measured interval lies wholly inside the window.
pub const REASON_NO_SCAN: &str = "no_scan";
pub const REASON_ZERO_TOTAL: &str = "zero_total";
/// The included intervals leave an unobserved hole.
pub const REASON_MISSING_SCAN: &str = "missing_scan";
/// A failed scan interval or failed frontier overlaps the window.
pub const REASON_FAILED_SCAN: &str = "failed_scan";
/// An unresolved stretch overlaps the window: this daemon's scanner failed
/// and has not recorded it yet, or a stopped scanner left an open frontier
/// (from 0 when it never sealed it on a clean stop).
pub const REASON_PENDING_SCAN: &str = "pending_scan";
/// Minute aggregates without interval bounds overlap the window.
pub const REASON_LEGACY_MINUTES: &str = "legacy_minutes";
/// Version-1 interval bounds (scan completions, not envelopes) may touch
/// the window; they are kept but never counted.
pub const REASON_LEGACY_BOUNDS: &str = "legacy_bounds";
/// The interval store exists but cannot be read.
pub const REASON_STORE_UNAVAILABLE: &str = "store_unavailable";
/// Proven roots read for one scan.
pub const MAX_MANAGED_ROOTS: i64 = 256;
const MAX_MS: u64 = 9_007_199_254_740_991;

/// The scanner running in this process, and the start of a stretch it
/// failed to observe and has not resolved by a committed baseline.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LiveScanner {
    pub scanner: String,
    pub unrecorded_from_ms: Option<u64>,
}

static LIVE: Mutex<Option<LiveScanner>> = Mutex::new(None);

fn live() -> Option<LiveScanner> {
    LIVE.lock().map(|live| live.clone()).unwrap_or(None)
}

/// The daemon's scanner; any other scanner's frontier is unresolved.
pub fn register_scanner(scanner: &ScannerId) {
    if let Ok(mut live) = LIVE.lock() {
        *live = Some(LiveScanner {
            scanner: scanner.as_str().to_owned(),
            unrecorded_from_ms: None,
        });
    }
}

/// A pass failed: everything from `from_ms` on is unresolved until
/// [`clear_unrecorded_failure`]. The earliest start is kept; a wall clock
/// that stepped back reports the floor it implies (`floor_of`).
pub fn note_unrecorded_failure(scanner: &ScannerId, from_ms: u64) {
    if let Ok(mut live) = LIVE.lock() {
        if let Some(live) = live.as_mut().filter(|l| l.scanner == scanner.as_str()) {
            live.unrecorded_from_ms =
                Some(live.unrecorded_from_ms.map_or(from_ms, |f| f.min(from_ms)));
        }
    }
}

/// A baseline committed and turned the stretch into a failed interval.
pub fn clear_unrecorded_failure(scanner: &ScannerId) {
    if let Ok(mut live) = LIVE.lock() {
        if let Some(live) = live.as_mut().filter(|l| l.scanner == scanner.as_str()) {
            live.unrecorded_from_ms = None;
        }
    }
}

/// Lease evidence of one managed root; the scan reads it before and after
/// its native pass and trusts only evidence present, unchanged, in both.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedRootEvidence {
    pub identity: ProcessIdentity,
    pub vendor: String,
    pub job_id: String,
    pub generation: i64,
}

/// A RUNNING job's recorded child whose managed spawn verified ownership,
/// with the job's vendor or [`PROVEN_WITHOUT_VENDOR`]. The Traffic reservation
/// must be active with the same generation; the scan independently revalidates
/// the identity. A RUNNING row alone is not a lease.
pub fn managed_roots(connection: &Connection) -> JobResult<Vec<ManagedRootEvidence>> {
    let mut statement = connection.prepare(
        "SELECT l.child_pid,l.child_start_token,l.child_boot_id,j.vendor,j.id,e.generation
         FROM jobs j JOIN job_lifecycle l ON l.job_id=j.id
         JOIN traffic_entries e ON e.job_id=j.id
         JOIN permits p ON p.job_id=COALESCE(e.root_job_id,e.job_id)
           AND p.generation=e.generation
         WHERE j.state='RUNNING' AND l.child_pid IS NOT NULL
           AND l.child_verified_at_ms IS NOT NULL
           AND e.state='RUNNING' AND p.state='ACTIVE' AND p.released_at IS NULL
         ORDER BY j.id LIMIT ?1",
    )?;
    let rows = statement.query_map([MAX_MANAGED_ROOTS + 1], |row| {
        Ok(ManagedRootEvidence {
            identity: ProcessIdentity {
                pid: row.get(0)?,
                start_token: row.get(1)?,
                boot_id: row.get(2)?,
            },
            vendor: row
                .get::<_, Option<String>>(3)?
                .unwrap_or_else(|| PROVEN_WITHOUT_VENDOR.to_owned()),
            job_id: row.get(4)?,
            generation: row.get(5)?,
        })
    })?;
    let rows = rows.collect::<Result<Vec<_>, _>>()?;
    if rows.len() > MAX_MANAGED_ROOTS as usize || rows.iter().any(|root| !root.identity.is_valid())
    {
        return Err(JobError::IdentityUnavailable);
    }
    Ok(rows)
}

/// The attribution section of `[from_ms, to_ms)` (`meter.usage_summary.v2`).
///
/// Only measured intervals whose envelope lies wholly inside the window are
/// included; `window.first_scan_ms` is the earliest included start and
/// `window.last_scan_ms` the latest included end, and `window.scans` counts
/// the included intervals. Every value and the ratio come from exactly
/// those intervals' deltas. Values are `partial` lower bounds when a process
/// was skipped, the union of included envelopes leaves a hole, a failed
/// interval or an unresolved frontier overlaps the window, or minute
/// aggregates of the earlier store or version-1 interval bounds overlap it;
/// except for skipped processes
/// the ratio is then unavailable with that reason, as it is for a zero
/// total. Nothing included is unavailable (never zero). `connection` is the
/// caller's read snapshot of the main database, which names the store.
pub fn section(
    connection: &Connection,
    from_ms: u64,
    to_ms: u64,
    _now_ms: u64,
) -> JobResult<AttributionSection> {
    section_with(connection, from_ms, to_ms, live().as_ref())
}

/// [`section`] against an explicit running scanner.
pub fn section_with(
    connection: &Connection,
    from_ms: u64,
    to_ms: u64,
    live: Option<&LiveScanner>,
) -> JobResult<AttributionSection> {
    if from_ms >= to_ms || to_ms > MAX_MS {
        return Err(JobError::InvalidRequest);
    }
    let store = connection
        .path()
        .filter(|path| !path.is_empty())
        .and_then(|path| IntervalStore::beside(Path::new(path)));
    let selection = match &store {
        Some(store) => store.select(from_ms, to_ms),
        None => Ok(Default::default()),
    };
    let legacy: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM process_attribution_scans
           WHERE bucket_start_ms<?2 AND bucket_start_ms+60000>?1)",
        params![from_ms as i64, to_ms as i64],
        |row| row.get(0),
    )?;
    let selection = match selection {
        Ok(selection) => selection,
        Err(JobError::Busy) => return Err(JobError::Busy),
        Err(_) => {
            return Ok(unavailable(from_ms, to_ms, REASON_STORE_UNAVAILABLE));
        }
    };
    // The frontiers whose unresolved stretch reaches the window: from its
    // floor, or from 0 for an unsealed fence of a scanner not running here.
    let own = live.map(|live| live.scanner.as_str());
    let open: Vec<_> = selection
        .open
        .iter()
        .filter(|open| open.floor(own) < to_ms)
        .collect();
    let failed = selection.failed > 0 || open.iter().any(|open| open.failed);
    // A frontier no running scanner will close, or this daemon's own
    // unrecorded failure, leaves the stretch from its floor unresolved.
    let lifecycle_pending = match &store {
        Some(store) => match store.lifecycle_pending(own) {
            Ok(pending) => pending,
            Err(JobError::Busy) => return Err(JobError::Busy),
            Err(_) => return Ok(unavailable(from_ms, to_ms, REASON_STORE_UNAVAILABLE)),
        },
        None => false,
    };
    let pending = lifecycle_pending
        || open
            .iter()
            .any(|open| open.conflicted || own != Some(open.scanner.as_str()))
        || live
            .and_then(|live| live.unrecorded_from_ms)
            .is_some_and(|from| from < to_ms);
    let (first, last) = (
        selection.first_start_ms.unwrap_or(from_ms),
        selection.last_end_ms.unwrap_or(to_ms),
    );
    let hole = selection.intervals > 0 && selection.covered_ms < last.saturating_sub(first);
    let gap = if failed {
        Some(REASON_FAILED_SCAN)
    } else if pending {
        Some(REASON_PENDING_SCAN)
    } else if hole {
        Some(REASON_MISSING_SCAN)
    } else if legacy {
        Some(REASON_LEGACY_MINUTES)
    } else if selection.uncertain {
        Some(REASON_LEGACY_BOUNDS)
    } else {
        None
    };
    if selection.intervals == 0 {
        return Ok(unavailable(from_ms, to_ms, gap.unwrap_or(REASON_NO_SCAN)));
    }
    let availability = if selection.skipped == 0 && gap.is_none() {
        Availability::Complete
    } else {
        Availability::Partial
    };
    let flagged = |value: f64| Flagged {
        value: Some(value),
        availability,
        basis: BASIS_SCAN_DELTAS.to_owned(),
        reason: None,
    };
    let mut proven = Vec::new();
    let mut estimated = Vec::new();
    let mut unattributed = 0.0;
    for (confidence, vendor, seconds) in selection.groups {
        let group = |vendor: String| VendorCpu {
            vendor,
            cpu_core_seconds: flagged(seconds),
        };
        match confidence.as_str() {
            "proven" => proven.push(group(vendor)),
            "estimated" => estimated.push(group(vendor)),
            "unattributed" if vendor.is_empty() => unattributed += seconds,
            _ => return Err(JobError::StorageFailure),
        }
    }
    let total = proven
        .iter()
        .chain(&estimated)
        .filter_map(|g| g.cpu_core_seconds.value)
        .sum::<f64>()
        + unattributed;
    let ratio = match gap {
        Some(reason) => Flagged::unavailable(BASIS_SCAN_DELTAS, reason),
        None => ratio(unattributed, total, flagged),
    };
    Ok(AttributionSection {
        window: AttributionWindow {
            from_ms,
            to_ms,
            scans: selection.intervals,
            first_scan_ms: Some(first),
            last_scan_ms: Some(last),
            skipped_processes: selection.skipped,
        },
        proven,
        estimated,
        unattributed: flagged(unattributed),
        unattributed_ratio: ratio,
    })
}

/// `100 * unattributed / total`, unavailable (never 0) for a zero or
/// non-finite total.
fn ratio(unattributed: f64, total: f64, flagged: impl Fn(f64) -> Flagged<f64>) -> Flagged<f64> {
    if total > 0.0 && total.is_finite() {
        flagged((100.0 * unattributed / total).clamp(0.0, 100.0))
    } else {
        Flagged::unavailable(BASIS_SCAN_DELTAS, REASON_ZERO_TOTAL)
    }
}

fn unavailable(from_ms: u64, to_ms: u64, reason: &str) -> AttributionSection {
    let missing = || Flagged::unavailable(BASIS_SCAN_DELTAS, reason);
    AttributionSection {
        window: AttributionWindow {
            from_ms,
            to_ms,
            scans: 0,
            first_scan_ms: None,
            last_scan_ms: None,
            skipped_processes: 0,
        },
        proven: Vec::new(),
        estimated: Vec::new(),
        unattributed: missing(),
        unattributed_ratio: missing(),
    }
}

#[cfg(test)]
#[path = "attribution/fence_tests.rs"]
mod fence_tests;
#[cfg(test)]
#[path = "attribution/merge_conflicts_tests.rs"]
mod merge_conflicts_tests;
#[cfg(test)]
#[path = "attribution/merge_tests.rs"]
mod merge_tests;
#[cfg(test)]
#[path = "attribution/roots_tests.rs"]
mod roots_tests;
#[cfg(test)]
#[path = "attribution/store_tests.rs"]
mod store_tests;
#[cfg(test)]
#[path = "attribution/tests.rs"]
mod tests;
#[cfg(test)]
#[path = "attribution/upgrade_tests.rs"]
mod upgrade_tests;

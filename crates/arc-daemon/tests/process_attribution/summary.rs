//! Summary reads through built `arcctl`, the interval store recompute and
//! the shared assertions of `process_attribution.rs`.
use crate::{
    fixture::{Line, Probe},
    journey::{self, Sandbox},
    runtime::{child_watchdog, Fixture},
};
use arc_core::{
    job_meter::{JobMeterSummary, SummaryAccounting},
    process_identity::ProcessIdentity,
};
use arc_ipc::meter::{attribution::AttributionSection, job_tree::SUMMARY_KEY};
use arc_storage::attribution::IntervalStore;
use rusqlite::params;
use std::{
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

/// Float rounding of one aggregate summed per interval, never a measurement
/// tolerance: every comparison below is an exact subset relation in ns.
pub const ROUNDING: f64 = 1e-9;

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

pub fn identity(pid: u64) -> ProcessIdentity {
    ProcessIdentity::capture(pid as u32).unwrap()
}

/// The attribution section of `arcctl meter summary --by vendor ... --json`.
pub fn attribution(f: &Fixture, s: &Sandbox, cwd: &Path, from: u64, to: u64) -> AttributionSection {
    let output = child_watchdog::output(
        s.arcctl(f, cwd).args([
            "meter",
            "summary",
            "--by",
            "vendor",
            "--from",
            &from.to_string(),
            "--to",
            &to.to_string(),
            "--bucket",
            "hour",
            "--json",
        ]),
        "arcctl meter summary",
    )
    .unwrap();
    assert!(output.status.success(), "{output:?}");
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    // Unattributed CPU is never a vendor group of the job summary.
    for group in value["groups"].as_array().unwrap() {
        let vendor = group["vendor"].as_str();
        assert!(
            matches!(vendor, Some("claude-code" | "cursor" | "none")),
            "{group}"
        );
    }
    let section: AttributionSection = serde_json::from_value(value["attribution"].clone())
        .expect("the daemon advertises meter.attribution.v1");
    assert!(section.is_valid(), "{section:?}");
    assert_eq!((section.window.from_ms, section.window.to_ms), (from, to));
    section
}

/// The abbreviated vendor command uses one captured end and a 24-hour window.
pub fn assert_default_vendor_summary(f: &Fixture, s: &Sandbox, cwd: &Path, store: &IntervalStore) {
    let before = now_ms();
    let output = child_watchdog::output(
        s.arcctl(f, cwd)
            .args(["meter", "summary", "--by", "vendor", "--json"]),
        "default vendor summary",
    )
    .unwrap();
    let after = now_ms();
    assert!(output.status.success(), "{output:?}");
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let query = &value["query"];
    let from = query["from_ms"].as_u64().unwrap();
    let to = query["to_ms"].as_u64().unwrap();
    assert!(before <= to && to <= after, "{query}");
    assert_eq!(to.checked_sub(from), Some(24 * 60 * 60 * 1000));
    assert_eq!(query["by"], "vendor");
    assert_eq!(query["bucket"], "hour");
    let section: AttributionSection = serde_json::from_value(value["attribution"].clone()).unwrap();
    assert!(
        section.is_valid() && section.window.scans >= 2,
        "{section:?}"
    );
    assert_eq!((section.window.from_ms, section.window.to_ms), (from, to));
    assert_own_ratio(&section);
    assert_matches_store(&section, store);
}

/// The job's v1 waited CPU (`wait4` of the managed root), once recorded.
pub fn waited_cpu(f: &Fixture, s: &Sandbox, cwd: &Path, job: &str) -> f64 {
    let mut waited = None;
    journey::eventually("recorded waited job report", || {
        let output = child_watchdog::output(
            s.arcctl(f, cwd)
                .args(["meter", "summary", "--job", job, "--json"]),
            "meter summary --job",
        )
        .unwrap();
        assert!(output.status.success(), "{output:?}");
        let mut value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        value.as_object_mut().unwrap().remove(SUMMARY_KEY);
        let summary: JobMeterSummary = serde_json::from_value(value).unwrap();
        waited = match summary.accounting {
            SummaryAccounting::Recorded { report } => report.cpu_core_seconds.total,
            _ => None,
        };
        waited.is_some()
    });
    waited.unwrap()
}

/// One stored interval of the daemon's interval store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stored {
    pub scanner: String,
    pub start: u64,
    pub end: u64,
    pub measured: bool,
}

pub fn store_db(store: &IntervalStore) -> rusqlite::Connection {
    let db = rusqlite::Connection::open_with_flags(
        store.path(),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    db.busy_timeout(Duration::from_secs(60)).unwrap();
    db
}

pub fn intervals(store: &IntervalStore) -> Vec<Stored> {
    let db = store_db(store);
    let mut statement = db
        .prepare(
            "SELECT scanner,start_ms,end_ms,status FROM attribution_intervals ORDER BY start_ms,id",
        )
        .unwrap();
    let rows = statement
        .query_map([], |row| {
            Ok(Stored {
                scanner: row.get(0)?,
                start: row.get::<_, i64>(1)? as u64,
                end: row.get::<_, i64>(2)? as u64,
                measured: row.get::<_, String>(3)? == "measured",
            })
        })
        .unwrap();
    rows.collect::<Result<_, _>>().unwrap()
}

/// Whether `scanner`'s durable frontier is sealed; `None` without one.
pub fn sealed(store: &IntervalStore, scanner: &str) -> Option<bool> {
    store_db(store)
        .query_row(
            "SELECT sealed FROM attribution_frontier WHERE scanner=?1",
            [scanner],
            |row| row.get::<_, i64>(0),
        )
        .ok()
        .map(|sealed| sealed == 1)
}

/// Per-class sums recomputed from the store for the measured intervals
/// wholly inside `[from, to)`, with their count.
pub fn stored(store: &IntervalStore, from: u64, to: u64) -> (u64, Vec<(String, String, f64)>) {
    let db = store_db(store);
    let range = params![from as i64, to as i64];
    let count: i64 = db
        .query_row(
            "SELECT count(*) FROM attribution_intervals
             WHERE status='measured' AND start_ms>=?1 AND end_ms<=?2",
            range,
            |row| row.get(0),
        )
        .unwrap();
    let mut statement = db
        .prepare(
            "SELECT c.confidence,c.vendor,SUM(c.cpu_core_seconds)
             FROM attribution_interval_cpu c JOIN attribution_intervals i ON i.id=c.interval_id
             WHERE i.status='measured' AND i.start_ms>=?1 AND i.end_ms<=?2
             GROUP BY c.confidence,c.vendor ORDER BY c.confidence,c.vendor",
        )
        .unwrap();
    let rows = statement
        .query_map(range, |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    (count as u64, rows)
}

pub fn group(groups: &[arc_ipc::meter::attribution::VendorCpu], vendor: &str) -> f64 {
    groups
        .iter()
        .find(|g| g.vendor == vendor)
        .and_then(|g| g.cpu_core_seconds.value)
        .unwrap_or_else(|| panic!("no {vendor} group: {groups:?}"))
}

pub fn vendors(groups: &[arc_ipc::meter::attribution::VendorCpu]) -> Vec<String> {
    groups.iter().map(|g| g.vendor.clone()).collect()
}

/// Every shown value is the store's sum over exactly the included intervals.
#[track_caller]
pub fn assert_matches_store(section: &AttributionSection, store: &IntervalStore) {
    let window = &section.window;
    let (count, rows) = stored(store, window.from_ms, window.to_ms);
    assert_eq!(count, window.scans);
    let unattributed = section.unattributed.value.unwrap_or(0.0);
    let mut shown = section.proven.len() + section.estimated.len();
    for (confidence, vendor, seconds) in rows {
        let value = match confidence.as_str() {
            "proven" => group(&section.proven, &vendor),
            "estimated" => group(&section.estimated, &vendor),
            _ => {
                assert_eq!(vendor, "", "unattributed CPU under a vendor");
                shown += 1;
                unattributed
            }
        };
        assert!((value - seconds).abs() < ROUNDING, "{confidence}/{vendor}");
        shown -= 1;
    }
    assert_eq!(shown, 0, "a shown group is absent from the store");
}

/// The ratio is the snapshot's own Unattributed over its own total.
#[track_caller]
pub fn assert_own_ratio(section: &AttributionSection) -> f64 {
    let unattributed = section.unattributed.value.unwrap();
    let total = section
        .proven
        .iter()
        .chain(&section.estimated)
        .map(|g| g.cpu_core_seconds.value.unwrap())
        .sum::<f64>()
        + unattributed;
    let ratio = section.unattributed_ratio.value.expect("available ratio");
    assert!(
        (ratio - 100.0 * unattributed / total).abs() < 1e-9,
        "{ratio}"
    );
    assert!(ratio > 0.0 && ratio <= 100.0, "{ratio}");
    ratio
}

/// A role whose identity changed between scans: each identity's first
/// sighting counted nothing although it had already used CPU, and what was
/// counted for it is exactly its CPU growth after that sighting.
#[track_caller]
pub fn assert_counted_only_after_first_sighting(probe: &Probe, role: &str) {
    let Some(Line::First { cpu_ns: first, .. }) = probe.lines.first() else {
        panic!("{role}: the first line is not a first sighting: {probe:?}");
    };
    assert!(*first > 0, "{role} had used CPU before its first sighting");
    let last = probe.lines.iter().rev().find_map(Probe::cpu_ns).unwrap();
    assert!(probe.scans >= 1, "{role}: {probe:?}");
    assert!(
        probe.lines[1..]
            .iter()
            .all(|line| matches!(line, Line::Counted { .. })),
        "{role}: a later scan of one identity is always counted: {probe:?}"
    );
    assert_eq!(probe.counted_ns(0), last - first, "{role}: {probe:?}");
}

/// Only aggregates are stored, and a zero-total interval, when one was
/// observed, reads unavailable through the CLI.
pub fn assert_aggregates_only(f: &Fixture, s: &Sandbox, cwd: &Path, store: &IntervalStore) {
    // A zero-total interval, when one was observed, reads unavailable
    // through the CLI. The scanner is itself a current-user process whose CPU
    // grows between its own two reads, so one is not expected on a live host;
    // its absence is reported, not replaced by a synthetic interval.
    let zero: Option<(i64, i64)> = store_db(store)
        .query_row(
            "SELECT start_ms,end_ms FROM attribution_intervals i WHERE status='measured'
               AND NOT EXISTS(SELECT 1 FROM attribution_interval_cpu c
                 WHERE c.interval_id=i.id AND c.cpu_core_seconds>0) LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .ok();
    match zero {
        Some((from, to)) => {
            let section = attribution(f, s, cwd, from as u64, to as u64);
            assert_eq!(section.unattributed_ratio.value, None, "{section:?}");
            assert!(section.unattributed_ratio.reason.is_some(), "{section:?}");
        }
        None => eprintln!("process_attribution: no zero-total interval was observed"),
    }
    // Only aggregates: the store holds no process identity or command text.
    let columns: Vec<String> = store_db(store)
        .prepare(
            "SELECT name FROM pragma_table_info('attribution_intervals')
             UNION ALL SELECT name FROM pragma_table_info('attribution_interval_cpu')
             UNION ALL SELECT name FROM pragma_table_info('attribution_frontier')",
        )
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        columns,
        [
            "id",
            "scanner",
            "seq",
            "start_ms",
            "end_ms",
            "status",
            "skipped_processes",
            "interval_id",
            "confidence",
            "vendor",
            "cpu_core_seconds",
            "scanner",
            "seq",
            "start_ms",
            "failed",
            "low_ms",
            "boot_id",
            "boot_ns",
            "sealed"
        ]
    );
}

/// The Proven CPU a probe counted.
pub fn proven(p: &Probe) -> f64 {
    p.classes
        .iter()
        .filter(|c| c.confidence == "proven")
        .map(|c| c.cpu_core_seconds)
        .sum()
}

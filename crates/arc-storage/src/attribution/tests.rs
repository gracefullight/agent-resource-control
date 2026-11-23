//! The interval store and window rules over a migrated file database and
//! its real interval store. The production path (real daemon scans, built
//! arcctl) is `arc-daemon tests/process_attribution.rs`.
use super::*;
use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf};

pub(super) const T0: u64 = 1_767_225_600_000; // 2026-01-01T00:00:00Z, a minute boundary.

fn database() -> Connection {
    let mut connection = Connection::open_in_memory().unwrap();
    crate::migration::migrate(&mut connection, crate::MIGRATIONS).unwrap();
    connection
}

/// A migrated file database with its interval store, removed on drop.
pub(super) struct Files {
    pub(super) root: PathBuf,
    pub(super) db: Connection,
    pub(super) store: IntervalStore,
}
impl Files {
    pub(super) fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "arc-attribution-store-{}-{}",
            std::process::id(),
            arc_core::process_identity::random_job_id().unwrap()
        ));
        fs::create_dir(&root).unwrap();
        let path = root.join("arc.sqlite3");
        let db = crate::open_and_migrate(&path).unwrap();
        let store = IntervalStore::beside(&path).unwrap();
        Self { root, db, store }
    }
    pub(super) fn section(&self, from: u64, to: u64) -> AttributionSection {
        let section = section(&self.db, from, to, to).unwrap();
        assert!(section.is_valid(), "{section:?}");
        section
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        if let Some(dir) = self.store.path().parent() {
            let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o700));
        }
        let _ = fs::set_permissions(self.store.path(), fs::Permissions::from_mode(0o600));
        let _ = fs::remove_dir_all(&self.root);
    }
}

pub(super) fn interval(
    start: u64,
    end: u64,
    proven: f64,
    estimated: f64,
    unattributed: f64,
) -> Interval {
    Interval {
        start_ms: start,
        end_ms: end,
        proven: vec![("claude-code".into(), proven), ("none".into(), 0.0)],
        estimated: vec![("claude-code".into(), estimated)],
        unattributed,
        skipped: 0,
    }
}

pub(super) fn reason(section: &AttributionSection) -> Option<&str> {
    section.unattributed_ratio.reason.as_deref()
}

pub(super) fn scanner() -> ScannerId {
    let id = ScannerId::random().unwrap();
    assert_eq!(id.as_str().len(), 32);
    id
}

pub(super) fn live(scanner: &ScannerId, unrecorded_from_ms: Option<u64>) -> LiveScanner {
    LiveScanner {
        scanner: scanner.as_str().to_owned(),
        unrecorded_from_ms,
    }
}

impl Files {
    pub(super) fn read(
        &self,
        from: u64,
        to: u64,
        live: Option<&LiveScanner>,
    ) -> AttributionSection {
        let section = section_with(&self.db, from, to, live).unwrap();
        assert!(section.is_valid(), "{section:?}");
        section
    }
    /// Scans `(start, end)` of one chain: a baseline, then one interval per
    /// later scan over `[previous start, this end]`, each with `cpu`.
    pub(super) fn chain(
        &self,
        id: &ScannerId,
        first_seq: u64,
        scans: &[(u64, u64)],
        cpu: (f64, f64, f64),
    ) {
        self.store
            .begin(id, first_seq, scans[0].0, scans[0].1, T0)
            .unwrap();
        for (i, pair) in scans.windows(2).enumerate() {
            let seq = first_seq + 1 + i as u64;
            self.store
                .close(
                    id,
                    seq - 1,
                    seq,
                    &interval(pair[0].0, pair[1].1, cpu.0, cpu.1, cpu.2),
                    pair[1].0,
                    T0,
                )
                .unwrap();
        }
    }
}

#[test]
fn an_unobserved_window_is_unavailable_never_zero() {
    let memory = database();
    let unnamed = section(&memory, T0 + 1, T0 + 2, T0 + 3).unwrap();
    assert_eq!(unnamed.window.scans, 0);
    assert_eq!(unnamed.unattributed.reason.as_deref(), Some(REASON_NO_SCAN));
    let files = Files::new();
    let empty = files.section(T0 + 1, T0 + 2);
    assert_eq!((empty.window.from_ms, empty.window.to_ms), (T0 + 1, T0 + 2));
    assert_eq!(empty.window.scans, 0);
    assert!(empty.proven.is_empty() && empty.estimated.is_empty());
    assert_eq!(empty.unattributed.value, None);
    assert_eq!(reason(&empty), Some(REASON_NO_SCAN));
    assert_eq!(
        empty.unattributed_ratio.availability,
        Availability::Unavailable
    );
    assert!(!files.store.path().exists(), "a read creates nothing");
    assert_eq!(
        section(&files.db, T0, T0, T0),
        Err(JobError::InvalidRequest)
    );
}

#[test]
fn overlapping_envelopes_count_each_delta_once_and_cover_their_union() {
    let files = Files::new();
    let id = scanner();
    let me = live(&id, None);
    // Scans read within 10-15, 2010-2014 and 4009-4012 ms: two envelopes
    // [10, 2014) and [2010, 4012) that overlap in 2010-2014.
    files.chain(
        &id,
        1,
        &[
            (T0 + 10, T0 + 15),
            (T0 + 2_010, T0 + 2_014),
            (T0 + 4_009, T0 + 4_012),
        ],
        (1.0, 2.0, 3.0),
    );
    let all = files.read(T0 + 10, T0 + 4_012, Some(&me));
    assert_eq!(all.window.scans, 2);
    assert_eq!(
        (all.window.first_scan_ms, all.window.last_scan_ms),
        (Some(T0 + 10), Some(T0 + 4_012))
    );
    assert_eq!(all.proven.len(), 1, "a zero group is not stored");
    assert_eq!(all.proven[0].cpu_core_seconds.value, Some(2.0));
    assert_eq!(all.estimated[0].cpu_core_seconds.value, Some(4.0));
    assert_eq!(all.unattributed.value, Some(6.0));
    assert_eq!(
        all.unattributed.availability,
        Availability::Complete,
        "overlap is no hole"
    );
    assert_eq!(all.unattributed_ratio.value, Some(50.0));
    // A window cutting through a scan's read drops the envelope holding it.
    let cut_end = files.read(T0 + 10, T0 + 4_011, Some(&me));
    assert_eq!(cut_end.window.scans, 1);
    assert_eq!(cut_end.window.last_scan_ms, Some(T0 + 2_014));
    assert_eq!(cut_end.unattributed.value, Some(3.0));
    let cut_start = files.read(T0 + 11, T0 + 4_012, Some(&me));
    assert_eq!(cut_start.window.scans, 1);
    assert_eq!(cut_start.window.first_scan_ms, Some(T0 + 2_010));
    // Cutting through the second scan, which both envelopes contain.
    assert_eq!(
        files.read(T0 + 2_011, T0 + 4_012, Some(&me)).window.scans,
        0
    );
    assert_eq!(files.read(T0 + 10, T0 + 2_013, Some(&me)).window.scans, 0);
    // A restart's reader sees the same store.
    let reopened = crate::open_and_migrate(files.root.join("arc.sqlite3")).unwrap();
    assert_eq!(
        section_with(&reopened, T0 + 10, T0 + 4_012, Some(&me)).unwrap(),
        all
    );
    // One scan closes at most one interval; a stale baseline closes none.
    let again = interval(T0 + 2_010, T0 + 4_012, 1.0, 0.0, 0.0);
    assert_eq!(
        files.store.close(&id, 2, 3, &again, T0 + 4_009, T0),
        Err(JobError::Conflict)
    );
    let next = interval(T0 + 4_009, T0 + 6_000, 1.0, 0.0, 0.0);
    assert_eq!(
        files.store.close(&id, 2, 4, &next, T0 + 5_990, T0),
        Err(JobError::Conflict)
    );
    assert_eq!(
        files.read(T0, T0 + 10_000, Some(&me)).unattributed.value,
        Some(6.0)
    );
}

#[test]
fn a_failure_is_unresolved_before_recovery_and_after_restart_then_failed() {
    let files = Files::new();
    let id = scanner();
    // Four scans, a failed pass, all in one calendar minute.
    let scans = [
        (T0 + 1_000, T0 + 1_003),
        (T0 + 3_000, T0 + 3_002),
        (T0 + 5_000, T0 + 5_004),
    ];
    files.chain(&id, 1, &scans, (0.0, 0.0, 1.0));
    let before = files.read(T0 + 1_000, T0 + 3_002, Some(&live(&id, None)));
    assert_eq!(before.unattributed_ratio.value, Some(100.0));
    // The failed pass at 7 s could not be recorded: only this process knows.
    let unrecorded = live(&id, Some(T0 + 5_000));
    let across = files.read(T0 + 1_000, T0 + 7_001, Some(&unrecorded));
    assert_eq!(across.window.scans, 2);
    assert_eq!(across.unattributed.value, Some(2.0));
    assert_eq!(across.unattributed.availability, Availability::Partial);
    assert_eq!(reason(&across), Some(REASON_PENDING_SCAN));
    // The frontier is the last scan's start: the window ending at it is not.
    let earlier = files.read(T0 + 1_000, T0 + 5_000, Some(&unrecorded));
    assert_eq!(earlier.unattributed_ratio.value, Some(100.0));
    // After a restart, no running scanner will close the old frontier.
    let other = scanner();
    let restarted = live(&other, None);
    assert_eq!(
        reason(&files.read(T0 + 1_000, T0 + 7_001, Some(&restarted))),
        Some(REASON_PENDING_SCAN)
    );
    assert_eq!(
        reason(&files.read(T0 + 1_000, T0 + 7_001, None)),
        Some(REASON_PENDING_SCAN)
    );
    assert_eq!(
        reason(&files.read(T0 + 6_000, T0 + 7_000, Some(&restarted))),
        Some(REASON_PENDING_SCAN)
    );
    // A durable mark is a failed frontier, whoever reads it.
    files.store.mark_failed(&id).unwrap();
    assert_eq!(
        reason(&files.read(T0 + 1_000, T0 + 7_001, Some(&live(&id, None)))),
        Some(REASON_FAILED_SCAN)
    );
    // Until its stopped scanner seals it, the frontier fences every window
    // from 0 for any other reader, even one ending at the frontier.
    let fenced = files.read(T0 + 1_000, T0 + 5_000, Some(&restarted));
    assert_eq!(fenced.unattributed.availability, Availability::Partial);
    assert_eq!(reason(&fenced), Some(REASON_FAILED_SCAN));
    // A clean stop seals it: the stretch is unresolved from its floor only.
    files.store.seal(&id, None, T0).unwrap();
    for reader in [Some(&restarted), None] {
        let sealed = files.read(T0 + 1_000, T0 + 5_000, reader);
        assert_eq!(sealed.unattributed_ratio.value, Some(100.0));
        assert_eq!(
            reason(&files.read(T0 + 1_000, T0 + 7_001, reader)),
            Some(REASON_FAILED_SCAN)
        );
    }
    // The new scanner's baseline at 9-9.5 s resolves it into a failed interval.
    files.chain(
        &other,
        1,
        &[
            (T0 + 9_000, T0 + 9_500),
            (T0 + 11_000, T0 + 11_002),
            (T0 + 13_000, T0 + 13_001),
        ],
        (0.0, 0.0, 1.0),
    );
    let stored: Vec<(String, i64, i64, String)> = Connection::open(files.store.path())
        .unwrap()
        .prepare("SELECT scanner,start_ms,end_ms,status FROM attribution_intervals ORDER BY id")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let failed = &stored[2];
    assert_eq!(
        (
            failed.0.as_str(),
            failed.1 as u64,
            failed.2 as u64,
            failed.3.as_str()
        ),
        (other.as_str(), T0 + 5_000, T0 + 9_500, "failed")
    );
    let resolved = files.read(T0 + 1_000, T0 + 11_002, Some(&restarted));
    assert_eq!(resolved.window.scans, 3);
    // The failed stretch reaches the baseline's end, which the first
    // interval after it contains; the next one is clear.
    let edge = files.read(T0 + 9_000, T0 + 11_002, Some(&restarted));
    assert_eq!(reason(&edge), Some(REASON_FAILED_SCAN));
    assert_eq!(reason(&resolved), Some(REASON_FAILED_SCAN));
    let after = files.read(T0 + 11_000, T0 + 13_001, Some(&restarted));
    assert_eq!(after.unattributed_ratio.value, Some(100.0));
    assert_eq!(
        after.unattributed_ratio.availability,
        Availability::Complete
    );
    let open: i64 = Connection::open(files.store.path())
        .unwrap()
        .query_row("SELECT count(*) FROM attribution_frontier", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(open, 1, "only the running scanner's frontier is open");
}

#[test]
fn a_hole_between_included_intervals_makes_the_ratio_unavailable() {
    let files = Files::new();
    let id = scanner();
    files.chain(
        &id,
        1,
        &[(T0, T0 + 1), (T0 + 999, T0 + 1_000)],
        (0.0, 0.0, 1.0),
    );
    // An interval no frontier led to (written by hand), after a hole.
    let raw = Connection::open(files.store.path()).unwrap();
    raw.execute(
        "INSERT INTO attribution_intervals(scanner,seq,start_ms,end_ms,status) VALUES(?1,9,?2,?3,'measured')",
        params![id.as_str(), (T0 + 5_000) as i64, (T0 + 6_000) as i64],
    )
    .unwrap();
    let section = files.read(T0, T0 + 6_000, Some(&live(&id, None)));
    assert_eq!(section.window.scans, 2);
    assert_eq!(section.unattributed.availability, Availability::Partial);
    assert_eq!(reason(&section), Some(REASON_MISSING_SCAN));
}

#[test]
fn a_zero_total_and_skipped_processes_are_flagged() {
    let files = Files::new();
    let id = scanner();
    files.store.begin(&id, 1, T0, T0 + 1, T0).unwrap();
    let mut zero = interval(T0, T0 + 2_000, 0.0, 0.0, 0.0);
    zero.skipped = 3;
    files.store.close(&id, 1, 2, &zero, T0 + 1_999, T0).unwrap();
    let section = files.read(T0, T0 + 2_000, Some(&live(&id, None)));
    assert_eq!(section.window.scans, 1);
    assert_eq!(section.window.skipped_processes, 3);
    assert_eq!(section.unattributed.value, Some(0.0));
    assert_eq!(section.unattributed.availability, Availability::Partial);
    assert_eq!(section.unattributed_ratio.value, None);
    assert_eq!(reason(&section), Some(REASON_ZERO_TOTAL));
    let flagged = |v: f64| Flagged {
        value: Some(v),
        availability: Availability::Complete,
        basis: BASIS_SCAN_DELTAS.into(),
        reason: None,
    };
    for total in [0.0, f64::NAN, f64::INFINITY] {
        assert_eq!(
            ratio(0.0, total, flagged).reason.as_deref(),
            Some(REASON_ZERO_TOTAL)
        );
    }
    assert_eq!(ratio(1.0, 4.0, flagged).value, Some(25.0));
}

#[test]
fn legacy_minute_aggregates_are_kept_and_never_given_interval_bounds() {
    let files = Files::new();
    files
        .db
        .execute_batch(&format!(
            "INSERT INTO process_attribution_scans VALUES({T0},2,{},{},0);
             INSERT INTO process_attribution VALUES({T0},'unattributed','',5.0);",
            T0 + 5,
            T0 + 50_000
        ))
        .unwrap();
    let id = scanner();
    let me = live(&id, None);
    let legacy = files.read(T0 + 1_000, T0 + 2_000, Some(&me));
    assert_eq!(legacy.window.scans, 0);
    assert_eq!(legacy.unattributed.value, None);
    assert_eq!(reason(&legacy), Some(REASON_LEGACY_MINUTES));
    files.chain(
        &id,
        1,
        &[(T0 + 1_000, T0 + 1_001), (T0 + 1_999, T0 + 2_000)],
        (0.0, 0.0, 1.0),
    );
    let mixed = files.read(T0 + 1_000, T0 + 2_000, Some(&me));
    assert_eq!(
        mixed.unattributed.value,
        Some(1.0),
        "legacy CPU is not added"
    );
    assert_eq!(reason(&mixed), Some(REASON_LEGACY_MINUTES));
    let count: i64 = files
        .db
        .query_row("SELECT count(*) FROM process_attribution", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1, "legacy rows are preserved");
    let after = files.read(T0 + 60_000, T0 + 61_000, Some(&me));
    assert_eq!(reason(&after), Some(REASON_NO_SCAN));
}

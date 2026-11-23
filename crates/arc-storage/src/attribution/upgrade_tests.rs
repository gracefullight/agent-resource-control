//! Earlier store versions and locations are kept and never overstated, and
//! a wall clock that stepped back keeps the stretch it hid unresolved.
use super::tests::{interval, live, reason, scanner, Files, T0};
use super::*;
use std::{fs, os::unix::fs::PermissionsExt, path::Path};

/// The tables of versions 1 and 2 (version 1 differed only in what its
/// bounds meant: scan completions, not envelopes).
const EARLIER: &str = "
CREATE TABLE attribution_intervals (
  id INTEGER PRIMARY KEY, scanner TEXT NOT NULL, seq INTEGER NOT NULL,
  start_ms INTEGER NOT NULL, end_ms INTEGER NOT NULL, status TEXT NOT NULL,
  skipped_processes INTEGER NOT NULL DEFAULT 0, UNIQUE(scanner, seq));
CREATE INDEX idx_attribution_intervals_end ON attribution_intervals(end_ms);
CREATE INDEX idx_attribution_intervals_start ON attribution_intervals(start_ms);
CREATE TABLE attribution_interval_cpu (
  interval_id INTEGER NOT NULL REFERENCES attribution_intervals(id) ON DELETE CASCADE,
  confidence TEXT NOT NULL, vendor TEXT NOT NULL, cpu_core_seconds REAL NOT NULL,
  PRIMARY KEY(interval_id, confidence, vendor)) WITHOUT ROWID;
CREATE TABLE attribution_frontier (
  scanner TEXT PRIMARY KEY NOT NULL, seq INTEGER NOT NULL, start_ms INTEGER NOT NULL,
  failed INTEGER NOT NULL DEFAULT 0) WITHOUT ROWID;
";

/// A store file written by an earlier build at `file`: one measured
/// interval of `owner` (seq 2) with 2 s Unattributed CPU and its frontier.
fn earlier_store(file: &Path, version: i64, owner: &ScannerId, bounds: (u64, u64), frontier: u64) {
    let dir = file.parent().unwrap();
    fs::create_dir_all(dir).unwrap();
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700)).unwrap();
    let raw = Connection::open(file).unwrap();
    fs::set_permissions(file, fs::Permissions::from_mode(0o600)).unwrap();
    raw.execute_batch(EARLIER).unwrap();
    raw.execute(
        "INSERT INTO attribution_intervals(scanner,seq,start_ms,end_ms,status) VALUES(?1,2,?2,?3,'measured')",
        params![owner.as_str(), bounds.0 as i64, bounds.1 as i64],
    )
    .unwrap();
    raw.execute(
        "INSERT INTO attribution_interval_cpu VALUES(last_insert_rowid(),'unattributed','',2.0)",
        [],
    )
    .unwrap();
    raw.execute(
        "INSERT INTO attribution_frontier VALUES(?1,2,?2,0)",
        params![owner.as_str(), frontier as i64],
    )
    .unwrap();
    raw.pragma_update(None, "user_version", version).unwrap();
}

fn count(store: &IntervalStore, query: &str) -> i64 {
    Connection::open(store.path())
        .unwrap()
        .query_row(query, [], |r| r.get(0))
        .unwrap()
}

#[test]
fn a_version_1_store_is_preserved_and_its_windows_stay_uncertain_after_reopening() {
    let files = Files::new();
    let old = scanner();
    earlier_store(
        files.store.path(),
        1,
        &old,
        (T0 + 1_000, T0 + 3_000),
        T0 + 3_000,
    );
    let me_id = scanner();
    let me = live(&me_id, None);
    // Before any write: nothing of version 1 is counted or called complete.
    let before = files.read(T0, T0 + 5_000, Some(&me));
    assert_eq!(before.window.scans, 0);
    assert_eq!(before.unattributed.value, None);
    assert_eq!(reason(&before), Some(REASON_LEGACY_BOUNDS));
    // An upgrade that cannot complete leaves version 1 whole.
    let broken = Files::new();
    earlier_store(
        broken.store.path(),
        1,
        &old,
        (T0 + 1_000, T0 + 3_000),
        T0 + 3_000,
    );
    Connection::open(broken.store.path())
        .unwrap()
        .execute_batch("ALTER TABLE attribution_frontier DROP COLUMN failed")
        .unwrap();
    assert!(broken
        .store
        .begin(&me_id, 1, T0 + 10_000, T0 + 10_001, T0 + 10_000)
        .is_err());
    assert_eq!(count(&broken.store, "PRAGMA user_version"), 1);
    assert_eq!(
        count(&broken.store, "SELECT count(*) FROM attribution_intervals"),
        1
    );

    // The first write upgrades in its transaction and keeps every row.
    files.chain(
        &me_id,
        1,
        &[
            (T0 + 10_000, T0 + 10_001),
            (T0 + 11_999, T0 + 12_000),
            (T0 + 13_999, T0 + 14_000),
        ],
        (0.0, 0.0, 1.0),
    );
    let check = |store: &IntervalStore| {
        assert_eq!(count(store, "PRAGMA user_version"), 4);
        assert_eq!(
            count(store, "SELECT count(*) FROM attribution_v1_intervals"),
            1
        );
        assert_eq!(
            count(
                store,
                "SELECT CAST(SUM(cpu_core_seconds) AS INTEGER) FROM attribution_v1_interval_cpu"
            ),
            2
        );
        assert_eq!(
            count(store, "SELECT count(*) FROM attribution_v1_frontier"),
            1
        );
        assert_eq!(
            count(
                store,
                "SELECT count(*) FROM attribution_uncertain WHERE from_ms=0"
            ),
            1
        );
    };
    check(&files.store);
    // Its frontier stayed unresolved and, never sealed by an earlier build,
    // became a failed interval from 0.
    assert_eq!(
        count(
            &files.store,
            &format!(
                "SELECT count(*) FROM attribution_intervals
                 WHERE status='failed' AND start_ms=0 AND end_ms={}",
                T0 + 10_001
            )
        ),
        1
    );
    let read_all = |db: &Connection, store_live: &LiveScanner| {
        let legacy = section_with(db, T0 + 1_000, T0 + 2_000, Some(store_live)).unwrap();
        assert_eq!(reason(&legacy), Some(REASON_FAILED_SCAN));
        assert_eq!(legacy.window.scans, 0);
        let clear = section_with(db, T0 + 11_999, T0 + 14_000, Some(store_live)).unwrap();
        assert_eq!(clear.window.scans, 1);
        assert_eq!(clear.unattributed_ratio.value, Some(100.0));
    };
    read_all(&files.db, &me);
    // Reopened, and written again: the same reads, no second upgrade.
    let reopened = crate::open_and_migrate(files.root.join("arc.sqlite3")).unwrap();
    read_all(&reopened, &me);
    files
        .store
        .close(
            &me_id,
            3,
            4,
            &interval(T0 + 13_999, T0 + 16_000, 0.0, 0.0, 1.0),
            T0 + 15_999,
            T0 + 16_000,
        )
        .unwrap();
    check(&files.store);
    read_all(&reopened, &me);
}

#[test]
fn a_version_2_store_keeps_its_envelopes_and_open_frontier() {
    let files = Files::new();
    let id = scanner();
    earlier_store(files.store.path(), 2, &id, (T0, T0 + 2_000), T0 + 1_999);
    let me = live(&id, None);
    let read = files.read(T0, T0 + 2_000, Some(&me));
    assert_eq!((read.window.scans, read.unattributed.value), (1, Some(2.0)));
    assert_eq!(read.unattributed_ratio.value, Some(100.0));
    // The same scanner closes its version-2 frontier after the upgrade.
    files
        .store
        .close(
            &id,
            2,
            3,
            &interval(T0 + 1_999, T0 + 4_000, 0.0, 0.0, 1.0),
            T0 + 3_999,
            T0 + 4_000,
        )
        .unwrap();
    let both = files.read(T0, T0 + 4_000, Some(&me));
    assert_eq!((both.window.scans, both.unattributed.value), (2, Some(3.0)));
    assert_eq!(both.unattributed.availability, Availability::Complete);
}

#[test]
fn the_earlier_location_moves_atomically_or_merges_once() {
    let files = Files::new();
    let earlier = files.store.earlier_directory().to_owned();
    let current = files.store.path().parent().unwrap().to_owned();
    assert_eq!(earlier.file_name().unwrap(), "arc.sqlite3.attribution");
    assert_eq!(current.file_name().unwrap(), ".arc.sqlite3.attribution");
    assert_eq!(earlier.parent(), current.parent());
    let id = scanner();
    let me = live(&id, None);
    files.chain(
        &id,
        1,
        &[(T0, T0 + 1), (T0 + 999, T0 + 1_000)],
        (0.0, 0.0, 1.0),
    );
    // An earlier build's location only: read in place, then moved whole.
    fs::rename(&current, &earlier).unwrap();
    assert_eq!(files.read(T0, T0 + 1_000, Some(&me)).window.scans, 1);
    files.chain(
        &id,
        5,
        &[(T0 + 5_000, T0 + 5_001), (T0 + 5_999, T0 + 6_000)],
        (0.0, 0.0, 1.0),
    );
    assert!(!earlier.exists() && files.store.path().exists());
    let all = files.read(T0, T0 + 6_000, Some(&me));
    assert_eq!(all.window.scans, 2);
    // Both locations: the same rows in both count once, before and after
    // the merge, and a removal that did not happen merges again harmlessly.
    fs::create_dir(&earlier).unwrap();
    fs::set_permissions(&earlier, fs::Permissions::from_mode(0o700)).unwrap();
    fs::copy(files.store.path(), earlier.join("intervals.sqlite3")).unwrap();
    let other = scanner();
    let raw = Connection::open(earlier.join("intervals.sqlite3")).unwrap();
    raw.execute(
        "INSERT INTO attribution_intervals(scanner,seq,start_ms,end_ms,status) VALUES(?1,9,?2,?3,'measured')",
        params![other.as_str(), (T0 + 1_000) as i64, (T0 + 5_000) as i64],
    )
    .unwrap();
    raw.execute(
        "INSERT INTO attribution_interval_cpu VALUES(last_insert_rowid(),'unattributed','',4.0)",
        [],
    )
    .unwrap();
    drop(raw);
    let merged_view = files.read(T0, T0 + 6_000, Some(&me));
    assert_eq!(merged_view.window.scans, 3);
    assert_eq!(merged_view.unattributed.value, Some(6.0));
    let saved = fs::read(earlier.join("intervals.sqlite3")).unwrap();
    files.store.mark_failed(&scanner()).unwrap();
    assert!(!earlier.exists(), "the merged location is removed");
    assert_eq!(files.read(T0, T0 + 6_000, Some(&me)), merged_view);
    fs::create_dir(&earlier).unwrap();
    fs::set_permissions(&earlier, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(earlier.join("intervals.sqlite3"), saved).unwrap();
    fs::set_permissions(
        earlier.join("intervals.sqlite3"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    assert_eq!(files.read(T0, T0 + 6_000, Some(&me)), merged_view);
    files.store.mark_failed(&scanner()).unwrap();
    assert_eq!(files.read(T0, T0 + 6_000, Some(&me)), merged_view);
    assert_eq!(
        count(
            &files.store,
            "SELECT count(*) FROM attribution_intervals WHERE status='measured'"
        ),
        3
    );
    // Nothing in the database's directory starts with its file name.
    for entry in fs::read_dir(&files.root).unwrap() {
        let name = entry.unwrap().file_name().into_string().unwrap();
        assert!(
            !name.starts_with("arc.sqlite3")
                || fs::metadata(files.root.join(&name)).unwrap().is_file(),
            "{name}"
        );
    }
}

fn boot(ns: u64) -> Option<BootClock> {
    Some(BootClock {
        boot_id: "00000000-0000-4000-8000-000000000000".into(),
        ns,
    })
}

#[test]
fn a_backward_clock_step_implies_a_floor_and_fails_closed_without_one() {
    let frontier = Frontier {
        start_ms: T0 + 5_000,
        low_ms: T0 + 5_000,
        boot: boot(1_000_000_000),
    };
    let at = |wall: u64, ns: u64| ScanPoint {
        wall_ms: wall,
        boot: boot(ns),
    };
    // 2 s later by the boot clock the wall clock reads 3 s earlier.
    assert_eq!(
        floor_of(
            std::slice::from_ref(&frontier),
            &at(T0 + 2_000, 3_000_000_000)
        ),
        Some(T0)
    );
    // A wall clock that advanced with the boot clock implies nothing.
    assert_eq!(
        floor_of(
            std::slice::from_ref(&frontier),
            &at(T0 + 7_000, 3_000_000_000)
        ),
        None
    );
    assert_eq!(
        floor_of(
            std::slice::from_ref(&frontier),
            &at(T0 + 6_999, 3_000_000_000)
        ),
        None
    );
    // Another boot session: a clock behind the frontier fails closed.
    let other = Frontier {
        boot: None,
        ..frontier.clone()
    };
    assert_eq!(
        floor_of(std::slice::from_ref(&other), &at(T0 + 4_000, 3_000_000_000)),
        Some(0)
    );
    assert_eq!(floor_of(&[other], &ScanPoint::wall(T0 + 6_000)), None);
    assert_eq!(floor_of(&[], &at(T0, 0)), None);
}

#[test]
fn a_reversed_range_is_unresolved_live_and_durably_then_failed() {
    let files = Files::new();
    let id = scanner();
    // An earlier interval at -50 s; the frontier later at +5 s.
    files.chain(
        &id,
        1,
        &[(T0 - 50_000, T0 - 49_999), (T0 - 48_001, T0 - 48_000)],
        (0.0, 0.0, 1.0),
    );
    let start = |wall: u64, ns: u64| ScanPoint {
        wall_ms: wall,
        boot: boot(ns),
    };
    files
        .store
        .close_at(
            &id,
            2,
            3,
            &interval(T0 - 48_001, T0 + 5_002, 0.0, 0.0, 1.0),
            &start(T0 + 5_000, 1_000_000_000),
            T0,
        )
        .unwrap();
    let window = (T0 - 50_000, T0 - 48_000);
    let me = live(&id, None);
    assert_eq!(
        files
            .read(window.0, window.1, Some(&me))
            .unattributed_ratio
            .value,
        Some(100.0)
    );
    // The clock stepped back to -60 s: the floor reads it from the frontier.
    let reversed = start(T0 - 58_000, 3_000_000_000);
    let floor = files.store.reversal_floor(&reversed).unwrap().unwrap();
    assert_eq!(floor, T0 - 60_000);
    // Before it is recorded only this process knows; the window ending
    // between the reversed time and the old frontier is pending.
    let unrecorded = live(&id, Some(floor));
    assert_eq!(
        reason(&files.read(window.0, window.1, Some(&unrecorded))),
        Some(REASON_PENDING_SCAN)
    );
    // Recorded: every reader, and a restarted daemon, sees it failed.
    files
        .store
        .mark_failed_from(&id, Some((4, floor)), T0)
        .unwrap();
    for reader in [Some(&me), Some(&live(&scanner(), None)), None] {
        assert_eq!(
            reason(&files.read(window.0, window.1, reader)),
            Some(REASON_FAILED_SCAN)
        );
    }
    // Each scanner below stops cleanly: its frontier is sealed at its floor.
    files.store.seal(&id, None, T0).unwrap();
    // The next baseline, after the clock was corrected, keeps the range failed.
    let next = scanner();
    files
        .store
        .begin_at(
            &next,
            1,
            &start(T0 + 70_000, 76_000_000_000),
            T0 + 70_001,
            None,
            T0,
        )
        .unwrap();
    assert_eq!(
        count(&files.store, &format!("SELECT count(*) FROM attribution_intervals WHERE status='failed' AND start_ms={} AND end_ms={}", T0 - 60_000, T0 + 70_001)),
        1
    );
    assert_eq!(
        reason(&files.read(window.0, window.1, Some(&live(&next, None)))),
        Some(REASON_FAILED_SCAN)
    );
    // A failure nothing durable recorded is carried into the next baseline.
    files.store.seal(&next, None, T0).unwrap();
    let third = scanner();
    files
        .store
        .begin_at(
            &third,
            1,
            &start(T0 + 90_000, 96_000_000_000),
            T0 + 90_001,
            Some(T0 - 100_000),
            T0,
        )
        .unwrap();
    assert_eq!(
        count(
            &files.store,
            &format!(
                "SELECT count(*) FROM attribution_intervals WHERE status='failed' AND start_ms={}",
                T0 - 100_000
            )
        ),
        1
    );
    // Without a comparable boot reading, a clock behind the frontier fails
    // closed: the failed interval starts at 0.
    files.store.seal(&third, None, T0).unwrap();
    let fourth = scanner();
    files
        .store
        .begin_at(
            &fourth,
            1,
            &ScanPoint::wall(T0 + 80_000),
            T0 + 80_001,
            None,
            T0,
        )
        .unwrap();
    assert_eq!(
        count(
            &files.store,
            "SELECT count(*) FROM attribution_intervals WHERE status='failed' AND start_ms=0"
        ),
        1
    );
    // A scanner that never committed records its failure as a failed frontier.
    let fresh = Files::new();
    let lone = scanner();
    fresh
        .store
        .mark_failed_from(&lone, Some((1, T0)), T0)
        .unwrap();
    assert_eq!(
        reason(&fresh.read(T0, T0 + 1, Some(&live(&lone, None)))),
        Some(REASON_FAILED_SCAN)
    );
}

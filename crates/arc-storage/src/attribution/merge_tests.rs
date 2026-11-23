//! Both store locations hold distinct histories whose row ids collide, with
//! preserved version-1 rows in each: a merge keeps every row once, parents
//! before children, atomically, across an interrupted write, an interrupted
//! cleanup, a retry and a reopened database.
use super::intervals::KEEP_EARLIER;
use super::tests::{interval, live, reason, scanner, Files, T0};
use super::*;
use std::{fs, os::unix::fs::PermissionsExt, path::Path};

/// Version 1 of the store: the same tables, read as scan-completion bounds.
const VERSION_1: &str = "
CREATE TABLE attribution_intervals (
  id INTEGER PRIMARY KEY, scanner TEXT NOT NULL, seq INTEGER NOT NULL,
  start_ms INTEGER NOT NULL, end_ms INTEGER NOT NULL, status TEXT NOT NULL,
  skipped_processes INTEGER NOT NULL DEFAULT 0, UNIQUE(scanner, seq));
CREATE TABLE attribution_interval_cpu (
  interval_id INTEGER NOT NULL REFERENCES attribution_intervals(id) ON DELETE CASCADE,
  confidence TEXT NOT NULL, vendor TEXT NOT NULL, cpu_core_seconds REAL NOT NULL,
  PRIMARY KEY(interval_id, confidence, vendor)) WITHOUT ROWID;
CREATE TABLE attribution_frontier (
  scanner TEXT PRIMARY KEY NOT NULL, seq INTEGER NOT NULL, start_ms INTEGER NOT NULL,
  failed INTEGER NOT NULL DEFAULT 0) WITHOUT ROWID;
PRAGMA user_version=1;
";

/// A version-1 file at `file` with one interval (id 1) of `owner`.
pub(super) fn version_1(file: &Path, owner: &ScannerId, bounds: (u64, u64), cpu: f64) {
    let dir = file.parent().unwrap();
    fs::create_dir_all(dir).unwrap();
    fs::set_permissions(dir, fs::Permissions::from_mode(0o700)).unwrap();
    let raw = Connection::open(file).unwrap();
    fs::set_permissions(file, fs::Permissions::from_mode(0o600)).unwrap();
    raw.execute_batch(VERSION_1).unwrap();
    raw.execute(
        "INSERT INTO attribution_intervals(id,scanner,seq,start_ms,end_ms,status)
         VALUES(1,?1,2,?2,?3,'measured')",
        params![owner.as_str(), bounds.0 as i64, bounds.1 as i64],
    )
    .unwrap();
    raw.execute(
        "INSERT INTO attribution_interval_cpu VALUES(1,'unattributed','',?1)",
        [cpu],
    )
    .unwrap();
}

/// `(scanner, summed CPU)` per scanner of `cpu` joined to `intervals`.
fn per_scanner(file: &Path, intervals: &str, cpu: &str) -> Vec<(String, f64)> {
    Connection::open(file)
        .unwrap()
        .prepare(&format!(
            "SELECT i.scanner,SUM(c.cpu_core_seconds) FROM {cpu} c
             JOIN {intervals} i ON i.id=c.interval_id GROUP BY i.scanner ORDER BY i.scanner"
        ))
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

pub(super) fn rows(file: &Path, query: &str) -> i64 {
    Connection::open(file)
        .unwrap()
        .query_row(query, [], |r| r.get(0))
        .unwrap()
}

fn sorted(mut pairs: Vec<(String, f64)>) -> Vec<(String, f64)> {
    pairs.sort_by(|a, b| a.0.cmp(&b.0));
    pairs
}

#[test]
fn both_locations_merge_distinct_histories_once_with_colliding_ids() {
    let files = Files::new();
    let (a, b) = (scanner(), scanner());
    let (old_a, old_b) = (scanner(), scanner());
    // The current location: version-1 id 1, then A's intervals ids 1 and 2.
    version_1(files.store.path(), &old_a, (T0 + 1_000, T0 + 3_000), 2.0);
    files.chain(
        &a,
        1,
        &[
            (T0 + 10_000, T0 + 10_001),
            (T0 + 11_999, T0 + 12_000),
            (T0 + 13_999, T0 + 14_000),
        ],
        (0.0, 0.0, 1.0),
    );
    // The earlier location, written apart: the same ids for other rows.
    let apart = Files::new();
    version_1(apart.store.path(), &old_b, (T0 + 500, T0 + 4_000), 5.0);
    apart.chain(
        &b,
        1,
        &[
            (T0 + 20_000, T0 + 20_001),
            (T0 + 21_999, T0 + 22_000),
            (T0 + 23_999, T0 + 24_000),
        ],
        (0.0, 0.0, 4.0),
    );
    apart.store.seal(&b, None, T0).unwrap();
    let earlier = files.store.earlier_directory().join("intervals.sqlite3");
    fs::create_dir(files.store.earlier_directory()).unwrap();
    fs::set_permissions(
        files.store.earlier_directory(),
        fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    fs::copy(apart.store.path(), &earlier).unwrap();
    drop(apart);
    // Multi-file commits are not crash-atomic in WAL mode. Start both files
    // in WAL and verify that a merge actually switches both to DELETE.
    for file in [files.store.path(), earlier.as_path()] {
        let connection = Connection::open(file).unwrap();
        let mode: String = connection
            .query_row("PRAGMA journal_mode=WAL", [], |r| r.get(0))
            .unwrap();
        assert_eq!(mode.to_ascii_lowercase(), "wal");
    }
    // Both files number their measured rows 1 and 2.
    for file in [files.store.path(), earlier.as_path()] {
        let numbered: String = Connection::open(file)
            .unwrap()
            .query_row(
                "SELECT group_concat(id) FROM attribution_intervals WHERE status='measured'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(numbered, "1,2");
        assert_eq!(rows(file, "SELECT id FROM attribution_v1_intervals"), 1);
    }
    let me = live(&a, None);
    let window = (T0 + 10_000, T0 + 24_000);
    let both = files.read(window.0, window.1, Some(&me));
    assert_eq!(
        (both.window.scans, both.unattributed.value),
        (4, Some(10.0))
    );

    // Interrupted before its commit (a stale baseline): nothing moved.
    let next = interval(T0 + 13_999, T0 + 16_000, 0.0, 0.0, 1.0);
    assert_eq!(
        files.store.close(&a, 1, 4, &next, T0 + 15_999, T0),
        Err(JobError::Conflict)
    );
    let measured = "SELECT count(*) FROM attribution_intervals WHERE status='measured'";
    assert_eq!(rows(files.store.path(), measured), 2);
    assert_eq!(rows(&earlier, measured), 2);
    assert_eq!(files.read(window.0, window.1, Some(&me)), both);

    // Committed, but the earlier directory's removal did not happen.
    KEEP_EARLIER.set(true);
    files.store.close(&a, 3, 4, &next, T0 + 15_999, T0).unwrap();
    KEEP_EARLIER.set(false);
    assert!(earlier.exists());
    for file in [files.store.path(), earlier.as_path()] {
        let mode: String = Connection::open(file)
            .unwrap()
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap();
        assert_eq!(mode.to_ascii_lowercase(), "delete");
    }
    let check = |at: &str| {
        let main = files.store.path();
        assert_eq!(rows(main, measured), 5, "{at}");
        assert_eq!(
            sorted(per_scanner(
                main,
                "attribution_intervals",
                "attribution_interval_cpu"
            )),
            sorted(vec![(a.as_str().into(), 3.0), (b.as_str().into(), 8.0)]),
            "{at}"
        );
        assert_eq!(
            sorted(per_scanner(
                main,
                "attribution_v1_intervals",
                "attribution_v1_interval_cpu"
            )),
            sorted(vec![
                (old_a.as_str().into(), 2.0),
                (old_b.as_str().into(), 5.0)
            ]),
            "{at}: both version-1 histories, under their own intervals"
        );
        assert_eq!(rows(main, "SELECT count(*) FROM attribution_uncertain"), 2);
        assert_eq!(
            rows(
                main,
                &format!(
                    "SELECT sealed FROM attribution_frontier WHERE scanner='{}'",
                    b.as_str()
                )
            ),
            1
        );
        let dangling: i64 = Connection::open(main)
            .unwrap()
            .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(dangling, 0, "{at}");
        let read = files.read(window.0, window.1, Some(&me));
        assert_eq!(
            (read.window.scans, read.unattributed.value),
            (5, Some(11.0))
        );
        // Version-1 bounds stay uncertain and uncounted.
        let legacy = files.read(T0 + 500, T0 + 4_000, Some(&me));
        assert_eq!(legacy.window.scans, 0, "{at}");
        assert_eq!(reason(&legacy), Some(REASON_LEGACY_BOUNDS), "{at}");
    };
    check("committed");
    // The earlier file was emptied in the same transaction.
    for table in [
        "attribution_intervals",
        "attribution_interval_cpu",
        "attribution_frontier",
        "attribution_uncertain",
        "attribution_v1_intervals",
        "attribution_v1_interval_cpu",
    ] {
        assert_eq!(rows(&earlier, &format!("SELECT count(*) FROM {table}")), 0);
    }
    // The retry copies nothing twice and removes the earlier directory.
    files.store.mark_failed(&scanner()).unwrap();
    assert!(!files.store.earlier_directory().exists());
    check("retried");
    let reopened = crate::open_and_migrate(files.root.join("arc.sqlite3")).unwrap();
    assert_eq!(
        section_with(&reopened, window.0, window.1, Some(&me)).unwrap(),
        files.read(window.0, window.1, Some(&me))
    );
    check("reopened");
}

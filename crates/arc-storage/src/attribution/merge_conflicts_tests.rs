//! Conflicting sidecar histories and real SQLite crash recovery.
use super::intervals::KEEP_EARLIER;
use super::merge_tests::{rows, version_1};
use super::tests::{interval, live, reason, scanner, Files, T0};
use super::*;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

#[test]
fn colliding_scanners_keep_both_frontiers_and_interval_children() {
    let files = Files::new();
    let apart = Files::new();
    let same = scanner();
    files.chain(
        &same,
        1,
        &[(T0 + 1_000, T0 + 1_001), (T0 + 3_000, T0 + 3_001)],
        (0.0, 0.0, 3.0),
    );
    files.store.seal(&same, None, T0).unwrap();
    apart.chain(
        &same,
        1,
        &[(T0 + 6_000, T0 + 6_001), (T0 + 8_999, T0 + 9_000)],
        (0.0, 0.0, 7.0),
    );
    apart.store.seal(&same, None, T0).unwrap();
    Connection::open(apart.store.path())
        .unwrap()
        .execute(
            "UPDATE attribution_frontier SET seq=3,start_ms=?1,low_ms=?1",
            [(T0 + 10_000) as i64],
        )
        .unwrap();
    let earlier = files.store.earlier_directory().join("intervals.sqlite3");
    fs::create_dir(files.store.earlier_directory()).unwrap();
    fs::set_permissions(
        files.store.earlier_directory(),
        fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    fs::copy(apart.store.path(), &earlier).unwrap();
    let saved = fs::read(&earlier).unwrap();
    let next = scanner();
    let me = live(&next, None);
    let all = files.read(T0 + 1_000, T0 + 9_000, Some(&me));
    assert_eq!((all.window.scans, all.unattributed.value), (2, Some(10.0)));
    let collision_live = live(&same, None);
    let ambiguous = files.read(T0 + 6_000, T0 + 9_000, Some(&collision_live));
    assert_eq!(ambiguous.unattributed.value, Some(7.0));
    assert_eq!(ambiguous.unattributed_ratio.value, None);

    // Roll back an attempted merge, then commit while retaining the emptied
    // earlier file as it would be after an interruption before cleanup.
    let invalid = interval(T0 + 1_000, T0 + 4_000, 0.0, 0.0, 1.0);
    assert_eq!(
        files.store.close(&same, 99, 100, &invalid, T0 + 3_999, T0),
        Err(JobError::Conflict)
    );
    assert_eq!(
        rows(&earlier, "SELECT count(*) FROM attribution_intervals"),
        1
    );
    KEEP_EARLIER.set(true);
    files.store.mark_failed(&next).unwrap();
    KEEP_EARLIER.set(false);
    assert_eq!(
        rows(
            files.store.path(),
            "SELECT count(*) FROM attribution_frontier"
        ),
        2
    );
    let frontier_starts: Vec<i64> = Connection::open(files.store.path())
        .unwrap()
        .prepare("SELECT start_ms FROM attribution_frontier ORDER BY start_ms")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(frontier_starts, [(T0 + 3_000) as i64, (T0 + 10_000) as i64]);
    assert_eq!(
        rows(
            files.store.path(),
            "SELECT count(*) FROM attribution_intervals WHERE status='measured'"
        ),
        2
    );
    assert_eq!(
        rows(
            files.store.path(),
            "SELECT CAST(SUM(cpu_core_seconds) AS INTEGER) FROM attribution_interval_cpu"
        ),
        10
    );
    assert_eq!(
        rows(
            files.store.path(),
            "SELECT count(*) FROM pragma_foreign_key_check"
        ),
        0
    );
    assert_eq!(
        rows(&earlier, "SELECT count(*) FROM attribution_intervals"),
        0
    );
    let before = files.read(T0 + 6_000, T0 + 9_000, Some(&me));
    assert_eq!(before.unattributed_ratio.value, None);
    let before_collision = files.read(T0 + 6_000, T0 + 9_000, Some(&collision_live));
    assert_eq!(before_collision.unattributed_ratio.value, None);
    files
        .store
        .begin(&next, 1, T0 + 5_000, T0 + 5_001, T0)
        .unwrap();
    let raw = Connection::open(files.store.path()).unwrap();
    let (start, end): (i64, i64) = raw.query_row(
        "SELECT start_ms,end_ms FROM attribution_intervals WHERE status='failed' AND scanner=?1",
        [next.as_str()], |r| Ok((r.get(0)?, r.get(1)?)),
    ).unwrap();
    assert!(
        start <= (T0 + 3_000) as i64 && end > (T0 + 10_000) as i64,
        "{start}..{end}"
    );
    drop(raw);
    let after = files.read(T0 + 6_000, T0 + 9_000, Some(&me));
    assert_eq!(after.unattributed_ratio.value, None);
    assert_eq!(reason(&after), Some(REASON_FAILED_SCAN));
    assert_eq!(after.unattributed.value, Some(7.0));
    assert!(!files.store.earlier_directory().exists());
    let reopened = crate::open_and_migrate(files.root.join("arc.sqlite3")).unwrap();
    assert_eq!(
        section_with(&reopened, T0 + 6_000, T0 + 9_000, Some(&me)).unwrap(),
        after
    );
    // A restored earlier copy is reconciled without counting its already
    // imported interval or CPU children for a second time.
    fs::create_dir(files.store.earlier_directory()).unwrap();
    fs::set_permissions(
        files.store.earlier_directory(),
        fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    fs::write(&earlier, saved).unwrap();
    fs::set_permissions(&earlier, fs::Permissions::from_mode(0o600)).unwrap();
    files.store.mark_failed(&scanner()).unwrap();
    assert_eq!(
        rows(
            files.store.path(),
            "SELECT count(*) FROM attribution_intervals WHERE status='measured'"
        ),
        2
    );
    assert_eq!(
        rows(
            files.store.path(),
            "SELECT CAST(SUM(cpu_core_seconds) AS INTEGER) FROM attribution_interval_cpu"
        ),
        10
    );
}

#[test]
fn colliding_version_one_histories_and_frontiers_are_preserved() {
    let files = Files::new();
    let apart = Files::new();
    let same = scanner();
    for (store, bounds, frontier, cpu) in [
        (&files.store, (T0 + 1_000, T0 + 2_000), T0 + 3_000, 3.0),
        (&apart.store, (T0 + 6_000, T0 + 9_000), T0 + 10_000, 7.0),
    ] {
        version_1(store.path(), &same, bounds, cpu);
        Connection::open(store.path())
            .unwrap()
            .execute(
                "INSERT INTO attribution_frontier VALUES(?1,3,?2,0)",
                params![same.as_str(), frontier as i64],
            )
            .unwrap();
    }
    let earlier = files.store.earlier_directory().join("intervals.sqlite3");
    fs::create_dir(files.store.earlier_directory()).unwrap();
    fs::set_permissions(
        files.store.earlier_directory(),
        fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    fs::copy(apart.store.path(), &earlier).unwrap();
    KEEP_EARLIER.set(true);
    files.store.mark_failed_from(&scanner(), None, T0).unwrap();
    KEEP_EARLIER.set(false);
    let main = files.store.path();
    assert_eq!(
        rows(main, "SELECT count(*) FROM attribution_v1_intervals"),
        2
    );
    assert_eq!(
        rows(main, "SELECT count(*) FROM attribution_v1_frontier"),
        2
    );
    assert_eq!(
        rows(
            main,
            "SELECT CAST(SUM(cpu_core_seconds) AS INTEGER) FROM attribution_v1_interval_cpu"
        ),
        10
    );
    assert_eq!(rows(main, "SELECT count(*) FROM attribution_frontier"), 2);
    assert_eq!(
        rows(
            main,
            "SELECT count(*) FROM attribution_uncertain WHERE from_ms=0"
        ),
        2
    );
    let bounds: (i64, i64) = Connection::open(main)
        .unwrap()
        .query_row(
            "SELECT MIN(to_ms),MAX(to_ms) FROM attribution_uncertain",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(bounds, ((T0 + 3_001) as i64, (T0 + 10_001) as i64));
    assert_eq!(
        rows(main, "SELECT count(*) FROM pragma_foreign_key_check"),
        0
    );
    assert_eq!(
        rows(&earlier, "SELECT count(*) FROM attribution_v1_intervals"),
        0
    );
    files.store.mark_failed(&scanner()).unwrap();
    let reopened = crate::open_and_migrate(files.root.join("arc.sqlite3")).unwrap();
    let section = section_with(
        &reopened,
        T0 + 6_000,
        T0 + 9_000,
        Some(&live(&scanner(), None)),
    )
    .unwrap();
    assert_eq!(section.unattributed_ratio.value, None);
}

/// Enter a real merge in a separate process so the parent can kill it while
/// SQLite holds both rollback journals. The ordinary test run does nothing.
#[test]
fn interrupted_merge_child() {
    let Some(database) = std::env::var_os("ARC_ATTRIBUTION_MERGE_CRASH_DB") else {
        return;
    };
    let store = IntervalStore::beside(Path::new(&database)).unwrap();
    store.mark_failed(&scanner()).unwrap();
    panic!("the test process was expected to be killed before commit");
}

struct KillOnDrop(Child);
impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn killed_merge_recovers_both_rollback_journals_before_retry() {
    let files = Files::new();
    let apart = Files::new();
    let (a, b) = (scanner(), scanner());
    files.chain(
        &a,
        1,
        &[(T0 + 1_000, T0 + 1_001), (T0 + 3_000, T0 + 3_001)],
        (0.0, 0.0, 3.0),
    );
    apart.chain(
        &b,
        1,
        &[(T0 + 6_000, T0 + 6_001), (T0 + 8_999, T0 + 9_000)],
        (0.0, 0.0, 7.0),
    );
    let earlier = files.store.earlier_directory().join("intervals.sqlite3");
    fs::create_dir(files.store.earlier_directory()).unwrap();
    fs::set_permissions(
        files.store.earlier_directory(),
        fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    fs::copy(apart.store.path(), &earlier).unwrap();
    let ready = files.root.join("merge-ready");
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .env_clear()
        .env("HOME", &files.root)
        .env(
            "ARC_ATTRIBUTION_MERGE_CRASH_DB",
            files.root.join("arc.sqlite3"),
        )
        .env("ARC_ATTRIBUTION_MERGE_CRASH_READY", &ready)
        .args([
            "--exact",
            "attribution::merge_conflicts_tests::interrupted_merge_child",
            "--nocapture",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile);
    }
    let mut child = KillOnDrop(command.spawn().unwrap());
    let deadline = Instant::now() + Duration::from_secs(60);
    while !ready.exists() {
        assert!(
            child.0.try_wait().unwrap().is_none(),
            "merge child exited before the pause"
        );
        assert!(
            Instant::now() < deadline,
            "merge child did not reach the transaction within 60 seconds"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    for file in [files.store.path(), earlier.as_path()] {
        let journal = format!("{}-journal", file.display());
        assert!(fs::metadata(&journal).unwrap().len() > 0, "{journal}");
    }
    child.0.kill().unwrap();
    assert!(!child.0.wait().unwrap().success());
    // Opening each file read/write triggers SQLite's hot-journal recovery.
    for file in [files.store.path(), earlier.as_path()] {
        let connection = Connection::open(file).unwrap();
        let mode: String = connection
            .query_row("PRAGMA journal_mode", [], |r| r.get(0))
            .unwrap();
        assert_eq!(mode.to_ascii_lowercase(), "delete");
        let count: i64 = connection
            .query_row(
                "SELECT count(*) FROM attribution_intervals WHERE status='measured'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
        let dangling: i64 = connection
            .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(dangling, 0);
    }
    files.store.mark_failed(&scanner()).unwrap();
    assert!(!earlier.exists());
    assert_eq!(
        rows(
            files.store.path(),
            "SELECT count(*) FROM attribution_intervals WHERE status='measured'"
        ),
        2
    );
    assert_eq!(
        rows(
            files.store.path(),
            "SELECT CAST(SUM(cpu_core_seconds) AS INTEGER) FROM attribution_interval_cpu"
        ),
        10
    );
}

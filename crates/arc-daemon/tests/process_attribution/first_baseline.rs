//! Three daemon lifetimes around an unwritable first baseline.
use crate::{
    fixture::{self, Line, Scan},
    journey::{self, Sandbox},
    phases,
    runtime::{child_watchdog, Fixture},
    summary::*,
    tree,
};
use arc_ipc::meter::quality::Availability;
use arc_storage::attribution::IntervalStore;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Stdio,
};

/// Read the separate admission witness, never the interval store's frontier.
fn witness(f: &Fixture) -> (String, bool) {
    let name = f.database.file_name().unwrap().to_str().unwrap();
    let path = f
        .database
        .parent()
        .unwrap()
        .join(format!(".{name}.attribution-lifecycle.sqlite3"));
    let db =
        rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    db.busy_timeout(std::time::Duration::from_secs(60)).unwrap();
    db.query_row(
        "SELECT scanner,clean FROM scanner_lifecycle WHERE id=1",
        [],
        |row| Ok((row.get(0)?, row.get::<_, i64>(1)? == 1)),
    )
    .unwrap()
}

/// Verify the attempted write really is refused by SQLite, independently of
/// the witness database that remains writable for scanner admission.
fn assert_store_write_blocked(store: &IntervalStore, old: &str) {
    assert_eq!(
        fs::metadata(store.path()).unwrap().permissions().mode() & 0o777,
        0o400
    );
    let db = rusqlite::Connection::open_with_flags(
        store.path(),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE,
    )
    .unwrap();
    let error = db
        .execute(
            "UPDATE attribution_frontier SET sealed=sealed WHERE scanner=?1",
            [old],
        )
        .unwrap_err();
    assert_eq!(
        error.sqlite_error_code(),
        Some(rusqlite::ErrorCode::ReadOnly)
    );
}

fn job(
    f: &Fixture,
    s: &Sandbox,
    project: &Path,
    life: &fixture::Lifeline,
    name: &str,
    socket: &str,
) -> (child_watchdog::Owned, PathBuf) {
    let directory = tree::directory(f, name);
    let mut command = s.arcctl(f, project);
    life.arm(&mut command)
        .args(["exec", "--", fixture::PROGRAM, "build"])
        .env("ARC_ATTRIBUTION_DIR", &directory)
        .env("ARC_ATTRIBUTION_SELF_OBSERVE", f.root.join(socket))
        .stdin(Stdio::null());
    let child = child_watchdog::Owned::spawn(&mut command).unwrap();
    let root = tree::published(&directory, "root.pid");
    tree::running_job(&journey::database(f), root);
    (child, directory)
}

fn stop_job(mut child: child_watchdog::Owned, directory: &Path) {
    tree::release(directory);
    assert!(child.wait("first-baseline fixture job").unwrap().success());
}

pub fn failed_first_baseline_after_clean_stop_keeps_the_earlier_window_unavailable(
    f: &Fixture,
    s: &Sandbox,
    project: &Path,
    life: &fixture::Lifeline,
    store: &IntervalStore,
    old: &str,
    window: (u64, u64),
) {
    let frontier: i64 = store_db(store)
        .query_row(
            "SELECT start_ms FROM attribution_frontier WHERE scanner=?1",
            [old],
            |r| r.get(0),
        )
        .unwrap();
    assert!(
        window.1 < frontier as u64,
        "earlier window {window:?} must end before sealed frontier {frontier}"
    );
    assert_eq!(sealed(store, old), Some(true));
    let old_intervals = intervals(store);
    assert!(old_intervals.iter().any(|interval| {
        interval.scanner == old && interval.measured && (interval.start, interval.end) == window
    }));
    assert_eq!(
        witness(f),
        (old.to_owned(), true),
        "prior lifetime was clean"
    );
    fs::remove_file(f.root.join("runtime.json")).unwrap();

    // No baseline of the second daemon can be persisted. Its startup sees
    // the normal clock; only a later failed pass can lower the live floor
    // into the earlier measured window after the test shifts the clock.
    phases::protect(store, true);
    assert_store_write_blocked(store, old);
    let mut second = fixture::daemon(f, "first-baseline-2.sock");
    let mut admitted = None;
    journey::eventually("second scanner admits its own unclean lifetime", || {
        let current = witness(f);
        if current.0 != old && !current.1 {
            admitted = Some(current);
        }
        admitted.is_some()
    });
    let admitted = admitted.unwrap();
    let shift = now_ms() - window.0 + 30_000;
    fs::write(fixture::wall_shift(f), format!("-{shift}\n")).unwrap();
    let (two, two_dir) = job(
        f,
        s,
        project,
        life,
        "first-baseline-2",
        "first-baseline-2.sock",
    );
    let request = fixture::observation(&two_dir, "root");
    let mut failed_scan: Option<Scan> = None;
    journey::eventually(
        "native scan followed by failed first-baseline persistence",
        || {
            failed_scan = fixture::probe(&request).and_then(|probe| {
                probe.lines.windows(3).find_map(|lines| match lines {
                    [Line::BaselineScanned(scan), Line::BaselineWriteFailed {
                        scan: failed,
                        cause,
                    }, Line::Failed { seq, at_ms }]
                        if scan == failed
                            && cause == "storage"
                            && *seq == scan.seq
                            && scan.end_ms < window.0
                            && *at_ms < window.0
                            && at_ms.saturating_add(1) >= scan.end_ms =>
                    {
                        Some(*scan)
                    }
                    _ => None,
                })
            });
            failed_scan.is_some()
        },
    );
    let failed_scan = failed_scan.unwrap();
    assert!(failed_scan.seq > 0 && failed_scan.start_ms <= failed_scan.end_ms);
    eprintln!("first-baseline: prior witness clean; pass {} scanned [{}, {}] before {:?} and its baseline write failed with storage error", failed_scan.seq, failed_scan.start_ms, failed_scan.end_ms, window);
    assert_eq!(witness(f), admitted);
    let second_frontiers: i64 = store_db(store)
        .query_row(
            "SELECT count(*) FROM attribution_frontier WHERE scanner=?1",
            [&admitted.0],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        second_frontiers, 0,
        "no baseline of the second scanner committed"
    );
    let reversed = attribution(f, s, project, window.0, window.1);
    assert_eq!(reversed.unattributed_ratio.value, None);
    assert_eq!(
        reversed.unattributed_ratio.reason.as_deref(),
        Some("pending_scan")
    );
    assert_eq!(intervals(store), old_intervals);
    fs::remove_file(fixture::wall_shift(f)).unwrap();
    let corrected = attribution(f, s, project, window.0, window.1);
    assert_eq!(corrected.unattributed_ratio.value, None);
    assert_eq!(
        corrected.unattributed_ratio.reason.as_deref(),
        Some("pending_scan")
    );
    stop_job(two, &two_dir);
    second.stop(libc::SIGTERM);
    assert_eq!(
        witness(f),
        admitted,
        "failed seal leaves this witness unclean"
    );
    assert_eq!(sealed(store, old), Some(true));
    assert_eq!(intervals(store), old_intervals);
    fs::remove_file(f.root.join("runtime.json")).unwrap();

    let mut third = fixture::daemon(f, "first-baseline-3.sock");
    let mut third_witness = None;
    journey::eventually("third scanner admits its own unclean lifetime", || {
        let current = witness(f);
        if current.0 != admitted.0 && !current.1 {
            third_witness = Some(current);
        }
        third_witness.is_some()
    });
    let third_witness = third_witness.unwrap();
    assert_eq!(sealed(store, old), Some(true));
    assert_eq!(intervals(store), old_intervals);
    let before = attribution(f, s, project, window.0, window.1);
    assert_eq!(before.window.scans, 1);
    assert!(before.unattributed.value.is_some_and(|value| value > 0.0));
    assert_matches_store(&before, store);
    phases::protect(store, false);
    let (three, three_dir) = job(
        f,
        s,
        project,
        life,
        "first-baseline-3",
        "first-baseline-3.sock",
    );
    let mut recovery = None;
    journey::eventually(
        "new scanner commits its native baseline after recovery",
        || {
            recovery = intervals(store)
                .iter()
                .find(|i| !i.measured && !old_intervals.contains(i))
                .cloned();
            recovery.is_some()
        },
    );
    let recovery = recovery.unwrap();
    assert_ne!(recovery.scanner, old);
    assert_eq!(recovery.start, 0);
    assert!(recovery.end > window.1);
    let recovered_probe = fixture::observation(&three_dir, "root");
    journey::eventually(
        "third scanner observes its own live fixture after writes recover",
        || {
            fixture::probe(&recovered_probe)
                .is_some_and(|probe| probe.lines.iter().any(|line| line.scan().is_some()))
        },
    );
    let after = attribution(f, s, project, window.0, window.1);
    assert_eq!(after.window.scans, 1);
    assert!(after.unattributed.value.is_some_and(|value| value > 0.0));
    assert_matches_store(&after, store);
    stop_job(three, &three_dir);
    third.stop(libc::SIGTERM);
    assert_eq!(witness(f), (third_witness.0, true));
    f.report();
    assert!(
        before.unattributed_ratio.value.is_none() && after.unattributed_ratio.value.is_none(),
        "the pre-frontier window regained a ratio: while blocked={before:?}; after recovery={after:?}"
    );
    assert_eq!(
        before.unattributed_ratio.reason.as_deref(),
        Some("pending_scan")
    );
    assert_eq!(
        after.unattributed_ratio.reason.as_deref(),
        Some("failed_scan")
    );
    assert_eq!(before.unattributed.availability, Availability::Partial);
    assert_eq!(after.unattributed.availability, Availability::Partial);
}

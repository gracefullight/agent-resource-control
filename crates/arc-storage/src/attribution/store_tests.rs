//! The interval store's own safety: invalid, unwritable, outdated and
//! expired data commit or show nothing.
use super::tests::{interval, live, reason, scanner, Files, T0};
use super::*;
use std::{fs, os::unix::fs::PermissionsExt, path::Path};

#[test]
fn invalid_or_unwritable_intervals_commit_nothing() {
    let files = Files::new();
    let id = scanner();
    let me = live(&id, None);
    files.store.begin(&id, 1, T0, T0 + 1, T0).unwrap();
    for bad in [f64::NAN, -1.0, f64::INFINITY] {
        assert_eq!(
            files.store.close(
                &id,
                1,
                2,
                &interval(T0, T0 + 1_000, 0.0, 0.0, bad),
                T0 + 999,
                T0
            ),
            Err(JobError::InvalidRequest)
        );
    }
    let empty = interval(T0 + 1, T0 + 1, 0.0, 0.0, 1.0);
    assert_eq!(
        files.store.close(&id, 1, 2, &empty, T0 + 1, T0),
        Err(JobError::InvalidRequest)
    );
    let outside = interval(T0, T0 + 1_000, 0.0, 0.0, 1.0);
    assert_eq!(
        files.store.close(&id, 1, 2, &outside, T0 + 1_000, T0),
        Err(JobError::InvalidRequest),
        "the next baseline lies inside the envelope"
    );
    assert_eq!(
        files.store.begin(&id, 0, T0, T0 + 1, T0),
        Err(JobError::InvalidRequest)
    );
    let mut cursor = interval(T0, T0 + 1_000, 1.0, 0.0, 0.0);
    cursor.estimated = vec![("none".into(), 1.0)];
    assert!(
        files.store.close(&id, 1, 2, &cursor, T0 + 999, T0).is_err(),
        "estimated never names none"
    );
    files
        .store
        .close(
            &id,
            1,
            2,
            &interval(T0, T0 + 1_000, 1.0, 0.0, 1.0),
            T0 + 999,
            T0,
        )
        .unwrap();
    // A write-protected store is a real write failure and commits nothing.
    fs::set_permissions(files.store.path(), fs::Permissions::from_mode(0o400)).unwrap();
    let blocked = interval(T0 + 999, T0 + 2_000, 1.0, 0.0, 1.0);
    assert!(files
        .store
        .close(&id, 2, 3, &blocked, T0 + 1_999, T0)
        .is_err());
    assert!(files.store.mark_failed(&id).is_err());
    assert!(files
        .store
        .begin(&id, 3, T0 + 1_999, T0 + 2_000, T0)
        .is_err());
    fs::set_permissions(files.store.path(), fs::Permissions::from_mode(0o600)).unwrap();
    let section = files.read(T0, T0 + 2_000, Some(&me));
    assert_eq!(section.window.scans, 1);
    assert_eq!(section.unattributed.value, Some(1.0));
    assert_eq!(section.unattributed.availability, Availability::Complete);
    // An open store directory is refused for writing and reading.
    let dir = files.store.path().parent().unwrap().to_owned();
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(
        files.store.close(&id, 2, 3, &blocked, T0 + 1_999, T0),
        Err(JobError::StorageFailure)
    );
    let refused = files.read(T0, T0 + 2_000, Some(&me));
    assert_eq!(reason(&refused), Some(REASON_STORE_UNAVAILABLE));
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
}

#[test]
fn old_intervals_are_deleted_by_a_later_write() {
    let files = Files::new();
    let id = scanner();
    let me = live(&id, None);
    files.chain(
        &id,
        1,
        &[(T0, T0 + 1), (T0 + 999, T0 + 1_000)],
        (0.0, 0.0, 1.0),
    );
    let later = T0 + RETENTION_MS + 10_000;
    files
        .store
        .close(
            &id,
            2,
            3,
            &interval(T0 + 999, later, 0.0, 0.0, 1.0),
            later - 1,
            later,
        )
        .unwrap();
    assert_eq!(files.read(T0, T0 + 1_000, Some(&me)).window.scans, 0);
    assert_eq!(files.read(T0 + 999, later, Some(&me)).window.scans, 1);
    let cpu_rows: i64 = Connection::open(files.store.path())
        .unwrap()
        .query_row("SELECT count(*) FROM attribution_interval_cpu", [], |r| {
            r.get(0)
        })
        .unwrap();
    assert_eq!(cpu_rows, 1, "the deleted interval's CPU rows go with it");
}

#[test]
fn the_store_is_beside_the_database_and_names_one_file_for_either_path_form() {
    let files = Files::new();
    let direct = files.root.join("arc.sqlite3");
    let indirect = files.root.join(".").join("arc.sqlite3");
    assert_eq!(
        IntervalStore::beside(&direct),
        IntervalStore::beside(&indirect)
    );
    let store = IntervalStore::beside(&direct).unwrap();
    assert!(store
        .path()
        .ends_with(Path::new(".arc.sqlite3.attribution/intervals.sqlite3")));
    assert_eq!(IntervalStore::beside(Path::new("/")), None);
}

#[test]
fn codex_process_attribution_is_stored_and_read_with_existing_groups() {
    let files = Files::new();
    let id = scanner();
    files.store.begin(&id, 1, T0, T0 + 1, T0).unwrap();
    let mut values = interval(T0, T0 + 1000, 1.0, 0.0, 2.0);
    values.proven.push(("codex".into(), 3.0));
    values.estimated.push(("codex".into(), 4.0));
    files.store.close(&id, 1, 2, &values, T0 + 999, T0).unwrap();
    let section = files.read(T0, T0 + 1000, Some(&live(&id, None)));
    assert_eq!(
        section
            .proven
            .iter()
            .find(|g| g.vendor == "codex")
            .unwrap()
            .cpu_core_seconds
            .value,
        Some(3.0)
    );
    assert_eq!(
        section
            .estimated
            .iter()
            .find(|g| g.vendor == "codex")
            .unwrap()
            .cpu_core_seconds
            .value,
        Some(4.0)
    );
    assert_eq!(section.unattributed.value, Some(2.0));
}

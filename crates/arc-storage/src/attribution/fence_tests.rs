//! A stopped scanner's unsealed frontier fences every earlier window, a
//! clean stop's seal lifts the fence, and boot-clock readings past 2^53 ns
//! are kept exact while capped ones never imply certainty.
use super::tests::{interval, live, reason, scanner, Files, T0};
use super::*;
use std::{fs, os::unix::fs::PermissionsExt};

const SECOND_NS: u64 = 1_000_000_000;
const DAY_NS: u64 = 86_400 * SECOND_NS;
const CAP_NS: u64 = 9_007_199_254_740_991;

fn at(wall_ms: u64, ns: u64) -> ScanPoint {
    ScanPoint {
        wall_ms,
        boot: Some(BootClock {
            boot_id: "00000000-0000-4000-8000-000000000000".into(),
            ns,
        }),
    }
}

/// Three scans of `id` at 1, 3 and 5 s (boot clock in step): intervals
/// [1 s, 3.002 s) and [3 s, 5.004 s), frontier at 5 s.
fn three_scans(files: &Files, id: &ScannerId, boot_ns: u64) {
    files
        .store
        .begin_at(id, 1, &at(T0 + 1_000, boot_ns), T0 + 1_003, None, T0)
        .unwrap();
    for (seq, (from, to, start)) in [(2, (1_000, 3_002, 3_000)), (3, (3_000, 5_004, 5_000))] {
        files
            .store
            .close_at(
                id,
                seq - 1,
                seq,
                &interval(T0 + from, T0 + to, 0.0, 0.0, 1.0),
                &at(T0 + start, boot_ns + start * 1_000_000),
                T0,
            )
            .unwrap();
    }
}

fn failed_from(files: &Files) -> Vec<i64> {
    Connection::open(files.store.path())
        .unwrap()
        .prepare("SELECT start_ms FROM attribution_intervals WHERE status='failed' ORDER BY id")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

#[test]
fn an_unsealed_frontier_fences_every_earlier_window_after_a_corrected_clock() {
    let files = Files::new();
    let old = scanner();
    three_scans(&files, &old, SECOND_NS);
    let window = (T0 + 1_000, T0 + 3_002);
    assert_eq!(
        files
            .read(window.0, window.1, Some(&live(&old, None)))
            .unattributed_ratio
            .value,
        Some(100.0)
    );
    // The outage: the old scanner can neither record a failure nor seal.
    fs::set_permissions(files.store.path(), fs::Permissions::from_mode(0o400)).unwrap();
    assert!(files
        .store
        .mark_failed_from(&old, Some((4, T0 - 60_000)), T0)
        .is_err());
    assert!(files.store.seal(&old, None, T0).is_err());
    // The clock was corrected before the restart: nothing shows a step.
    let restart = at(T0 + 20_000, 20 * SECOND_NS);
    assert_eq!(files.store.reversal_floor(&restart).unwrap(), None);
    let next = scanner();
    for reader in [Some(&live(&next, None)), None] {
        let section = files.read(window.0, window.1, reader);
        assert_eq!(section.window.scans, 1);
        assert_eq!(section.unattributed.value, Some(1.0));
        assert_eq!(section.unattributed.availability, Availability::Partial);
        assert_eq!(section.unattributed_ratio.value, None);
        assert_eq!(reason(&section), Some(REASON_PENDING_SCAN));
    }
    // Writable again: the next baseline's failed interval starts at 0.
    fs::set_permissions(files.store.path(), fs::Permissions::from_mode(0o600)).unwrap();
    files
        .store
        .begin_at(&next, 1, &restart, T0 + 20_001, None, T0)
        .unwrap();
    assert_eq!(failed_from(&files), [0]);
    let resolved = files.read(window.0, window.1, Some(&live(&next, None)));
    assert_eq!(reason(&resolved), Some(REASON_FAILED_SCAN));
    assert_eq!(resolved.unattributed.availability, Availability::Partial);
    // Later intervals with trustworthy bounds are complete once past the
    // baseline's failed interval.
    for (seq, from, to) in [(2, 20_000, 22_000), (3, 21_999, 24_000)] {
        files
            .store
            .close_at(
                &next,
                seq - 1,
                seq,
                &interval(T0 + from, T0 + to, 0.0, 0.0, 1.0),
                &at(T0 + to - 1, (to - 1) * 1_000_000),
                T0,
            )
            .unwrap();
    }
    let later = files.read(T0 + 21_999, T0 + 24_000, Some(&live(&next, None)));
    assert_eq!(later.unattributed_ratio.value, Some(100.0));
    assert_eq!(later.unattributed.availability, Availability::Complete);
}

#[test]
fn a_clean_stop_seals_its_frontier_at_its_recorded_floor() {
    let files = Files::new();
    let old = scanner();
    three_scans(&files, &old, SECOND_NS);
    files.store.seal(&old, None, T0).unwrap();
    let next = scanner();
    let window = (T0 + 1_000, T0 + 3_002);
    for reader in [Some(&live(&next, None)), None] {
        let section = files.read(window.0, window.1, reader);
        assert_eq!(section.unattributed_ratio.value, Some(100.0));
        assert_eq!(section.unattributed.availability, Availability::Complete);
        // From the frontier on the stretch stays unresolved.
        assert_eq!(
            reason(&files.read(T0 + 3_000, T0 + 5_004, reader)),
            Some(REASON_PENDING_SCAN)
        );
    }
    files
        .store
        .begin_at(
            &next,
            1,
            &at(T0 + 20_000, 20 * SECOND_NS),
            T0 + 20_001,
            None,
            T0,
        )
        .unwrap();
    assert_eq!(failed_from(&files), [(T0 + 5_000) as i64]);

    // A seal carries what the stopped scanner still knew: a backward step
    // to 0.5 s is recorded with it, failed for every reader.
    let reversed = Files::new();
    three_scans(&reversed, &old, SECOND_NS);
    reversed.store.seal(&old, Some((4, T0 + 500)), T0).unwrap();
    assert_eq!(
        reason(&reversed.read(window.0, window.1, Some(&live(&next, None)))),
        Some(REASON_FAILED_SCAN)
    );
    // A running scanner's own unsealed frontier is no fence to its reads.
    let own = Files::new();
    three_scans(&own, &old, SECOND_NS);
    assert_eq!(
        own.read(window.0, window.1, Some(&live(&old, None)))
            .unattributed_ratio
            .value,
        Some(100.0)
    );
}

fn stored_boot(files: &Files) -> Option<i64> {
    Connection::open(files.store.path())
        .unwrap()
        .query_row("SELECT boot_ns FROM attribution_frontier", [], |r| r.get(0))
        .unwrap()
}

#[test]
fn boot_readings_past_104_days_stay_exact_and_capped_ones_imply_nothing() {
    // Just below and above 2^53 ns (about 104.25 days), and 120 days.
    for ns in [CAP_NS - 1, CAP_NS + 1, 120 * DAY_NS] {
        let files = Files::new();
        let id = scanner();
        files
            .store
            .begin_at(&id, 1, &at(T0, ns), T0 + 1, None, T0)
            .unwrap();
        assert_eq!(stored_boot(&files), Some(ns as i64), "{ns}");
        // A daemon restarted 5 s later sees no step.
        let restart = at(T0 + 5_000, ns + 5 * SECOND_NS);
        assert_eq!(files.store.reversal_floor(&restart).unwrap(), None, "{ns}");
        // A wall clock 8 s behind the boot clock implies its floor exactly.
        let behind = at(T0 - 3_000, ns + 5 * SECOND_NS);
        assert_eq!(
            files.store.reversal_floor(&behind).unwrap(),
            Some(T0 - 8_000),
            "{ns}"
        );
    }
    // A reading that does not fit the column is not stored: no certainty.
    let unfit = Files::new();
    let id = scanner();
    unfit
        .store
        .begin_at(&id, 1, &at(T0, u64::MAX), T0 + 1, None, T0)
        .unwrap();
    assert_eq!(stored_boot(&unfit), None);
    assert_eq!(
        unfit
            .store
            .reversal_floor(&at(T0 - 3_000, u64::MAX))
            .unwrap(),
        Some(0)
    );

    // A version-3 frontier capped at 2^53 - 1 ns while the host had been up
    // 120 days: read in place and after its upgrade, a 5 s restart implies
    // no multi-day step, and a clock behind the frontier fails closed.
    let legacy = Files::new();
    legacy
        .store
        .begin_at(&id, 1, &at(T0, 120 * DAY_NS), T0 + 1, None, T0)
        .unwrap();
    Connection::open(legacy.store.path())
        .unwrap()
        .execute_batch(&format!(
            "ALTER TABLE attribution_frontier DROP COLUMN sealed;
             UPDATE attribution_frontier SET boot_ns={CAP_NS};
             PRAGMA user_version=3;"
        ))
        .unwrap();
    let restart = at(T0 + 5_000, 120 * DAY_NS + 5 * SECOND_NS);
    let behind = at(T0 - 3_000, 120 * DAY_NS + 5 * SECOND_NS);
    let check = || {
        assert_eq!(legacy.store.reversal_floor(&restart).unwrap(), None);
        assert_eq!(legacy.store.reversal_floor(&behind).unwrap(), Some(0));
    };
    check();
    // Read in place, the capped frontier is still an unsealed fence.
    assert_eq!(
        reason(&legacy.read(T0 - 10_000, T0 - 9_000, Some(&live(&scanner(), None)))),
        Some(REASON_PENDING_SCAN)
    );
    legacy.store.mark_failed(&scanner()).unwrap();
    let raw = Connection::open(legacy.store.path()).unwrap();
    let (version, boot, sealed): (i64, Option<i64>, i64) = raw
        .query_row(
            "SELECT (SELECT user_version FROM pragma_user_version),boot_ns,sealed
             FROM attribution_frontier",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!((version, boot, sealed), (4, None, 0));
    check();
}

#[test]
fn clean_restart_at_120_days_preserves_the_measured_window() {
    let files = Files::new();
    let old = scanner();
    let uptime = 120 * DAY_NS;
    let (guard, uncertain) = files.store.arm_lifecycle(&old).unwrap();
    assert!(!uncertain);
    three_scans(&files, &old, uptime);
    let window = (T0 + 1_000, T0 + 3_002);
    files.store.seal(&old, None, T0).unwrap();
    files.store.seal_lifecycle(&guard, &old).unwrap();
    drop(guard);

    // The last frontier was read 5 s after the initial boot reading.
    // Restart five seconds after that, with both clocks advancing together.
    let next = scanner();
    let (_guard, uncertain) = files.store.arm_lifecycle(&next).unwrap();
    assert!(!uncertain);
    let restart = at(T0 + 10_000, uptime + 10 * SECOND_NS);
    assert_eq!(files.store.reversal_floor(&restart).unwrap(), None);
    files
        .store
        .begin_at(&next, 1, &restart, T0 + 10_001, None, T0)
        .unwrap();
    assert_eq!(failed_from(&files), [(T0 + 5_000) as i64]);
    let prior = files.read(window.0, window.1, Some(&live(&next, None)));
    assert_eq!(prior.window.scans, 1);
    assert_eq!(prior.unattributed.value, Some(1.0));
    assert_eq!(prior.unattributed_ratio.value, Some(100.0));
    assert_eq!(prior.unattributed.availability, Availability::Complete);
}

#[test]
fn lifecycle_witness_is_exclusive_and_unclean_until_the_frontier_is_sealed() {
    let files = Files::new();
    let first = scanner();
    let (guard, uncertain) = files.store.arm_lifecycle(&first).unwrap();
    assert!(!uncertain);
    assert!(files.store.arm_lifecycle(&scanner()).is_err());
    three_scans(&files, &first, SECOND_NS);
    files.store.seal(&first, None, T0).unwrap();
    files.store.seal_lifecycle(&guard, &first).unwrap();
    drop(guard);
    let second = scanner();
    let (guard, uncertain) = files.store.arm_lifecycle(&second).unwrap();
    assert!(!uncertain);
    fs::set_permissions(files.store.path(), fs::Permissions::from_mode(0o400)).unwrap();
    assert!(files
        .store
        .begin(&second, 1, T0 + 20_000, T0 + 20_001, T0)
        .is_err());
    assert!(files.store.seal(&second, Some((1, 0)), T0).is_err());
    // No attempt to certify the witness: a failed interval-store seal must
    // leave its independently committed admission record unclean.
    drop(guard);
    let offline = files.read(T0 + 1_000, T0 + 3_002, None);
    assert_eq!(offline.unattributed_ratio.value, None);
    assert_eq!(reason(&offline), Some(REASON_PENDING_SCAN));
    let next = scanner();
    let (_guard, uncertain) = files.store.arm_lifecycle(&next).unwrap();
    assert!(uncertain);
    fs::set_permissions(files.store.path(), fs::Permissions::from_mode(0o600)).unwrap();
    files
        .store
        .begin_at(
            &next,
            1,
            &at(T0 + 30_000, 30 * SECOND_NS),
            T0 + 30_001,
            Some(0),
            T0,
        )
        .unwrap();
    assert_eq!(failed_from(&files), [0]);
    let earlier = files.read(T0 + 1_000, T0 + 3_002, Some(&live(&next, None)));
    assert_eq!(earlier.unattributed_ratio.value, None);
    assert_eq!(reason(&earlier), Some(REASON_FAILED_SCAN));
}

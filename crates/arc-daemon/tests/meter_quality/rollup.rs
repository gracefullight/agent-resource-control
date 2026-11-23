//! Retention schedule assertions for real job samples.
use crate::{
    journey::{self, Sandbox},
    quality_support::{self as support, PROGRAM},
    recompute::{DAY_MS, HOUR_MS},
    runtime::{child_watchdog, Fixture, OwnedChild},
};
use rusqlite::Connection;
use std::{
    path::PathBuf,
    process::Stdio,
    time::{SystemTime, UNIX_EPOCH},
};

fn hold(f: &Fixture, s: &Sandbox, name: &str) -> (PathBuf, child_watchdog::Owned) {
    let project = s.project(name);
    let control = support::directory(f, &format!("{name}-control"));
    let mut command = s.arcctl(f, &project);
    command
        .args(["exec", "--", PROGRAM, "hold"])
        .arg(&control)
        .stdin(Stdio::null());
    (control, child_watchdog::Owned::spawn(&mut command).unwrap())
}

/// F2 over samples the daemon wrote: one job in two nonempty passes into
/// the same hour and day, beside another job, counts once there.
/// `before_restart` runs once the original daemon has stopped and before
/// the first restart recreates its output files.
pub fn assert_rollups_count_each_job_once(
    f: &Fixture,
    s: &Sandbox,
    db: &Connection,
    daemon: &mut OwnedChild,
    before_restart: impl FnOnce(),
) {
    let (same_dir, mut same) = hold(f, s, "hold-same");
    let (other_dir, mut other) = hold(f, s, "hold-other");
    let same_job = support::running_job(db, support::published(&same_dir, "hold.pid"));
    let other_job = support::running_job(db, support::published(&other_dir, "hold.pid"));
    journey::eventually("three samples of both held jobs", || {
        support::samples(db, &same_job).0 >= 3 && support::samples(db, &other_job).0 >= 3
    });
    for (directory, child, label) in [
        (&same_dir, &mut same, "held job"),
        (&other_dir, &mut other, "other held job"),
    ] {
        support::release(directory);
        assert!(child.wait(label).unwrap().success());
    }
    let ids = |job: &str| -> Vec<i64> {
        db.prepare("SELECT id FROM resource_samples WHERE job_id=?1 ORDER BY id LIMIT 3")
            .unwrap()
            .query_map([job], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    let same_ids = ids(&same_job);
    let other_ids = ids(&other_job);
    assert_eq!((same_ids.len(), other_ids.len()), (3, 3));
    daemon.stop(libc::SIGTERM);
    f.report();
    before_restart();
    std::fs::rename(
        f.root.join("runtime.json"),
        f.root.join("runtime-initial.json"),
    )
    .unwrap();

    // Age the actual observed rows into three distinct minutes in one hour
    // and day, so the real startup schedule can apply its 72 h retention
    // without changing the daemon clock or calling roll_up from the test.
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let aged = now - arc_storage::rollup::RAW_RETENTION_MS - 2 * HOUR_MS;
    let hour = aged - aged % HOUR_MS;
    let day = hour - hour % DAY_MS;
    for pass in 0..3 {
        let ms = hour + (pass as u64 + 1) * 60_000;
        assert_eq!(ms / HOUR_MS, hour / HOUR_MS);
        assert_eq!(ms / DAY_MS, day / DAY_MS);
        let timestamp = format!("{:019}", ms * 1_000_000);
        for id in [same_ids[pass], other_ids[pass]] {
            assert_eq!(
                db.execute(
                    "UPDATE resource_samples SET timestamp_ns=?1 WHERE id=?2",
                    rusqlite::params![timestamp, id]
                )
                .unwrap(),
                1
            );
        }
        let mut scheduled = f.spawn_isolated("");
        journey::eventually(
            "retention scheduler rolled the two staged observations",
            || {
                let left: i64 = db
                    .query_row(
                        "SELECT count(*) FROM resource_samples WHERE id IN (?1,?2)",
                        [same_ids[pass], other_ids[pass]],
                        |r| r.get(0),
                    )
                    .unwrap();
                left == 0
            },
        );
        for (granularity, bucket) in [("hour", hour), ("day", day)] {
            assert_eq!(
                support::job_count(db, granularity, bucket),
                Some(2),
                "pass {pass} {granularity}"
            );
            let members: i64 = db.query_row("SELECT count(*) FROM resource_rollup_jobs WHERE granularity=?1 AND bucket_start_ms=?2 AND job_id IN (?3,?4)", rusqlite::params![granularity, bucket as i64, same_job, other_job], |r| r.get(0)).unwrap();
            assert_eq!(members, 2, "pass {pass} {granularity}");
        }
        scheduled.stop(libc::SIGTERM);
        f.report();
        if pass < 2 {
            std::fs::rename(
                f.root.join("runtime.json"),
                f.root.join(format!("runtime-pass-{pass}.json")),
            )
            .unwrap();
        }
    }
}

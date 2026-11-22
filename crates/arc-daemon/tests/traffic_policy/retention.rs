//! MTR-014 items 1-2 and NFR-023 item 1: the daemon's own schedule rolls raw
//! samples older than 72 h into minute, hour and day buckets and deletes them,
//! expires stored Meter records, and keeps row counts bounded across repeated
//! batches. Samples are seeded into the database while the daemon is stopped;
//! every observation afterwards is daemon state, with no client request.
use super::fixture::*;
use crate::{journey::Sandbox, runtime::Fixture};
use rusqlite::params;
use std::time::{SystemTime, UNIX_EPOCH};

const MINUTE: u64 = 60_000;
const HOUR: u64 = 60 * MINUTE;
const DAY: u64 = 24 * HOUR;

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

fn ns(ms: u64) -> String {
    format!("{:019}", ms * 1_000_000)
}

/// Two processes of `job` sampled every 10 s over two whole minutes from
/// `base + batch` ms: cumulative CPU grows 1 s and 2 s per interval, RSS is
/// 1000 and 500 bytes, the first process writes 100 bytes per interval and
/// the second reports no writes. One more sample, one hour old, stays raw.
fn seed(db: &rusqlite::Connection, job: &str, batch: u64, base: u64, now: u64) {
    for (process, cpu_step, rss, writes) in [("a", 1_u64, 1_000_i64, true), ("b", 2, 500, false)] {
        for k in 0..12_u64 {
            db.execute(
                "INSERT INTO resource_samples(timestamp_ns,job_id,process_identity,cpu_time_ns,
                 rss_bytes,disk_write_bytes,attribution_confidence) VALUES(?1,?2,?3,?4,?5,?6,'A')",
                params![
                    ns(base + batch + k * 10_000),
                    job,
                    format!("seed-{batch}-{process}"),
                    (k * cpu_step * 1_000_000_000).to_string(),
                    rss,
                    writes.then_some((k * 100) as i64)
                ],
            )
            .unwrap();
        }
    }
    db.execute(
        "INSERT INTO resource_samples(timestamp_ns,job_id,process_identity,cpu_time_ns,
         attribution_confidence) VALUES(?1,?2,?3,'0','A')",
        params![ns(now - HOUR + batch), job, format!("seed-{batch}-recent")],
    )
    .unwrap();
}

type Bucket = (Option<f64>, Option<i64>, Option<i64>, i64, Option<i64>);

fn bucket(db: &rusqlite::Connection, granularity: &str, start: u64) -> Bucket {
    db.query_row(
        "SELECT cpu_core_seconds,peak_rss_bytes,disk_write_bytes,job_count,reclaimed_bytes
         FROM resource_rollups WHERE granularity=?1 AND bucket_start_ms=?2",
        params![granularity, start as i64],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
    )
    .unwrap()
}

fn count(db: &rusqlite::Connection, sql: &str) -> i64 {
    db.query_row(sql, [], |r| r.get(0)).unwrap()
}

pub fn raw_samples_roll_up_then_delete_and_rows_stay_bounded() {
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    s.stub("cargo");
    let project = s.project("project");
    let marker = f.root.join("starts");
    let now = now_ms();
    let old = now - 80 * HOUR;
    let base = old - old % HOUR;
    let day = base - base % DAY;
    let mut daemon = Some(daemon(&f, &[]));
    for batch in 0..3_u64 {
        // A real job of this batch, then the daemon stops for the seed.
        run(&f, &s, &project, "build", &marker);
        let job: String = database(&f)
            .query_row("SELECT id FROM jobs ORDER BY rowid DESC LIMIT 1", [], |r| {
                r.get(0)
            })
            .unwrap();
        stop(&f, daemon.take().unwrap());
        let db = arc_storage::open_and_migrate(&f.database).unwrap();
        seed(&db, &job, batch, base, now);
        if batch == 0 {
            // A stored Meter record past the 90-day metadata retention.
            let changed = db
                .execute(
                    "UPDATE job_meter_summaries SET terminal_at_ms=?2 WHERE job_id=?1
                     AND report_json IS NOT NULL",
                    params![job, (now - 91 * DAY) as i64],
                )
                .unwrap();
            assert_eq!(changed, 1, "the job has a stored Meter report");
        }
        drop(db);

        daemon = Some(super::fixture::daemon(&f, &[]));
        let db = database(&f);
        let older = format!(
            "SELECT count(*) FROM resource_samples WHERE timestamp_ns<'{}'",
            ns(now - 72 * HOUR)
        );
        crate::journey::eventually("the daemon's own rollup", || count(&db, &older) == 0);
        let n = batch as f64 + 1.0;
        let jobs = batch as i64 + 1;
        // Per batch: 18 and 15 core-seconds in the two minutes, writes of
        // 600 and 500 bytes, a sweep peak of 1500 bytes and one job.
        assert_eq!(
            bucket(&db, "minute", base),
            (Some(18.0 * n), Some(1_500), Some(600 * jobs), jobs, None)
        );
        assert_eq!(
            bucket(&db, "minute", base + MINUTE),
            (Some(15.0 * n), Some(1_500), Some(500 * jobs), jobs, None)
        );
        for (granularity, start) in [("hour", base), ("day", day)] {
            assert_eq!(
                bucket(&db, granularity, start),
                (Some(33.0 * n), Some(1_500), Some(1_100 * jobs), jobs, None)
            );
        }
        // Bounded: the rollups are the four buckets of the seeded range and
        // the only seeded raw rows left are those inside the 72 h window.
        assert_eq!(count(&db, "SELECT count(*) FROM resource_rollups"), 4);
        assert_eq!(
            count(
                &db,
                "SELECT count(*) FROM resource_samples WHERE process_identity LIKE 'seed-%'"
            ),
            jobs
        );
        crate::journey::eventually("the stored Meter record expired", || {
            count(
                &db,
                "SELECT count(*) FROM job_meter_summaries WHERE expired_at_ms IS NOT NULL
                 AND report_json IS NULL AND report_id IS NULL",
            ) == 1
        });
    }
    stop(&f, daemon.take().unwrap());
}

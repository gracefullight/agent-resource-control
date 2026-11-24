use super::*;

/// A migrated database in its own directory, removed on drop.
struct TempDb(std::path::PathBuf, Option<Connection>);
impl TempDb {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "arc-rollup-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let connection = crate::open_and_migrate(dir.join("db")).unwrap();
        Self(dir, Some(connection))
    }
    fn connection(&mut self) -> &mut Connection {
        self.1.as_mut().unwrap()
    }
}
impl Drop for TempDb {
    fn drop(&mut self) {
        drop(self.1.take());
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

const NOW: u64 = 1_800_000_000_000;

/// Two processes of two jobs sampled every 10 s over two whole minutes,
/// starting at `base`: cumulative CPU grows 1 s and 2 s per interval, the
/// first process writes 100 bytes per interval, the second reports no writes.
fn seed(connection: &Connection, batch: u32, base: u64) {
    for (index, (cpu_step, rss, writes)) in [(1_u64, 1_000_i64, true), (2, 500, false)]
        .into_iter()
        .enumerate()
    {
        let job = format!("job-{batch}-{index}");
        connection
            .execute(
                "INSERT INTO jobs(id,job_class,locality,enforcement,state,created_at)
                 VALUES(?1,'BUILD','LOCAL','OBSERVE','COMPLETED',0)",
                [&job],
            )
            .unwrap();
        for k in 0..12_u64 {
            connection
                .execute(
                    "INSERT INTO resource_samples(timestamp_ns,job_id,process_identity,cpu_time_ns,
                     rss_bytes,disk_write_bytes,attribution_confidence) VALUES(?1,?2,?3,?4,?5,?6,'A')",
                    params![
                        text_ns(base + k * 10_000),
                        job,
                        format!("{job}:pid"),
                        (k * cpu_step * 1_000_000_000).to_string(),
                        rss,
                        writes.then_some((k * 100) as i64)
                    ],
                )
                .unwrap();
        }
    }
}

fn rollup(
    connection: &Connection,
    granularity: &str,
    bucket: u64,
) -> (Option<f64>, Option<i64>, Option<i64>, i64, Option<i64>) {
    connection
        .query_row(
            "SELECT cpu_core_seconds,peak_rss_bytes,disk_write_bytes,job_count,reclaimed_bytes
             FROM resource_rollups WHERE granularity=?1 AND bucket_start_ms=?2",
            params![granularity, bucket as i64],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .unwrap()
}

fn count(connection: &Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
        .unwrap()
}

#[test]
fn old_samples_roll_up_once_into_every_granularity_and_are_deleted() {
    let mut db = TempDb::new();
    let old = NOW - 80 * 60 * 60 * 1000;
    let base = old - old % (60 * MINUTE_MS);
    for batch in 0..3_u32 {
        let connection = db.connection();
        seed(connection, batch, base);
        // One recent sample per batch stays raw.
        connection
            .execute(
                "INSERT INTO resource_samples(timestamp_ns,job_id,cpu_time_ns,attribution_confidence)
                 VALUES(?1,'job-0-0','5','A')",
                [text_ns(NOW - 60 * 60 * 1000 + u64::from(batch))],
            )
            .unwrap();
        let report = roll_up(connection, NOW).unwrap();
        assert_eq!(report.raw_rolled_up, 24, "batch {batch}");
        assert!(!report.more);
        // Rolling up again adds nothing: each raw row counted once.
        assert_eq!(roll_up(connection, NOW).unwrap().raw_rolled_up, 0);
        let n = f64::from(batch + 1);
        let jobs = 2 * i64::from(batch + 1);
        assert_eq!(
            rollup(connection, "minute", base),
            (
                Some(18.0 * n),
                Some(1_500),
                Some(600 * jobs / 2),
                jobs,
                None
            )
        );
        assert_eq!(
            rollup(connection, "minute", base + MINUTE_MS),
            (
                Some(15.0 * n),
                Some(1_500),
                Some(500 * jobs / 2),
                jobs,
                None
            )
        );
        let day = base - base % (24 * 60 * MINUTE_MS);
        for (granularity, bucket) in [("hour", base), ("day", day)] {
            assert_eq!(
                rollup(connection, granularity, bucket),
                (
                    Some(33.0 * n),
                    Some(1_500),
                    Some(1_100 * jobs / 2),
                    jobs,
                    None
                )
            );
        }
        assert_eq!(
            count(connection, "resource_rollups"),
            4,
            "bounded by buckets"
        );
        assert_eq!(count(connection, "resource_samples"), i64::from(batch + 1));
    }
}

#[test]
fn upgrade_preserves_old_bucket_count_and_marks_its_membership_unknown() {
    let mut db = Connection::open_in_memory().unwrap();
    crate::migration::migrate(&mut db, &crate::migration::MIGRATIONS[..9]).unwrap();
    let old = NOW - 80 * 60 * 60 * 1000;
    let base = old - old % (60 * MINUTE_MS);
    db.execute(
        "INSERT INTO resource_rollups(granularity,bucket_start_ms,job_count) VALUES('hour',?1,4)",
        [base as i64],
    )
    .unwrap();
    seed(&db, 0, base);
    crate::migration::migrate(&mut db, crate::migration::MIGRATIONS).unwrap();
    assert_eq!(db.query_row(
        "SELECT job_count,job_count_completeness FROM resource_rollups WHERE granularity='hour' AND bucket_start_ms=?1",
        [base as i64], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
    ).unwrap(), (4, "unknown".to_owned()));
    roll_up(&mut db, NOW).unwrap();
    assert_eq!(db.query_row(
        "SELECT job_count,job_count_completeness FROM resource_rollups WHERE granularity='hour' AND bucket_start_ms=?1",
        [base as i64], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)),
    ).unwrap(), (6, "unknown".to_owned()));
    assert_eq!(db.query_row(
        "SELECT job_count_completeness FROM resource_rollups WHERE granularity='minute' AND bucket_start_ms=?1",
        [base as i64], |r| r.get::<_, String>(0),
    ).unwrap(), "complete");
    assert_eq!(roll_up(&mut db, NOW).unwrap().raw_rolled_up, 0);
}

#[test]
fn expired_buckets_are_deleted_by_granularity() {
    let mut db = TempDb::new();
    let connection = db.connection();
    let day = 24 * 60 * MINUTE_MS;
    for (granularity, age) in [
        ("minute", 31 * day),
        ("minute", 29 * day),
        ("hour", 366 * day),
        ("hour", 300 * day),
        ("day", 366 * day),
        ("day", 364 * day),
    ] {
        connection
            .execute(
                "INSERT INTO resource_rollups(granularity,bucket_start_ms,job_count) VALUES(?1,?2,0)",
                params![granularity, (NOW - age) as i64],
            )
            .unwrap();
    }
    assert_eq!(roll_up(connection, NOW).unwrap().buckets_expired, 3);
    assert_eq!(count(connection, "resource_rollups"), 3);
}

#[test]
fn expiry_removes_dense_membership_in_bounded_passes_before_its_bucket() {
    let mut db = TempDb::new();
    let connection = db.connection();
    let expired = NOW - 31 * 24 * 60 * MINUTE_MS;
    connection.execute("INSERT INTO resource_rollups(granularity,bucket_start_ms,job_count) VALUES('minute',?1,?2)",
        params![expired as i64, MAX_RAW_BATCH + 1]).unwrap();
    let tx = connection.transaction().unwrap();
    for i in 0..=MAX_RAW_BATCH {
        tx.execute("INSERT INTO resource_rollup_jobs(granularity,bucket_start_ms,job_id) VALUES('minute',?1,?2)",
            params![expired as i64, format!("expiry-{i}")]).unwrap();
    }
    tx.commit().unwrap();
    let first = roll_up(connection, NOW).unwrap();
    assert!(first.more);
    assert_eq!(first.buckets_expired, 0);
    assert_eq!(count(connection, "resource_rollup_jobs"), 1);
    assert_eq!(count(connection, "resource_rollups"), 1);
    let second = roll_up(connection, NOW).unwrap();
    assert!(!second.more);
    assert_eq!(second.buckets_expired, 1);
    assert_eq!(count(connection, "resource_rollup_jobs"), 0);
    assert_eq!(count(connection, "resource_rollups"), 0);
}

#[test]
fn a_pass_is_bounded_even_inside_a_dense_minute() {
    let mut db = TempDb::new();
    let connection = db.connection();
    let first = (NOW - 100 * 60 * 60 * 1000) / MINUTE_MS * MINUTE_MS;
    let tx = connection.transaction().unwrap();
    for i in 0..(MAX_RAW_BATCH as u64 + 10) {
        tx.execute(
            "INSERT INTO resource_samples(timestamp_ns,attribution_confidence) VALUES(?1,'A')",
            [text_ns(first + i)],
        )
        .unwrap();
        tx.execute(
            "INSERT INTO resource_samples(timestamp_ns,attribution_confidence) VALUES(?1,'A')",
            [text_ns(first + MINUTE_MS + i % 1000)],
        )
        .unwrap();
    }
    tx.commit().unwrap();
    let report = roll_up(connection, NOW).unwrap();
    assert_eq!(report.raw_rolled_up, MAX_RAW_BATCH as u64);
    assert!(report.more);
    let report = roll_up(connection, NOW).unwrap();
    assert_eq!(report.raw_rolled_up, MAX_RAW_BATCH as u64);
    assert!(report.more);
    let report = roll_up(connection, NOW).unwrap();
    assert_eq!(report.raw_rolled_up, 20);
    assert!(!report.more);
    assert_eq!(count(connection, "resource_samples"), 0);
}

#[test]
fn dense_sweep_resumes_after_reopen_without_losing_peak_or_double_counting() {
    let mut db = TempDb::new();
    let hour = 60 * MINUTE_MS;
    let base = (NOW - 80 * hour) / hour * hour;
    let day = base / (24 * hour) * (24 * hour);
    for job in ["dense-a", "dense-b"] {
        db.connection()
            .execute(
                "INSERT INTO jobs(id,job_class,locality,enforcement,state,created_at)
             VALUES(?1,'BUILD','LOCAL','OBSERVE','COMPLETED',0)",
                [job],
            )
            .unwrap();
    }
    let tx = db.connection().transaction().unwrap();
    for i in 0..MAX_RAW_BATCH + 7 {
        tx.execute(
            "INSERT INTO resource_samples(timestamp_ns,job_id,rss_bytes,attribution_confidence)
                    VALUES(?1,?2,1,'A')",
            params![
                text_ns(base),
                if i % 2 == 0 { "dense-a" } else { "dense-b" }
            ],
        )
        .unwrap();
    }
    for (at, cpu, writes) in [(base + 1000, "0", 0), (base + 2000, "5000000000", 7)] {
        tx.execute("INSERT INTO resource_samples(timestamp_ns,job_id,process_identity,cpu_time_ns,rss_bytes,disk_write_bytes,attribution_confidence)
                    VALUES(?1,'dense-a','one',?2,10,?3,'A')", params![text_ns(at),cpu,writes]).unwrap();
    }
    tx.commit().unwrap();
    let report = roll_up(db.connection(), NOW).unwrap();
    assert_eq!(report.raw_rolled_up, MAX_RAW_BATCH as u64);
    assert!(report.more);
    assert_eq!(rollup(db.connection(), "minute", base).1, None);
    drop(db.1.take());
    db.1 = Some(crate::open_and_migrate(db.0.join("db")).unwrap());
    let report = roll_up(db.connection(), NOW).unwrap();
    assert_eq!(report.raw_rolled_up, 9);
    assert!(!report.more);
    for (g, at) in [("minute", base), ("hour", base), ("day", day)] {
        assert_eq!(
            rollup(db.connection(), g, at),
            (Some(5.0), Some(MAX_RAW_BATCH + 7), Some(7), 2, None)
        );
    }
    assert_eq!(roll_up(db.connection(), NOW).unwrap().raw_rolled_up, 0);
    assert_eq!(count(db.connection(), "resource_rollup_pending_sweeps"), 0);
}

#[test]
fn dense_multi_process_successors_use_index_across_bounded_passes() {
    let mut db = TempDb::new();
    let connection = db.connection();
    let hour = 60 * MINUTE_MS;
    let base = (NOW - 80 * hour) / hour * hour;
    connection
        .execute(
            "INSERT INTO jobs(id,job_class,locality,enforcement,state,created_at)
         VALUES('dense-job','BUILD','LOCAL','OBSERVE','COMPLETED',0)",
            [],
        )
        .unwrap();
    let tx = connection.transaction().unwrap();
    for i in 0..=MAX_RAW_BATCH {
        for (offset, cpu, written) in [(0, "0", 0), (1000, "1", 1)] {
            tx.execute(
                "INSERT INTO resource_samples(timestamp_ns,job_id,process_identity,cpu_time_ns,rss_bytes,disk_write_bytes,attribution_confidence)
                 VALUES(?1,'dense-job',?2,?3,1,?4,'A')",
                params![text_ns(base + offset), format!("process-{i}"), cpu, written],
            ).unwrap();
        }
    }
    tx.commit().unwrap();
    let first = roll_up(connection, NOW).unwrap();
    assert_eq!(first.raw_rolled_up, MAX_RAW_BATCH as u64);
    assert!(first.more);
    let plan: String = connection
        .query_row(
            "EXPLAIN QUERY PLAN SELECT x.id FROM resource_samples x WHERE x.job_id IS ?1
         AND x.process_identity=?2 AND x.timestamp_ns>?3 AND length(x.timestamp_ns)=19
         ORDER BY x.timestamp_ns LIMIT 1",
            params!["dense-job", "process-1", text_ns(base)],
            |row| row.get(3),
        )
        .unwrap();
    assert!(plan.contains("idx_resource_samples_rollup_next"), "{plan}");
    let second = roll_up(connection, NOW).unwrap();
    assert_eq!(second.raw_rolled_up, MAX_RAW_BATCH as u64);
    assert!(second.more);
    let third = roll_up(connection, NOW).unwrap();
    assert_eq!(third.raw_rolled_up, 2);
    assert!(!third.more);
    let actual = rollup(connection, "hour", base);
    assert!((actual.0.unwrap() - (MAX_RAW_BATCH + 1) as f64 / 1e9).abs() < 1e-12);
    assert_eq!(
        (actual.1, actual.2, actual.3),
        (Some(MAX_RAW_BATCH + 1), Some(MAX_RAW_BATCH + 1), 1)
    );
    assert_eq!(roll_up(connection, NOW).unwrap().raw_rolled_up, 0);
    assert_eq!(rollup(connection, "hour", base), actual);
}

#[test]
fn failed_delete_rolls_back_partial_aggregates_and_replay_counts_once() {
    let mut db = TempDb::new();
    let connection = db.connection();
    let hour = 60 * MINUTE_MS;
    let base = (NOW - 80 * hour) / hour * hour;
    seed(connection, 0, base);
    connection
        .execute_batch(
            "CREATE TRIGGER fixture_abort BEFORE DELETE ON resource_samples
         BEGIN SELECT RAISE(ABORT,'fixture rollback'); END;",
        )
        .unwrap();
    assert!(roll_up(connection, NOW).is_err());
    assert_eq!(count(connection, "resource_samples"), 24);
    assert_eq!(count(connection, "resource_rollups"), 0);
    assert_eq!(count(connection, "resource_rollup_jobs"), 0);
    connection
        .execute_batch("DROP TRIGGER fixture_abort;")
        .unwrap();
    assert_eq!(roll_up(connection, NOW).unwrap().raw_rolled_up, 24);
    assert_eq!(
        rollup(connection, "hour", base),
        (Some(33.0), Some(1_500), Some(1_100), 2, None)
    );
    assert_eq!(roll_up(connection, NOW).unwrap().raw_rolled_up, 0);
    assert_eq!(
        rollup(connection, "hour", base),
        (Some(33.0), Some(1_500), Some(1_100), 2, None)
    );
}

/// Cycle 35 F2: one job sampled across two nonempty passes into the same
/// hour and day, beside another job, counts once there; the minute buckets
/// of each pass count it once each, and repeating a pass changes nothing.
#[test]
fn a_job_rolled_up_by_two_passes_counts_once_per_hour_and_day() {
    let mut db = TempDb::new();
    let connection = db.connection();
    let hour = 60 * MINUTE_MS;
    let old = NOW - 80 * hour;
    let base = old - old % hour;
    let day = base - base % (24 * hour);
    for job in ["same", "other"] {
        connection
            .execute(
                "INSERT INTO jobs(id,job_class,locality,enforcement,state,created_at)
                 VALUES(?1,'BUILD','LOCAL','OBSERVE','COMPLETED',0)",
                [job],
            )
            .unwrap();
    }
    let sample = |connection: &Connection, job: &str, at: u64| {
        connection
            .execute(
                "INSERT INTO resource_samples(timestamp_ns,job_id,process_identity,cpu_time_ns,
                 rss_bytes,attribution_confidence) VALUES(?1,?2,?3,'0',1,'A')",
                params![text_ns(at), job, format!("{job}:pid")],
            )
            .unwrap();
    };
    let counts = |connection: &Connection| -> Vec<i64> {
        [
            ("minute", base),
            ("minute", base + 30 * MINUTE_MS),
            ("hour", base),
            ("day", day),
        ]
        .iter()
        .map(|(g, b)| rollup(connection, g, *b).3)
        .collect()
    };
    // Pass one: `same` and `other` in the first minute.
    sample(connection, "same", base);
    sample(connection, "other", base + 1);
    assert_eq!(
        roll_up(connection, NOW - 40 * MINUTE_MS)
            .unwrap()
            .raw_rolled_up,
        2
    );
    assert_eq!(rollup(connection, "hour", base).3, 2);
    // Pass two: `same` again, 30 minutes later, in the same hour and day.
    sample(connection, "same", base + 30 * MINUTE_MS);
    assert_eq!(roll_up(connection, NOW).unwrap().raw_rolled_up, 1);
    assert_eq!(counts(connection), [2, 1, 2, 2]);
    // Repeated passes leave every count unchanged.
    for _ in 0..2 {
        assert_eq!(roll_up(connection, NOW).unwrap().raw_rolled_up, 0);
        assert_eq!(counts(connection), [2, 1, 2, 2]);
    }
    assert_eq!(count(connection, "resource_rollup_jobs"), 2 + 1 + 2 + 2);
    // A bucket rolled before membership existed keeps its stored count.
    connection
        .execute("DELETE FROM resource_rollup_jobs", [])
        .unwrap();
    sample(connection, "same", base + 31 * MINUTE_MS);
    roll_up(connection, NOW).unwrap();
    assert_eq!(
        rollup(connection, "hour", base).3,
        3,
        "stored count kept, not rebuilt"
    );
}

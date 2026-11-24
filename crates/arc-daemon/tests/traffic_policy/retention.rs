//! MTR-014 items 1-2 and NFR-023 item 1: the daemon's own schedule rolls raw
//! samples older than 72 h into minute, hour and day buckets and deletes them,
//! expires stored Meter records, and keeps row counts bounded across repeated
//! batches. Samples are seeded into the database while the daemon is stopped;
//! every observation afterwards is daemon state, with no client request.
//!
//! R-014 item 3: the same schedule advances several bounded passes over the
//! same jobs while real client requests stay outstanding on the writer, as
//! the daemon's own final runtime report records for an uninterrupted run.
use super::fixture::*;
use crate::{journey::Sandbox, runtime::Fixture};
use rusqlite::params;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};
use std::time::{SystemTime, UNIX_EPOCH};
use std::{
    thread,
    time::{Duration, Instant},
};

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

/// Each worker keeps a real doctor request in flight on its own connection.
/// The guard joins all threads on unwind, including when a watchdog or
/// assertion fails.
struct Load {
    stop: Arc<AtomicBool>,
    go: Arc<AtomicBool>,
    connected: Arc<AtomicUsize>,
    progress: Arc<AtomicUsize>,
    failures: Arc<AtomicUsize>,
    threads: Vec<thread::JoinHandle<()>>,
}
impl Load {
    const CLIENTS: usize = 8;

    fn start(f: &Fixture) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let go = Arc::new(AtomicBool::new(false));
        let connected = Arc::new(AtomicUsize::new(0));
        let progress = Arc::new(AtomicUsize::new(0));
        let failures = Arc::new(AtomicUsize::new(0));
        let mut threads = Vec::new();
        for _ in 0..Self::CLIENTS {
            let (stop, go, connected, progress, failures) = (
                stop.clone(),
                go.clone(),
                connected.clone(),
                progress.clone(),
                failures.clone(),
            );
            let socket = f.socket.clone();
            threads.push(thread::spawn(move || {
                let mut session = arc_ipc::client::Session::connect_until(
                    &socket,
                    Instant::now() + Duration::from_secs(90),
                    &|| false,
                    None,
                )
                .unwrap();
                connected.fetch_add(1, Ordering::SeqCst);
                while !go.load(Ordering::SeqCst) && !stop.load(Ordering::SeqCst) {
                    thread::yield_now();
                }
                let request = arc_ipc::Request::new("load", arc_ipc::OPERATION_DOCTOR);
                while !stop.load(Ordering::SeqCst) {
                    let result = session.exchange(&request, &|| stop.load(Ordering::SeqCst));
                    if stop.load(Ordering::SeqCst) {
                        break;
                    }
                    if result.is_ok() {
                        progress.fetch_add(1, Ordering::SeqCst);
                    } else if !matches!(result, Err(arc_ipc::client::Failure::Busy)) {
                        failures.fetch_add(1, Ordering::SeqCst);
                        break;
                    }
                }
            }));
        }
        let load = Self {
            stop,
            go,
            connected,
            progress,
            failures,
            threads,
        };
        crate::journey::eventually("load clients connected", || {
            load.connected.load(Ordering::SeqCst) == Self::CLIENTS
        });
        load.go.store(true, Ordering::SeqCst);
        crate::journey::eventually("load clients making progress", || {
            load.progress.load(Ordering::SeqCst) >= Self::CLIENTS
        });
        load
    }
}
impl Drop for Load {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.go.store(true, Ordering::SeqCst);
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}

/// Processes per job in each dense sweep, sweeps and their spacing. Two jobs
/// give 14 rows per sweep and 42,000 rows over two minutes: three bounded
/// passes of `MAX_RAW_BATCH`, 20,000 and 2,000 rows. Both batch boundaries
/// split a sweep (row 20,000 is in sweep 1428, row 40,000 in sweep 2857), and
/// both jobs are measurable in every pass and in more than one pass of each
/// bucket.
const PROCESSES: u64 = 7;
const SWEEPS: u64 = 3_000;
const SPACING_MS: u64 = 40;
const DENSE_ROWS: i64 = (2 * PROCESSES * SWEEPS) as i64;

/// Job `a` adds 1 core-second and writes 100 bytes per interval with 1000
/// bytes RSS per process; job `b` adds 2 core-seconds, reports no writes and
/// 500 bytes RSS. RSS doubles in sweep 1428 and triples in sweep 2857, the
/// two split sweeps, so only whole-sweep peaks give the expected values. One
/// recent sample per job stays inside the 72 h window.
fn seed_dense(db: &rusqlite::Connection, base: u64, now: u64) {
    let mut insert = db
        .prepare(
            "INSERT INTO resource_samples(timestamp_ns,job_id,process_identity,cpu_time_ns,
             rss_bytes,disk_write_bytes,attribution_confidence) VALUES(?1,?2,?3,?4,?5,?6,'A')",
        )
        .unwrap();
    for k in 0..SWEEPS {
        let scale = match k {
            1428 => 2,
            2857 => 3,
            _ => 1,
        };
        for (job, cpu_step, rss, writes) in [
            ("load-seed-a", 1_u64, 1_000_i64, true),
            ("load-seed-b", 2, 500, false),
        ] {
            for process in 0..PROCESSES {
                insert
                    .execute(params![
                        ns(base + k * SPACING_MS),
                        job,
                        format!("{job}-{process}"),
                        (k * cpu_step * 1_000_000_000).to_string(),
                        rss * scale,
                        writes.then_some((k * 100) as i64)
                    ])
                    .unwrap();
            }
        }
    }
    for job in ["load-seed-a", "load-seed-b"] {
        db.execute(
            "INSERT INTO resource_samples(timestamp_ns,job_id,process_identity,cpu_time_ns,
             attribution_confidence) VALUES(?1,?2,?3,'0','A')",
            params![ns(now - HOUR), job, format!("{job}-recent")],
        )
        .unwrap();
    }
}

fn policy_audit(db: &rusqlite::Connection) -> Vec<(String, String)> {
    db.prepare(
        "SELECT entity_id,payload_redacted_json FROM audit_events
         WHERE event_type='POLICY_SWITCH_CHANGED' ORDER BY rowid",
    )
    .unwrap()
    .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
    .unwrap()
    .map(Result::unwrap)
    .collect()
}

pub fn scheduled_rollups_progress_during_sustained_load() {
    use arc_ipc::governor::*;
    let f = Fixture::new();
    let daemon_first = daemon(&f, &[]);
    let key = GovernorSwitchKey::StartRateControl;
    set(&f, OPERATION_SET_SWITCH, key, false, 0, 11);
    set(&f, OPERATION_SET_SWITCH, key, true, 1, 12);
    let before = policy_audit(&database(&f));
    assert_eq!(before.len(), 2);
    stop(&f, daemon_first);

    let now = now_ms();
    let old = now - 80 * HOUR;
    let base = old - old % HOUR;
    let day = base - base % DAY;
    const { assert!(SWEEPS * SPACING_MS <= 2 * MINUTE) };
    let mut db = arc_storage::open_and_migrate(&f.database).unwrap();
    let tx = db.transaction().unwrap();
    for job in ["load-seed-a", "load-seed-b"] {
        tx.execute(
            "INSERT INTO jobs(id,job_class,locality,enforcement,state,created_at)
             VALUES(?1,'BUILD','LOCAL','OBSERVE','COMPLETED',0)",
            [job],
        )
        .unwrap();
    }
    seed_dense(&tx, base, now);
    tx.commit().unwrap();
    drop(db);

    let daemon = daemon(&f, &[]);
    // Hold SQLite's write lock until the load runs: a scheduled pass admitted
    // before then fails busy and rolls nothing.
    let gate = database(&f);
    gate.execute_batch("BEGIN IMMEDIATE").unwrap();
    let db = database(&f);
    let older = format!(
        "SELECT count(*) FROM resource_samples WHERE timestamp_ns<'{}'",
        ns(now - 72 * HOUR)
    );
    assert_eq!(count(&db, &older), DENSE_ROWS);
    let load = Load::start(&f);
    gate.execute_batch("ROLLBACK").unwrap();
    drop(gate);
    let started = load.progress.load(Ordering::SeqCst);
    crate::journey::eventually("scheduled bounded passes during client load", || {
        assert_eq!(load.failures.load(Ordering::SeqCst), 0);
        count(&db, &older) == 0
    });
    let after_rollup = load.progress.load(Ordering::SeqCst);
    assert!(
        after_rollup > started,
        "clients progressed during retention"
    );
    let expected = |db: &rusqlite::Connection| {
        // CPU: 1500 sweeps with a successor in the first minute, 1499 in the
        // second; writes likewise for job a only; peaks are the split sweeps.
        let (a, b) = (PROCESSES as f64, PROCESSES as f64 * 2.0);
        let cpu = |sweeps: f64| (a + b) * sweeps;
        let writes = |sweeps: i64| PROCESSES as i64 * sweeps * 100;
        assert_eq!(
            bucket(db, "minute", base),
            (Some(cpu(1500.0)), Some(21_000), Some(writes(1500)), 2, None)
        );
        assert_eq!(
            bucket(db, "minute", base + MINUTE),
            (Some(cpu(1499.0)), Some(31_500), Some(writes(1499)), 2, None)
        );
        for (g, at) in [("hour", base), ("day", day)] {
            assert_eq!(
                bucket(db, g, at),
                (Some(cpu(2999.0)), Some(31_500), Some(writes(2999)), 2, None)
            );
        }
        assert_eq!(count(db, "SELECT count(*) FROM resource_rollups"), 4);
        assert_eq!(
            count(
                db,
                "SELECT count(*) FROM resource_rollup_jobs WHERE job_id LIKE 'load-seed-%'"
            ),
            8
        );
        assert_eq!(
            count(
                db,
                "SELECT count(*) FROM resource_samples WHERE job_id LIKE 'load-seed-%'"
            ),
            2
        );
    };
    expected(&db);
    crate::journey::eventually("clients continue after rollup", || {
        load.progress.load(Ordering::SeqCst) >= after_rollup + 20
    });
    expected(&db);
    assert_eq!(policy_audit(&db), before);
    assert_eq!(load.failures.load(Ordering::SeqCst), 0);
    drop(load);
    drop(db);
    let report = stop(&f, daemon);
    // The daemon tracks every DbOutstanding transition to zero, from before
    // admission of the first pass until after the final pass returns. All
    // rows must be rolled in one run without any such transition; the batch
    // bound requires at least three passes in that uninterrupted interval.
    let retention = &report["retention"];
    eprintln!("r-014 retention: {retention}");
    assert_eq!(retention["raw_rolled_up"], DENSE_ROWS, "{report}");
    assert_eq!(retention["loaded_raw_rolled_up"], DENSE_ROWS, "{report}");
    assert_eq!(
        retention["max_loaded_run_raw_rolled_up"], DENSE_ROWS,
        "{report}"
    );
    assert!(
        retention["loaded_passes"].as_u64().unwrap() >= 3,
        "{report}"
    );
}

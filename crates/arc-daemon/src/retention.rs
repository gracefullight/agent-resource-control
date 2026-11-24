//! The daemon's own rollup and retention schedule (lane C, MTR-014, NFR-023).
//!
//! `start` runs one bounded writer operation once at startup and then on a
//! fixed cadence for the supervisor's lifetime: `arc_storage::rollup::roll_up`
//! (raw samples older than 72 h into minute, hour and day buckets, deleted in
//! the same transaction; expired buckets removed) and the stored Meter expiry
//! (`arc_storage::job_meter::expire`), with no client request. A pass that
//! left older raw rows is followed by another, up to a bound, yielding to
//! request handlers between passes. Dropping the handle stops it.
//!
//! The first pass starts [`FIRST_DELAY`] after startup. One reserved FIFO
//! position lets bounded passes progress under sustained client load without
//! taking a client's queue slot. Client requests ahead of a pass run first.
//!
//! The final runtime report counts passes and raw rows from schedule runs
//! during which client `DbOutstanding` never reached zero. The worker tracks
//! every zero transition, including any between two passes. The largest such
//! run's raw-row count makes one uninterrupted multi-pass run observable.
use super::{jobs, DbClient, DbValue};
use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

const INTERVAL: Duration = Duration::from_secs(600);
const FIRST_DELAY: Duration = Duration::from_secs(2);
const BUSY_RETRY: Duration = Duration::from_secs(5);
/// One pass rolls at most `arc_storage::rollup::MAX_RAW_BATCH` raw rows.
const PASS_DEADLINE: Duration = Duration::from_secs(30);
const MAX_PASSES: usize = 16;
const MAX_METER_EXPIRY_BATCHES: usize = 8;

pub(crate) struct Retention {
    task: tokio::task::JoinHandle<()>,
    stats: Arc<Stats>,
}

/// Lifetime counts. "Loaded" covers whole runs with no client-idle gap.
#[derive(Default)]
struct Stats {
    passes: AtomicU64,
    loaded_passes: AtomicU64,
    raw_rolled_up: AtomicU64,
    loaded_raw_rolled_up: AtomicU64,
    max_loaded_run_raw_rolled_up: AtomicU64,
}

impl Retention {
    /// Fixed keys and integer values for the final runtime report.
    pub(crate) fn json(&self) -> serde_json::Value {
        let read = |value: &AtomicU64| value.load(Ordering::Acquire);
        serde_json::json!({
            "passes": read(&self.stats.passes),
            "loaded_passes": read(&self.stats.loaded_passes),
            "raw_rolled_up": read(&self.stats.raw_rolled_up),
            "loaded_raw_rolled_up": read(&self.stats.loaded_raw_rolled_up),
            "max_loaded_run_raw_rolled_up": read(&self.stats.max_loaded_run_raw_rolled_up),
        })
    }
}

impl Drop for Retention {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub(crate) fn start(client: &DbClient) -> Option<Retention> {
    let client = client.clone();
    let stats = Arc::new(Stats::default());
    let task_stats = Arc::clone(&stats);
    let task = tokio::spawn(async move {
        let mut delay = FIRST_DELAY;
        loop {
            tokio::time::sleep(delay).await;
            delay = if run(&client, &task_stats).await {
                BUSY_RETRY
            } else {
                INTERVAL
            };
        }
    });
    Some(Retention { task, stats })
}

async fn run(client: &DbClient, stats: &Stats) -> bool {
    let start = client.shared.outstanding.snapshot();
    let mut run_passes = 0_u64;
    let mut run_raw = 0_u64;
    let mut retry = true;
    for _ in 0..MAX_PASSES {
        if client.shared.stopping.load(Ordering::Acquire) {
            retry = false;
            break;
        }
        let operation = jobs::Operation::Retention(Operation::Run {
            now_ms: jobs::now(),
        });
        let Ok(ticket) = client.admit_retention(Instant::now() + PASS_DEADLINE, operation) else {
            break;
        };
        let value = ticket.receive_value().await;
        stats.passes.fetch_add(1, Ordering::AcqRel);
        run_passes += 1;
        match value {
            Ok(DbValue::Lifecycle(value)) => match *value {
                jobs::Value::Retention(Value::Run(Ok(report))) => {
                    stats
                        .raw_rolled_up
                        .fetch_add(report.raw_rolled_up, Ordering::AcqRel);
                    run_raw += report.raw_rolled_up;
                    if !report.more {
                        retry = false;
                        break;
                    }
                }
                _ => break,
            },
            _ => break,
        }
        tokio::task::yield_now().await;
    }
    if start.continuous_until(client.shared.outstanding.snapshot()) {
        stats.loaded_passes.fetch_add(run_passes, Ordering::AcqRel);
        stats
            .loaded_raw_rolled_up
            .fetch_add(run_raw, Ordering::AcqRel);
        stats
            .max_loaded_run_raw_rolled_up
            .fetch_max(run_raw, Ordering::AcqRel);
    }
    retry
}

pub(crate) enum Operation {
    Run { now_ms: u64 },
}

pub(crate) enum Value {
    Run(Result<arc_storage::rollup::RollupReport, arc_storage::jobs::JobError>),
}

pub(crate) fn execute(connection: &mut rusqlite::Connection, operation: Operation) -> Value {
    match operation {
        Operation::Run { now_ms } => {
            let report = arc_storage::rollup::roll_up(connection, now_ms);
            for _ in 0..MAX_METER_EXPIRY_BATCHES {
                match arc_storage::job_meter::expire(connection, now_ms) {
                    Ok(expired) if expired >= arc_storage::job_meter::MAX_EXPIRY_BATCH => {}
                    _ => break,
                }
            }
            Value::Run(report)
        }
    }
}

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
//! The schedule is low priority. The first pass starts [`FIRST_DELAY`] after
//! startup, and a pass runs only while the writer has no outstanding request:
//! a busy writer postpones it by [`BUSY_RETRY`]. It never waits for a queue
//! slot, and a full or stopping writer skips the pass without counting a
//! rejected request, so it never takes a client's slot or changes the
//! accounting of client requests.
use super::{jobs, DbClient, DbValue};
use crate::runtime_metrics::Gauge;
use std::{
    sync::atomic::Ordering,
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
}

impl Drop for Retention {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub(crate) fn start(client: &DbClient) -> Option<Retention> {
    let client = client.clone();
    let task = tokio::spawn(async move {
        let mut delay = FIRST_DELAY;
        loop {
            tokio::time::sleep(delay).await;
            delay = if idle(&client) {
                run(&client).await;
                INTERVAL
            } else {
                BUSY_RETRY
            };
        }
    });
    Some(Retention { task })
}

/// No client request is queued or executing on the writer.
fn idle(client: &DbClient) -> bool {
    client
        .metrics
        .snapshot()
        .gauge(Gauge::DbOutstanding)
        .current
        == 0
}

async fn run(client: &DbClient) {
    for pass in 0..MAX_PASSES {
        // Later passes follow this task's own just-answered request.
        if client.shared.stopping.load(Ordering::Acquire) || (pass == 0 && !idle(client)) {
            return;
        }
        let operation = jobs::Operation::Retention(Operation::Run {
            now_ms: jobs::now(),
        });
        // `admit` directly: a skipped low-priority pass is not a rejection.
        let Ok(ticket) = client.admit(Instant::now() + PASS_DEADLINE, Some(operation)) else {
            return;
        };
        match ticket.receive_value().await {
            Ok(DbValue::Lifecycle(value)) => match *value {
                jobs::Value::Retention(Value::Run(Ok(report))) if report.more => {}
                _ => return,
            },
            _ => return,
        }
        tokio::task::yield_now().await;
    }
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

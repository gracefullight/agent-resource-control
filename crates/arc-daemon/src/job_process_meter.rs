//! Job process-tree metering on the daemon's Meter cadence (lane A, MTR-002,
//! MTR-003, MET-002..010, DET-012).
//!
//! After each Meter publication, the process tree of every RUNNING managed
//! job is sampled from its recorded child identity (pid, start token, boot
//! id): its process group and ppid descendants, read with `proc_pid_rusage`
//! and `proc_pidinfo` (`arc_platform::process_tree`), each sample
//! revalidated against identity before it counts, then written through
//! `db_worker::resource_samples` on the single writer. Native reads never
//! run on a request path or under the SQLite writer.
//!
//! The hook runs on the Meter thread. It sends no writer request while the
//! writer's committed state has no active job and no tree report is pending,
//! so an idle daemon does no extra work.
//! One pass shares one deadline; a busy writer only skips that pass.
use crate::db_worker::{
    resource_samples::{self, Record},
    DbClient,
};
use arc_core::process_identity::probe_live;
use arc_meter::job_tree::{revalidate, JobTree};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

/// One pass's budget across all of its writer requests.
const PASS_BUDGET: Duration = Duration::from_secs(2);
/// Idle passes that still look for a waited usage to complete a report.
const AWAITING_PASSES: u32 = 30;

#[derive(Default)]
struct State {
    jobs: BTreeMap<String, JobTree>,
    awaiting: u32,
}

pub(super) fn bind(meter: &arc_meter::MeterWorker, client: &DbClient) {
    let client = client.clone();
    let mut state = State::default();
    meter.set_sample_hook(Box::new(move |_snapshot| {
        if !resource_samples::jobs_active(&client) && state.jobs.is_empty() && state.awaiting == 0 {
            return;
        }
        // A runtime per pass is created and dropped on this thread, never
        // inside the daemon's async context.
        let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
        else {
            return;
        };
        runtime.block_on(pass(&client, &mut state));
    }));
}

fn wall_ns() -> Option<u64> {
    let ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_nanos();
    u64::try_from(ns).ok()
}

async fn pass(client: &DbClient, state: &mut State) {
    let end = Instant::now() + PASS_BUDGET;
    let Some(pass) = resource_samples::pass(client, crate::db_worker::jobs::now(), end).await
    else {
        return;
    };
    state.awaiting = match (pass.awaiting_waited, state.awaiting) {
        (false, _) => 0,
        (true, 0) => AWAITING_PASSES,
        (true, left) => left - 1,
    };
    state
        .jobs
        .retain(|id, _| pass.running.iter().any(|job| &job.job_id == id));
    for job in pass.running {
        let tree = arc_platform::process_tree::read_tree(&job.child);
        let readings = revalidate(tree, &job.child, probe_live);
        let Some(timestamp_ns) = wall_ns().filter(|_| !readings.is_empty()) else {
            continue;
        };
        let sampler = state
            .jobs
            .entry(job.job_id.clone())
            .or_insert_with(|| JobTree::new(job.resumed));
        // Advance the sampler only with a committed sample, so each stored
        // core-percent is the delta from the identity's previous stored row.
        let mut next = sampler.clone();
        let rows = next.observe(timestamp_ns, &readings);
        let record = Record {
            job_id: job.job_id,
            child: job.child,
            timestamp_ns,
            rows,
            totals: next.totals(),
        };
        if resource_samples::record(client, record, end).await {
            *sampler = next;
        } else if Instant::now() >= end {
            return;
        }
    }
}

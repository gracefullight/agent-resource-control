//! The one post-publication hook of the daemon's Meter
//! (`arc_meter::MeterWorker::set_sample_hook` holds a single slot).
//!
//! After each Meter publication, on the Meter thread and never on a request
//! path or under the SQLite writer, the hook runs the existing job
//! process-tree sampler (`job_process_meter`) and then lane A's native
//! attribution scan (`process_attribution`), in that order, on the same
//! adaptive cadence: slow when idle (SRS 10.7). Each keeps its own state and
//! its own bounded pass budget; neither replaces the other.
use super::{job_process_meter, process_attribution};
use crate::db_worker::DbClient;

pub(super) fn bind(meter: &arc_meter::MeterWorker, client: &DbClient) {
    let mut job_tree = job_process_meter::sampler(client);
    let mut attribution = process_attribution::sampler(client);
    meter.set_sample_hook(Box::new(move |snapshot| {
        job_tree(snapshot);
        attribution(snapshot);
    }));
}

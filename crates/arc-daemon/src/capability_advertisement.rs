//! Standing advertisements of the Phase 1 meter and policy capabilities.
//!
//! Each capability is its own file beside the socket, write-locked for the
//! supervisor's lifetime exactly like `traffic_fairness_advertisement.rs`, so
//! a client learns it without another request and a protocol-1 peer sees the
//! requests it saw before: `meter.job-tree.v1`, `meter.usage-summary.v1`
//! (also gating the registration usage fields), `meter.snapshot-traffic.v1`
//! and `traffic.decision-history.v1`. A failure only means no client uses it.
use super::fairness_advertisement::{advertise_named, Advertisement};
use arc_ipc::{meter, traffic::decision_history};
use std::path::Path;

pub(super) const NAMES: [&str; 4] = [
    meter::job_tree::ADVERTISEMENT,
    meter::usage::ADVERTISEMENT,
    meter::usage::SNAPSHOT_TRAFFIC_ADVERTISEMENT,
    decision_history::ADVERTISEMENT,
];

/// Held until the supervisor returns.
pub(super) fn advertise(socket: &Path) -> Vec<Advertisement> {
    NAMES
        .iter()
        .filter_map(|name| advertise_named(socket, name))
        .collect()
}

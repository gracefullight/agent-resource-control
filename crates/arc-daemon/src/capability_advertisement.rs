//! Standing advertisements of the Phase 1 meter and policy capabilities.
//!
//! Each capability is its own file beside the socket, write-locked for the
//! supervisor's lifetime exactly like `traffic_fairness_advertisement.rs`, so
//! a client learns it without another request and a protocol-1 peer sees the
//! requests it saw before: `meter.job-tree.v1`, `meter.usage-summary.v1`
//! (also gating the registration usage fields), `meter.snapshot-traffic.v1`,
//! `traffic.decision-history.v1`, `meter.usage-summary.v2` (also gating the
//! registration session digest), `meter.attribution.v1` and
//! `meter.job-footprint.v1`. A failure only means no client uses it.
//!
//! Whether `meter.attribution.v1` is held is recorded for this daemon's own
//! v2 usage-summary replies, which carry the attribution section only then.
use super::fairness_advertisement::{advertise_named, Advertisement};
use crate::connection::policy_dispatch::usage_summary::ATTRIBUTION_ADVERTISED;
use arc_ipc::{meter, traffic::decision_history};
use std::{path::Path, sync::atomic::Ordering};

pub(super) const NAMES: [&str; 9] = [
    meter::job_tree::ADVERTISEMENT,
    meter::job_tree_quality::ADVERTISEMENT,
    meter::usage::ADVERTISEMENT,
    meter::usage::SNAPSHOT_TRAFFIC_ADVERTISEMENT,
    decision_history::ADVERTISEMENT,
    meter::usage::ADVERTISEMENT_V2,
    meter::usage::CODEX_ADVERTISEMENT,
    meter::attribution::ADVERTISEMENT,
    meter::footprint::ADVERTISEMENT,
];

/// Held until the supervisor returns.
pub(super) fn advertise(socket: &Path) -> Vec<Advertisement> {
    let mut attribution = false;
    let advertisements = NAMES
        .iter()
        .filter_map(|name| {
            let advertisement = advertise_named(socket, name)?;
            attribution |= *name == meter::attribution::ADVERTISEMENT;
            Some(advertisement)
        })
        .collect();
    ATTRIBUTION_ADVERTISED.store(attribution, Ordering::Release);
    advertisements
}

//! Job process-tree aggregation (lane A, MTR-002, MTR-003, MET-002..005,
//! MET-009, MET-010, MTR-009, DET-012).
//!
//! Pure functions over `arc_platform::process_tree` readings of one job:
//! identity revalidation, per-process CPU core-percent (100 = one core) from
//! consecutive own-CPU deltas of one revalidated identity, RSS, disk read
//! and written bytes, and the running totals with which the daemon writes
//! the job's tree report.
//!
//! CPU of the whole tree is the root's waited rusage plus the last sampled
//! CPU (own and reaped children) of each *detached* descendant: one whose
//! parent was not a tree member at its last sample, so no tree process can
//! reap it into the waited total. A nested descendant is left to its parent's
//! reaped-children time, which is how its CPU reaches the root's wait. A
//! descendant that may have exited between samples keeps `partial` with
//! `unobserved_descendants`, never a silent zero.
use arc_core::process_identity::{IdentityStatus, ProcessIdentity};
use arc_platform::process_tree::NativeProcess;
use std::collections::{BTreeMap, BTreeSet};

/// Distinct identities tracked for one job; more are counted as partial.
pub const MAX_TRACKED_PROCESSES: usize = 4096;

/// How one reading relates to the recorded child.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Membership {
    /// The recorded child itself (attribution A).
    Root,
    /// Its parent is a member of the same sample (attribution C).
    Nested,
    /// Its parent is not a member: an orphan kept by the root's process
    /// group, never reaped into the root's waited usage (attribution C).
    Detached,
}

/// One identity-revalidated reading of one process of a job's tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessReading {
    pub identity: ProcessIdentity,
    pub cpu_time_ns: u64,
    pub child_cpu_time_ns: u64,
    pub rss_bytes: u64,
    pub disk_read_bytes: u64,
    pub disk_write_bytes: u64,
    pub membership: Membership,
}
impl ProcessReading {
    pub fn root(&self) -> bool {
        self.membership == Membership::Root
    }
}

/// Keep only readings whose identity is still live and unchanged. Without a
/// verified root nothing is attributed: its PID may belong to another process.
pub fn revalidate(
    tree: Vec<NativeProcess>,
    root: &ProcessIdentity,
    probe: impl Fn(&ProcessIdentity) -> IdentityStatus,
) -> Vec<ProcessReading> {
    let verified: Vec<NativeProcess> = tree
        .into_iter()
        .filter(|p| probe(&p.identity) == IdentityStatus::VerifiedMatch)
        .collect();
    if !verified.iter().any(|p| p.identity == *root) {
        return Vec::new();
    }
    let members: BTreeSet<u32> = verified.iter().map(|p| p.identity.pid).collect();
    verified
        .into_iter()
        .map(|p| ProcessReading {
            membership: if p.identity == *root {
                Membership::Root
            } else if members.contains(&p.parent_pid) {
                Membership::Nested
            } else {
                Membership::Detached
            },
            identity: p.identity,
            cpu_time_ns: p.cpu_time_ns,
            child_cpu_time_ns: p.child_cpu_time_ns,
            rss_bytes: p.rss_bytes,
            disk_read_bytes: p.disk_read_bytes,
            disk_write_bytes: p.disk_write_bytes,
        })
        .collect()
}

/// Core-percent of one identity between two samples: 100 is one core.
/// `None` for a first sample, a clock step back or a counter that fell.
pub fn core_percent(previous: (u64, u64), current: (u64, u64)) -> Option<f64> {
    let ((t0, c0), (t1, c1)) = (previous, current);
    (t1 > t0 && c1 >= c0).then(|| (c1 - c0) as f64 / (t1 - t0) as f64 * 100.0)
}

/// One row for `resource_samples`.
#[derive(Debug, Clone, PartialEq)]
pub struct SampleRow {
    pub reading: ProcessReading,
    pub cpu_core_percent: Option<f64>,
}

/// Running totals of one job's samples; values are `None` until observed.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TreeTotals {
    /// Last sampled own plus reaped-children CPU of the root.
    pub root_cpu_ns: Option<u64>,
    /// Sum of the last sampled own plus reaped-children CPU of detached descendants.
    pub detached_cpu_ns: u64,
    /// Highest sum of resident bytes over the members of one sample.
    pub peak_rss_bytes: Option<u64>,
    pub disk_read_bytes: Option<u64>,
    pub disk_write_bytes: Option<u64>,
    pub samples: u64,
    pub processes: u64,
    /// A descendant was observed, so one may have exited between samples, or
    /// earlier samples of this job were lost.
    pub partial: bool,
}

#[derive(Debug, Clone)]
struct Last {
    timestamp_ns: u64,
    cpu_time_ns: u64,
    inclusive_ns: u64,
    disk_read_bytes: u64,
    disk_write_bytes: u64,
    membership: Membership,
}

/// The sampler's working state for one job, private to the Meter thread.
#[derive(Debug, Clone, Default)]
pub struct JobTree {
    processes: BTreeMap<(u32, String), Last>,
    peak_rss_bytes: Option<u64>,
    samples: u64,
    partial: bool,
}

impl JobTree {
    /// `resumed` marks a job whose earlier samples were taken by another
    /// daemon process: those identities and peaks are not known here.
    pub fn new(resumed: bool) -> Self {
        Self {
            partial: resumed,
            ..Self::default()
        }
    }

    /// Fold one revalidated sample; returns its rows with core-percent.
    pub fn observe(&mut self, timestamp_ns: u64, readings: &[ProcessReading]) -> Vec<SampleRow> {
        if readings.is_empty() {
            return Vec::new();
        }
        self.samples += 1;
        let rss = readings
            .iter()
            .fold(0u64, |sum, r| sum.saturating_add(r.rss_bytes));
        self.peak_rss_bytes = Some(self.peak_rss_bytes.map_or(rss, |peak| peak.max(rss)));
        let mut rows = Vec::with_capacity(readings.len());
        for reading in readings {
            if !reading.root() {
                self.partial = true;
            }
            let key = (reading.identity.pid, reading.identity.start_token.clone());
            let percent = self.processes.get(&key).and_then(|last| {
                core_percent(
                    (last.timestamp_ns, last.cpu_time_ns),
                    (timestamp_ns, reading.cpu_time_ns),
                )
            });
            let last = Last {
                timestamp_ns,
                cpu_time_ns: reading.cpu_time_ns,
                inclusive_ns: reading
                    .cpu_time_ns
                    .saturating_add(reading.child_cpu_time_ns),
                disk_read_bytes: reading.disk_read_bytes,
                disk_write_bytes: reading.disk_write_bytes,
                membership: reading.membership,
            };
            if self.processes.contains_key(&key) || self.processes.len() < MAX_TRACKED_PROCESSES {
                self.processes.insert(key, last);
            } else {
                self.partial = true;
            }
            rows.push(SampleRow {
                reading: reading.clone(),
                cpu_core_percent: percent,
            });
        }
        rows
    }

    pub fn totals(&self) -> TreeTotals {
        let observed = !self.processes.is_empty();
        let sum = |value: fn(&Last) -> u64| {
            observed.then(|| {
                self.processes
                    .values()
                    .fold(0u64, |sum, last| sum.saturating_add(value(last)))
            })
        };
        TreeTotals {
            root_cpu_ns: self
                .processes
                .values()
                .find(|last| last.membership == Membership::Root)
                .map(|last| last.inclusive_ns),
            detached_cpu_ns: self
                .processes
                .values()
                .filter(|last| last.membership == Membership::Detached)
                .fold(0u64, |sum, last| sum.saturating_add(last.inclusive_ns)),
            peak_rss_bytes: self.peak_rss_bytes,
            disk_read_bytes: sum(|last| last.disk_read_bytes),
            disk_write_bytes: sum(|last| last.disk_write_bytes),
            samples: self.samples,
            processes: self.processes.len() as u64,
            partial: self.partial,
        }
    }
}

#[cfg(test)]
#[path = "job_tree/tests.rs"]
mod tests;

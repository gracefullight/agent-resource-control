//! Per-scan attribution of the current user's processes (lane A: MTR-007,
//! AT-MTR-03, MET-027, NFR-012).
//!
//! Pure classification and accumulation over `arc_platform::process_scan`
//! readings; no native I/O. Each live process identity gets one confidence
//! (the rules of `arc_ipc::meter::attribution`):
//!
//! - **Proven**: a member of a managed root's tree in this same scan. The
//!   root is a RUNNING job's recorded child whose identity (pid, start
//!   token, boot id) matches a live reading of this scan; a member reaches it
//!   over identity-ordered ppid edges. Group membership by itself does not
//!   establish ownership. Grouped under the root's vendor.
//! - **Estimated**: not Proven, and the process or one of its
//!   identity-ordered ppid ancestors has an OS process name that the caller's
//!   finite agent-executable mapping names. Grouped under that vendor.
//! - **Unattributed**: everything else, under no vendor.
//!
//! A ppid edge counts only when the parent is a reading of the same scan
//! that did not start after the child, so a recycled parent PID never
//! carries an ancestry. CPU is each identity's own CPU-time delta since the
//! previous scan of that same identity; a first sighting or a counter that
//! fell contributes nothing and is counted as skipped.
use arc_core::process_identity::ProcessIdentity;
use arc_platform::process_scan::{Scan, ScannedProcess};
use std::collections::{BTreeMap, HashMap};

/// Ppid edges followed from one process; deeper ancestry stops the walk.
pub const MAX_ANCESTRY: usize = 128;

/// A RUNNING managed job's recorded child and its vendor group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedRoot<V> {
    pub identity: ProcessIdentity,
    pub vendor: V,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Class<V> {
    Proven(V),
    Estimated(V),
    Unattributed,
}

/// One process of one scan: its class and the CPU it contributed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attributed<V> {
    pub identity: ProcessIdentity,
    pub class: Class<V>,
    /// `None` for a first sighting or a fallen counter.
    pub delta_ns: Option<u64>,
    /// The process's own cumulative CPU time read by this scan.
    pub cpu_time_ns: u64,
}

/// The aggregates of one scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanTotals<V> {
    pub proven_ns: BTreeMap<V, u64>,
    pub estimated_ns: BTreeMap<V, u64>,
    pub unattributed_ns: u64,
    /// Processes that contributed nothing: first sightings, fallen counters
    /// and listed processes that could not be read.
    pub skipped: u64,
}
impl<V> Default for ScanTotals<V> {
    fn default() -> Self {
        Self {
            proven_ns: BTreeMap::new(),
            estimated_ns: BTreeMap::new(),
            unattributed_ns: 0,
            skipped: 0,
        }
    }
}

/// The scanner's working state, private to the Meter thread: the last CPU
/// time of each identity of the previous scan.
#[derive(Debug, Clone, Default)]
pub struct Attributor {
    last: HashMap<(u32, String, String), u64>,
}

impl Attributor {
    /// Classify one scan and fold its CPU deltas.
    pub fn observe<V: Clone + Ord>(
        &mut self,
        scan: &Scan,
        managed: &[ManagedRoot<V>],
        agent: impl Fn(&str) -> Option<V>,
        verify: impl Fn(&ProcessIdentity, &ProcessIdentity) -> bool,
    ) -> (ScanTotals<V>, Vec<Attributed<V>>) {
        let classes = classify_with_proof(&scan.processes, managed, agent, verify);
        let mut totals = ScanTotals {
            skipped: scan.unreadable,
            ..ScanTotals::default()
        };
        let mut next = HashMap::with_capacity(scan.processes.len());
        let mut attributed = Vec::with_capacity(scan.processes.len());
        for (process, class) in scan.processes.iter().zip(classes) {
            let key = (
                process.identity.pid,
                process.identity.start_token.clone(),
                process.identity.boot_id.clone(),
            );
            let delta = self
                .last
                .get(&key)
                .and_then(|last| process.cpu_time_ns.checked_sub(*last));
            next.insert(key, process.cpu_time_ns);
            match (delta, &class) {
                (None, _) => totals.skipped += 1,
                (Some(ns), Class::Proven(vendor)) => {
                    add(totals.proven_ns.entry(vendor.clone()).or_default(), ns)
                }
                (Some(ns), Class::Estimated(vendor)) => {
                    add(totals.estimated_ns.entry(vendor.clone()).or_default(), ns)
                }
                (Some(ns), Class::Unattributed) => add(&mut totals.unattributed_ns, ns),
            }
            attributed.push(Attributed {
                identity: process.identity.clone(),
                class,
                delta_ns: delta,
                cpu_time_ns: process.cpu_time_ns,
            });
        }
        self.last = next;
        (totals, attributed)
    }
}

fn add(sum: &mut u64, ns: u64) {
    *sum = sum.saturating_add(ns);
}

/// One class per process, in scan order.
pub fn classify<V: Clone>(
    processes: &[ScannedProcess],
    managed: &[ManagedRoot<V>],
    agent: impl Fn(&str) -> Option<V>,
) -> Vec<Class<V>> {
    classify_with_proof(processes, managed, agent, |_, _| true)
}

fn classify_with_proof<V: Clone>(
    processes: &[ScannedProcess],
    managed: &[ManagedRoot<V>],
    agent: impl Fn(&str) -> Option<V>,
    verify: impl Fn(&ProcessIdentity, &ProcessIdentity) -> bool,
) -> Vec<Class<V>> {
    let by_pid: HashMap<u32, usize> = processes
        .iter()
        .enumerate()
        .map(|(i, p)| (p.identity.pid, i))
        .collect();
    // Roots revalidated in this scan: the recorded identity is a live reading.
    let roots: HashMap<u32, (&ScannedProcess, &V)> = managed
        .iter()
        .filter_map(|root| {
            let reading = &processes[*by_pid.get(&root.identity.pid)?];
            (reading.identity == root.identity)
                .then_some((root.identity.pid, (reading, &root.vendor)))
        })
        .collect();
    let parent = |child: &ScannedProcess| -> Option<&ScannedProcess> {
        let parent = &processes[*by_pid.get(&child.parent_pid)?];
        (parent.identity.pid != child.identity.pid
            && parent.identity.boot_id == child.identity.boot_id
            && parent.start_us()? <= child.start_us()?)
        .then_some(parent)
    };
    processes
        .iter()
        .map(|process| {
            let mut agent_vendor = None;
            let mut current = Some(process);
            for _ in 0..=MAX_ANCESTRY {
                let Some(node) = current else { break };
                if let Some((_, vendor)) = roots
                    .get(&node.identity.pid)
                    .filter(|(root, _)| verify(&process.identity, &root.identity))
                {
                    return Class::Proven((*vendor).clone());
                }
                if agent_vendor.is_none() {
                    agent_vendor = agent(&node.name);
                }
                current = parent(node);
            }
            match agent_vendor {
                Some(vendor) => Class::Estimated(vendor),
                None => Class::Unattributed,
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "attribution/tests.rs"]
mod tests;

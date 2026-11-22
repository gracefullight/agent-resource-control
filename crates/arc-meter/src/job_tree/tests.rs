use super::*;

const BOOT: &str = "01234567-89ab-cdef-0123-456789abcdef";

fn identity(pid: u32, start: u64) -> ProcessIdentity {
    ProcessIdentity {
        pid,
        start_token: start.to_string(),
        boot_id: BOOT.into(),
    }
}
fn native(pid: u32, parent: u32, cpu: u64, child: u64, rss: u64, io: (u64, u64)) -> NativeProcess {
    NativeProcess {
        identity: identity(pid, 1000 + u64::from(pid)),
        parent_pid: parent,
        process_group: 10,
        cpu_time_ns: cpu,
        child_cpu_time_ns: child,
        rss_bytes: rss,
        disk_read_bytes: io.0,
        disk_write_bytes: io.1,
    }
}
fn verified(_: &ProcessIdentity) -> IdentityStatus {
    IdentityStatus::VerifiedMatch
}

#[test]
fn revalidation_requires_the_root_and_classifies_each_member() {
    let root = identity(10, 1010);
    let tree = vec![
        native(10, 1, 5, 0, 100, (0, 0)),
        native(11, 10, 5, 0, 100, (0, 0)),
        native(12, 1, 5, 0, 100, (0, 0)),
        native(13, 12, 5, 0, 100, (0, 0)),
    ];
    let readings = revalidate(tree.clone(), &root, verified);
    let kinds: Vec<_> = readings.iter().map(|r| r.membership).collect();
    use Membership::*;
    assert_eq!(kinds, [Root, Nested, Detached, Nested]);

    // A recycled or exited root attributes nothing, even with live members.
    let mismatched = |id: &ProcessIdentity| {
        if id.pid == 10 {
            IdentityStatus::Mismatch
        } else {
            IdentityStatus::VerifiedMatch
        }
    };
    assert!(revalidate(tree.clone(), &root, mismatched).is_empty());
    let recycled = identity(10, 9999);
    assert!(revalidate(tree.clone(), &recycled, verified).is_empty());

    // A member that fails revalidation is dropped, and its child is detached.
    let gone = |id: &ProcessIdentity| {
        if id.pid == 12 {
            IdentityStatus::Missing
        } else {
            IdentityStatus::VerifiedMatch
        }
    };
    let readings = revalidate(tree, &root, gone);
    assert_eq!(
        readings
            .iter()
            .map(|r| (r.identity.pid, r.membership))
            .collect::<Vec<_>>(),
        [(10, Root), (11, Nested), (13, Detached)]
    );
}

#[test]
fn core_percent_is_one_hundred_per_core_from_consecutive_deltas() {
    assert_eq!(
        core_percent((0, 0), (1_000_000_000, 1_000_000_000)),
        Some(100.0)
    );
    assert_eq!(
        core_percent((0, 0), (2_000_000_000, 500_000_000)),
        Some(25.0)
    );
    assert_eq!(
        core_percent((0, 0), (1_000_000_000, 3_000_000_000)),
        Some(300.0)
    );
    assert_eq!(core_percent((5, 0), (5, 10)), None);
    assert_eq!(core_percent((6, 0), (5, 10)), None);
    assert_eq!(core_percent((0, 10), (5, 9)), None);
}

#[test]
fn totals_add_detached_cpu_only_and_keep_peaks_and_last_io() {
    let root = identity(10, 1010);
    let mut tree = JobTree::new(false);
    assert_eq!(tree.totals(), TreeTotals::default());
    let first = revalidate(
        vec![
            native(10, 1, 100, 0, 1000, (10, 20)),
            native(11, 10, 50, 0, 3000, (1, 2)),
            native(12, 1, 200, 0, 5000, (0, 4096)),
        ],
        &root,
        verified,
    );
    let rows = tree.observe(1_000, &first);
    assert!(rows.iter().all(|r| r.cpu_core_percent.is_none()));
    // The nested child was reaped into the root (child time); the detached
    // grandchild kept running.
    let second = revalidate(
        vec![
            native(10, 1, 120, 80, 900, (11, 20)),
            native(12, 1, 1200, 0, 4000, (0, 8192)),
        ],
        &root,
        verified,
    );
    let rows = tree.observe(2_000, &second);
    let percents: Vec<_> = rows.iter().map(|r| r.cpu_core_percent).collect();
    assert_eq!(percents, [Some(2.0), Some(100.0)]);
    let totals = tree.totals();
    assert_eq!(totals.root_cpu_ns, Some(200));
    assert_eq!(totals.detached_cpu_ns, 1200);
    assert_eq!(totals.peak_rss_bytes, Some(9000));
    // Disk counters are per process and never reaped: each identity's last.
    assert_eq!(totals.disk_read_bytes, Some(11 + 1));
    assert_eq!(totals.disk_write_bytes, Some(20 + 2 + 8192));
    assert_eq!((totals.samples, totals.processes), (2, 3));
    assert!(totals.partial);
}

#[test]
fn a_root_only_tree_is_not_partial_and_a_resumed_one_is() {
    let root = identity(10, 1010);
    let readings = revalidate(vec![native(10, 1, 5, 0, 7, (0, 0))], &root, verified);
    let mut tree = JobTree::new(false);
    tree.observe(1, &readings);
    assert!(!tree.totals().partial);
    let mut resumed = JobTree::new(true);
    resumed.observe(1, &readings);
    assert!(resumed.totals().partial);
    // An empty sample is not a zero sample.
    let mut empty = JobTree::new(false);
    assert!(empty.observe(1, &[]).is_empty());
    assert_eq!(empty.totals().samples, 0);
    assert_eq!(empty.totals().peak_rss_bytes, None);
}

#[test]
fn identities_beyond_the_bound_mark_the_tree_partial() {
    let root = identity(1, 1001);
    let mut readings = vec![native(1, 0, 0, 0, 1, (0, 0))];
    readings
        .extend((2..=MAX_TRACKED_PROCESSES as u32 + 1).map(|pid| native(pid, 1, 0, 0, 1, (0, 0))));
    let readings = revalidate(readings, &root, verified);
    let mut tree = JobTree::new(false);
    assert_eq!(tree.observe(1, &readings).len(), MAX_TRACKED_PROCESSES + 1);
    let totals = tree.totals();
    assert_eq!(totals.processes, MAX_TRACKED_PROCESSES as u64);
    assert!(totals.partial);
}

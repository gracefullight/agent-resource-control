//! Classification and delta rules over synthetic readings. The production
//! path (real daemon, installed hook, built arcctl, native scan) is
//! `arc-daemon tests/process_attribution.rs`.
use super::*;

const BOOT: &str = "00000000-0000-4000-8000-000000000000";

fn process(pid: u32, parent: u32, group: u32, start: u64, name: &str, cpu: u64) -> ScannedProcess {
    ScannedProcess {
        identity: ProcessIdentity {
            pid,
            start_token: start.to_string(),
            boot_id: BOOT.into(),
        },
        parent_pid: parent,
        process_group: group,
        name: name.into(),
        cpu_time_ns: cpu,
    }
}

fn agent(name: &str) -> Option<&'static str> {
    (name == "claude").then_some("claude-code")
}

fn root(pid: u32, start: u64) -> ManagedRoot<&'static str> {
    ManagedRoot {
        identity: process(pid, 0, 0, start, "", 0).identity,
        vendor: "claude-code",
    }
}

fn scan(processes: Vec<ScannedProcess>) -> Scan {
    Scan {
        processes,
        unreadable: 0,
    }
}

#[test]
fn managed_members_are_proven_agent_descendants_estimated_and_the_rest_unattributed() {
    let processes = vec![
        process(10, 1, 10, 100, "claude", 0),  // an agent
        process(11, 10, 11, 110, "zsh", 0),    // its shell
        process(20, 11, 20, 120, "cargo", 0),  // managed root under the agent
        process(21, 20, 20, 130, "rustc", 0),  // managed descendant
        process(22, 1, 20, 140, "orphan", 0),  // group alone does not prove ownership
        process(30, 1, 30, 150, "burner", 0),  // no agent ancestry
        process(31, 11, 31, 160, "burner", 0), // the agent's own descendant
    ];
    let classes = classify(&processes, &[root(20, 120)], agent);
    assert_eq!(
        classes,
        vec![
            Class::Estimated("claude-code"),
            Class::Estimated("claude-code"),
            Class::Proven("claude-code"),
            Class::Proven("claude-code"),
            Class::Unattributed,
            Class::Unattributed,
            Class::Estimated("claude-code"),
        ]
    );
}

#[test]
fn a_root_whose_recorded_identity_is_not_live_proves_nothing() {
    let processes = vec![
        process(20, 1, 20, 120, "cargo", 0),
        process(21, 20, 20, 130, "rustc", 0),
    ];
    // The recorded child names PID 20 with another start time (a recycled PID).
    let classes = classify(&processes, &[root(20, 119)], agent);
    assert_eq!(classes, vec![Class::Unattributed, Class::Unattributed]);
}

#[test]
fn a_parent_that_started_after_its_child_carries_no_ancestry() {
    // PID 10 was recycled into an agent after process 11 started.
    let processes = vec![
        process(10, 1, 10, 500, "claude", 0),
        process(11, 10, 11, 110, "burner", 0),
    ];
    let classes = classify(&processes, &[], agent);
    assert_eq!(
        classes,
        vec![Class::Estimated("claude-code"), Class::Unattributed]
    );
    // A group that the root does not lead proves nothing either.
    let processes = vec![
        process(20, 1, 99, 120, "cargo", 0),
        process(21, 1, 20, 130, "rustc", 0),
    ];
    let classes = classify(&processes, &[root(20, 120)], agent);
    assert_eq!(
        classes,
        vec![Class::Proven("claude-code"), Class::Unattributed]
    );
}

#[test]
fn a_parent_cycle_ends_the_walk() {
    let processes = vec![
        process(10, 11, 10, 100, "a", 0),
        process(11, 10, 11, 100, "b", 0),
    ];
    let classes = classify(&processes, &[], agent);
    assert_eq!(classes, vec![Class::Unattributed, Class::Unattributed]);
}

#[test]
fn only_a_second_sighting_of_the_same_identity_contributes_cpu() {
    let mut attributor = Attributor::default();
    let first = scan(vec![
        process(30, 1, 30, 150, "burner", 1_000),
        process(31, 1, 31, 160, "claude", 5_000),
    ]);
    let (totals, rows) =
        attributor.observe(&first, &[] as &[ManagedRoot<&str>], agent, |_, _| true);
    assert_eq!(totals.unattributed_ns, 0);
    assert!(totals.estimated_ns.is_empty());
    assert_eq!(totals.skipped, 2, "first sightings contribute nothing");
    assert!(rows.iter().all(|r| r.delta_ns.is_none()));

    let mut second = scan(vec![
        process(30, 1, 30, 150, "burner", 4_000),
        // PID 31 now names another process: a changed identity.
        process(31, 1, 31, 170, "claude", 9_000),
    ]);
    second.unreadable = 3;
    let (totals, rows) = attributor.observe(&second, &[], agent, |_, _| true);
    assert_eq!(totals.unattributed_ns, 3_000);
    assert!(totals.estimated_ns.is_empty(), "a changed identity adds 0");
    assert_eq!(totals.skipped, 1 + 3);
    assert_eq!(rows[0].delta_ns, Some(3_000));
    assert_eq!(rows[1].delta_ns, None);
    assert_eq!(
        (rows[0].cpu_time_ns, rows[1].cpu_time_ns),
        (4_000, 9_000),
        "each row carries its own reading"
    );

    let third = scan(vec![
        process(30, 1, 30, 150, "burner", 3_999), // a counter that fell
        process(31, 1, 31, 170, "claude", 9_500),
    ]);
    let (totals, _) = attributor.observe(&third, &[], agent, |_, _| true);
    assert_eq!(totals.unattributed_ns, 0);
    assert_eq!(totals.estimated_ns, BTreeMap::from([("claude-code", 500)]));
    assert_eq!(totals.skipped, 1);
}

#[test]
fn a_boot_change_never_contributes_a_delta() {
    let mut attributor = Attributor::default();
    let first = scan(vec![process(30, 1, 30, 150, "burner", 1_000)]);
    attributor.observe(&first, &[] as &[ManagedRoot<&str>], agent, |_, _| true);
    let mut second = scan(vec![process(30, 1, 30, 150, "burner", 9_000)]);
    second.processes[0].identity.boot_id = "00000000-0000-4000-8000-000000000001".into();
    let (totals, rows) = attributor.observe(&second, &[], agent, |_, _| true);
    assert_eq!(totals.unattributed_ns, 0);
    assert_eq!(rows[0].delta_ns, None);
}

#[test]
fn proven_cpu_is_grouped_by_the_roots_vendor_and_never_counted_twice() {
    let managed = [
        ManagedRoot {
            identity: process(20, 0, 0, 120, "", 0).identity,
            vendor: "none",
        },
        root(40, 140),
    ];
    let readings = |cpu: u64| {
        scan(vec![
            process(20, 1, 20, 120, "cargo", cpu),
            process(40, 1, 40, 140, "bun", 2 * cpu),
            process(41, 40, 40, 141, "claude", 3 * cpu), // an agent inside a job
        ])
    };
    let mut attributor = Attributor::default();
    attributor.observe(&readings(10), &managed, agent, |_, _| true);
    let (totals, _) = attributor.observe(&readings(20), &managed, agent, |_, _| true);
    assert_eq!(
        totals.proven_ns,
        BTreeMap::from([("claude-code", 20 + 30), ("none", 10)])
    );
    assert!(totals.estimated_ns.is_empty());
    assert_eq!(totals.unattributed_ns, 0);
}

#[test]
fn live_ownership_proof_is_required_for_each_managed_member() {
    let readings = scan(vec![
        process(20, 1, 20, 120, "cargo", 10),
        process(21, 20, 20, 130, "rustc", 10),
    ]);
    let mut attributor = Attributor::default();
    let (_, rows) = attributor.observe(&readings, &[root(20, 120)], agent, |member, _| {
        member.pid == 20
    });
    assert_eq!(rows[0].class, Class::Proven("claude-code"));
    assert_eq!(rows[1].class, Class::Unattributed);
}

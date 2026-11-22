//! Fair queue-head selection across projects (AT-TRF-08), on the repository
//! functions the daemon's writer calls. Times are supplied, never measured.
use super::*;
use arc_ipc::traffic::fairness::ProjectKey;
use arc_policy::{evaluator::MissingObservation, DecisionReason, FairnessReason, NegotiatedReason};

const A: u32 = 0xa;
const B: u32 = 0xb;

fn add_in(c: &mut Connection, n: u32, class: &str, project: Option<u32>, now: u64) -> Reply {
    let request = Request::Enqueue {
        job_id: id(n),
        observer: identity(),
        classification: classification(class),
        lineage_context: None,
        project: project.map(|p| ProjectKey::try_from(format!("{p:064x}")).unwrap()),
        event: None,
        usage: None,
    };
    enqueue(
        c,
        &request,
        &OwnerEvidence::gather(&identity()),
        &roomy(),
        &context(n),
        None,
        now,
    )
    .unwrap()
}

/// Windows of one per class with a budget that never holds a job back, so
/// only the queue head and the class window decide.
fn roomy() -> Policy {
    let mut policy = Policy::default();
    policy.budget.cpu_millis = 64_000;
    policy.budget.memory_bytes *= 16;
    policy
}

fn poll_in(c: &mut Connection, n: u32, now: u64) -> Reply {
    poll_job(c, n, now, &roomy())
}

fn finish(c: &mut Connection, n: u32, granted: &Reply, now: u64) {
    complete_job(c, n, generation(granted), Outcome::Exited { code: 0 }, now);
}

/// The raw recorded decision and reason of one job, newest first.
fn recorded(c: &Connection, n: u32) -> Vec<(String, String)> {
    c.prepare(
        "SELECT decision,reason_code FROM policy_decisions WHERE job_id=?1 ORDER BY rowid DESC",
    )
    .unwrap()
    .query_map([id(n).as_str()], |row| Ok((row.get(0)?, row.get(1)?)))
    .unwrap()
    .map(Result::unwrap)
    .collect()
}

fn last(c: &Connection, n: u32) -> (String, String) {
    recorded(c, n).remove(0)
}

fn pair(kind: &str, reason: &str) -> (String, String) {
    (kind.to_owned(), reason.to_owned())
}

fn waits(reply: &Reply) -> WaitReason {
    match reply {
        Reply::Queued { reason, .. } => *reason,
        other => panic!("expected a queued reply, got {other:?}"),
    }
}

#[test]
fn a_later_project_is_granted_after_one_job_of_the_earlier_project() {
    let db = Database::new();
    let mut c = db.open();
    let held = add_in(&mut c, 1, "BUILD", Some(0xe), 100);
    // Project A queues jobs 2 to 5, then project B queues job 6.
    for n in 2..=5 {
        let reply = add_in(&mut c, n, "BUILD", Some(A), 100 + u64::from(n));
        let expected = if n == 2 {
            WaitReason::ClassWindow
        } else {
            WaitReason::Fifo
        };
        assert_eq!(waits(&reply), expected, "{n}");
    }
    assert_eq!(
        waits(&add_in(&mut c, 6, "BUILD", Some(B), 110)),
        WaitReason::Fifo
    );
    assert_eq!(last(&c, 6), pair("wait", "fifo"));
    finish(&mut c, 1, &held, 120);

    // No project of the queue has been granted: the earliest entry is first.
    assert_eq!(waits(&poll_in(&mut c, 6, 121)), WaitReason::Fifo);
    assert_eq!(last(&c, 6), pair("wait", "fifo"));
    let first = poll_in(&mut c, 2, 122);
    assert!(matches!(first, Reply::Granted { .. }), "{first:?}");
    assert_eq!(last(&c, 2), pair("admit", "admitted"));

    // Project B has waited longest for a grant: its job is the head now.
    // The earlier jobs of project A still get the "not the head" reply, and
    // their recorded cause is the class window, never an earlier job.
    for n in 3..=5 {
        assert_eq!(waits(&poll_in(&mut c, n, 123)), WaitReason::Fifo, "{n}");
        assert_eq!(last(&c, n), pair("wait", "class_window"), "{n}");
    }
    assert_eq!(waits(&poll_in(&mut c, 6, 124)), WaitReason::ClassWindow);
    finish(&mut c, 2, &first, 130);
    // A released window does not let project A in ahead of project B.
    for n in 3..=5 {
        assert_eq!(waits(&poll_in(&mut c, n, 131)), WaitReason::Fifo, "{n}");
    }
    assert_eq!(permit_counts(&c), (2, 0));
    let second = poll_in(&mut c, 6, 132);
    assert!(matches!(second, Reply::Granted { .. }), "{second:?}");

    // The fair turn is the grant decision of that job, raw in SQLite and
    // through the negotiated read, and `admitted` to every other reader.
    assert_eq!(last(&c, 6), pair("admit", "project_fair_turn"));
    let fair = Some(NegotiatedReason::Fairness(FairnessReason::ProjectFairTurn));
    assert_eq!(
        crate::policy::fairness::negotiated(&c, &id(6), 133).unwrap(),
        fair
    );
    let projected = diagnostics(&mut c, &id(6), &roomy(), 133)
        .unwrap()
        .admission
        .unwrap()
        .last_decision
        .unwrap();
    assert_eq!(projected.reason, DecisionReason::Admitted);
    // A repeated poll and the launch-time decision keep it the last reason.
    let decisions = recorded(&c, 6).len();
    assert_eq!(poll_in(&mut c, 6, 134), second);
    crate::policy::transaction(&mut c, &MissingObservation, |tx, effective| {
        crate::policy::fairness::keep(
            tx,
            &id(6),
            effective,
            arc_policy::DecisionKind::Admit,
            DecisionReason::Admitted,
            Some(generation(&second)),
            135,
        )
    })
    .unwrap();
    assert_eq!(recorded(&c, 6).len(), decisions);
    assert_eq!(last(&c, 6), pair("admit", "project_fair_turn"));
    finish(&mut c, 6, &second, 140);
    assert_eq!(
        crate::policy::fairness::negotiated(&c, &id(6), 141).unwrap(),
        fair
    );

    // Project A continues first in, first out; none of its grants is fair.
    for n in 3..=5 {
        for later in (n + 1..=5).rev() {
            let now = 140 + u64::from(n) * 10;
            assert_eq!(waits(&poll_in(&mut c, later, now)), WaitReason::Fifo);
            assert_eq!(last(&c, later), pair("wait", "fifo"), "{later}");
        }
        let granted = poll_in(&mut c, n, 141 + u64::from(n) * 10);
        assert!(matches!(granted, Reply::Granted { .. }), "{n}: {granted:?}");
        assert_eq!(last(&c, n), pair("admit", "admitted"), "{n}");
        finish(&mut c, n, &granted, 142 + u64::from(n) * 10);
    }
    assert_eq!(permit_counts(&c), (6, 0));
}

/// Poll every queued job, latest first, and return the one that is granted.
fn next_grant(c: &mut Connection, queued: &mut Vec<u32>, now: u64) -> (u32, Reply) {
    let mut granted = None;
    for n in queued.iter().rev() {
        let reply = poll_in(c, *n, now);
        if matches!(reply, Reply::Granted { .. }) {
            assert!(granted.is_none(), "two grants at a window of one");
            granted = Some((*n, reply));
        }
    }
    let (n, reply) = granted.expect("one queued job is granted");
    queued.retain(|other| *other != n);
    (n, reply)
}

fn drain(c: &mut Connection, mut queued: Vec<u32>, mut now: u64) -> Vec<u32> {
    let mut order = Vec::new();
    while !queued.is_empty() {
        now += 10;
        let (n, reply) = next_grant(c, &mut queued, now);
        finish(c, n, &reply, now + 1);
        order.push(n);
    }
    order
}

#[test]
fn projects_take_turns_and_each_stays_first_in_first_out() {
    let db = Database::new();
    let mut c = db.open();
    let held = add_in(&mut c, 1, "BUILD", Some(0xe), 100);
    for n in 2..=4 {
        add_in(&mut c, n, "BUILD", Some(A), 100 + u64::from(n));
    }
    for n in 5..=7 {
        add_in(&mut c, n, "BUILD", Some(B), 100 + u64::from(n));
    }
    finish(&mut c, 1, &held, 110);
    assert_eq!(drain(&mut c, (2..=7).collect(), 200), [2, 5, 3, 6, 4, 7]);
    // Jobs 5 and 6 passed earlier entries; job 7 was the earliest by then.
    for (n, reason) in [
        (5, "project_fair_turn"),
        (6, "project_fair_turn"),
        (7, "admitted"),
    ] {
        assert_eq!(last(&c, n), pair("admit", reason), "{n}");
    }
    assert_eq!(permit_counts(&c), (7, 0));
}

#[test]
fn a_keyless_entry_passes_nothing_and_is_passed_by_nothing() {
    let db = Database::new();
    let mut c = db.open();
    let held = add_in(&mut c, 1, "BUILD", None, 100);
    // A, A, keyless, A, B. Project B has never been granted, yet it may not
    // pass the keyless entry, and that entry waits for both earlier jobs of
    // project A. Behind it the two projects take fair turns again.
    add_in(&mut c, 2, "BUILD", Some(A), 101);
    add_in(&mut c, 3, "BUILD", Some(A), 102);
    add_in(&mut c, 4, "BUILD", None, 103);
    add_in(&mut c, 5, "BUILD", Some(A), 104);
    add_in(&mut c, 6, "BUILD", Some(B), 105);
    finish(&mut c, 1, &held, 110);
    assert_eq!(drain(&mut c, (2..=6).collect(), 200), [2, 3, 4, 6, 5]);
    assert_eq!(last(&c, 6), pair("admit", "project_fair_turn"));
    for n in [2, 3, 4, 5] {
        assert_eq!(last(&c, n), pair("admit", "admitted"), "{n}");
    }
}

#[test]
fn recency_is_counted_within_the_class_and_a_project_keeps_its_own_order() {
    let db = Database::new();
    let mut c = db.open();
    // Project B holds the Build window; project A is then granted a Test.
    let build = add_in(&mut c, 1, "BUILD", Some(B), 100);
    let test = add_in(&mut c, 2, "TEST", Some(A), 101);
    assert!(matches!(test, Reply::Granted { .. }), "{test:?}");
    // Queue: B build, A build, A test, A build.
    add_in(&mut c, 3, "BUILD", Some(B), 102);
    add_in(&mut c, 4, "BUILD", Some(A), 103);
    add_in(&mut c, 5, "TEST", Some(A), 104);
    add_in(&mut c, 6, "BUILD", Some(A), 105);
    finish(&mut c, 1, &build, 110);
    // Project A was granted last overall, but never a Build: it goes first.
    let mut queued = vec![3, 4, 5, 6];
    let (n, first) = next_grant(&mut c, &mut queued, 120);
    assert_eq!(n, 4);
    assert_eq!(last(&c, 4), pair("admit", "project_fair_turn"));
    finish(&mut c, 2, &test, 121);
    finish(&mut c, 4, &first, 122);
    // Then B's build; A's later build never passes A's own earlier test.
    assert_eq!(drain(&mut c, queued, 200), [3, 5, 6]);
    assert_eq!(permit_counts(&c), (6, 0));
}

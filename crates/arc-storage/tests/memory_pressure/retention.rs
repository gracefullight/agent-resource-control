use super::{admission::*, observation::*};
use arc_core::meter::MemoryPressure;
use arc_ipc::{policy::DecisionAvailability, traffic::JobRequest};
use arc_policy::*;
use arc_storage::{jobs::traffic::*, policy};

#[test]
fn semantic_transitions_dedupe_sequences_ages_and_time_and_bound_each_scope() {
    let mut c = database();
    let p = Provider::new(MemoryPressure::Critical);
    add(&mut c, 1, &p);
    let first = policy::job(&c, &id(1), p.policy(), 1000)
        .unwrap()
        .last_decision
        .unwrap();
    // Poll time, ordinary sequence changes and measurement age are not transitions.
    for now in 1001..1100 {
        p.set(MemoryPressure::Critical);
        p.0.lock().unwrap().time(now);
        poll_job(&mut c, 1, &p);
    }
    let last = policy::job(&c, &id(1), p.policy(), 1100)
        .unwrap()
        .last_decision
        .unwrap();
    assert_eq!(first, last);
    assert_eq!(last.observation.age_ms, Some(0));
    // A->B->A at equal/backwards wall timestamps must remain three transitions.
    for i in 0..300 {
        let mut effective = p.policy();
        effective.observation.availability = ObservationAvailability::Invalid;
        effective.observation.pressure = None;
        effective.mode = PolicyMode::L0;
        effective.reason = PolicyReason::ObservationInvalid;
        effective.observation.sampler.as_mut().unwrap().generation = (i % 2) + 1;
        let now = 2000 - i;
        policy::transaction(&mut c, &p, |tx, _| {
            policy::record(
                tx,
                &id(1),
                &effective,
                DecisionKind::Wait,
                DecisionReason::Fifo,
                None,
                now,
            )
        })
        .unwrap();
    }
    for (scope, expected) in [(true, MAX_JOB_DECISIONS), (false, MAX_GLOBAL_DECISIONS)] {
        let count: i64 = c
            .query_row(
                "SELECT count(*) FROM policy_decisions WHERE (job_id IS NOT NULL)=?1",
                [scope],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, expected as i64);
    }
    let read = policy::job(&c, &id(1), p.policy(), 3000).unwrap();
    assert_eq!(read.last_decision.unwrap().decided_at_ms, 1701);
}
#[test]
fn ninety_days_are_anchored_to_terminal_lifecycle_not_poll_time() {
    let mut c = database();
    let p = Provider::new(MemoryPressure::Normal);
    let generation = generation(add(&mut c, 1, &p));
    release(&mut c, 1, generation);
    let deadline = 1002 + DECISION_RETENTION_MS;
    assert_eq!(
        policy::job(&c, &id(1), p.policy(), deadline - 1)
            .unwrap()
            .decision_availability,
        DecisionAvailability::Available
    );
    assert_eq!(
        policy::job(&c, &id(1), p.policy(), deadline)
            .unwrap()
            .decision_availability,
        DecisionAvailability::Expired
    );
    assert!(policy::expire(&mut c, deadline).unwrap() <= MAX_EXPIRY_BATCH);
    assert_eq!(
        policy::job(&c, &id(1), p.policy(), deadline)
            .unwrap()
            .decision_availability,
        DecisionAvailability::Expired
    );
    let remaining:i64=c.query_row("SELECT count(*) FROM policy_decisions WHERE job_id=?1 AND observation_json IS NOT NULL",[id(1).as_str()],|r|r.get(0)).unwrap();
    assert_eq!(remaining, 0);
    // Active work is not expired from its old registration time.
    add(&mut c, 2, &p);
    assert_eq!(
        policy::job(&c, &id(2), p.policy(), deadline + 1)
            .unwrap()
            .decision_availability,
        DecisionAvailability::Available
    );
}
#[test]
fn queued_cancellation_has_a_retention_anchor_and_cannot_renew_it() {
    let mut c = database();
    let p = Provider::new(MemoryPressure::Critical);
    add(&mut c, 1, &p);
    let request = JobRequest::Cancel {
        job_id: id(1),
        observer: owner(),
        permit_generation: None,
    };
    cancel_observed(&mut c, &request, &OwnerEvidence::gather(&owner()), 1001, &p).unwrap();
    cancel_observed(&mut c, &request, &OwnerEvidence::gather(&owner()), 1002, &p).unwrap();
    let finished: i64 = c
        .query_row(
            "SELECT created_at FROM audit_events WHERE id='traffic-policy-terminal-'||?1",
            [id(1).as_str()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(finished, 1001);
    assert_eq!(
        policy::job(&c, &id(1), p.policy(), 1001 + DECISION_RETENTION_MS)
            .unwrap()
            .decision_availability,
        DecisionAvailability::Expired
    );
}
#[test]
fn expiry_is_bounded_and_last_job_lookup_uses_the_registered_index() {
    let mut c = database();
    let p = Provider::new(MemoryPressure::Critical);
    for n in 1..=200 {
        add(&mut c, n, &p);
    }
    for n in 1..=200 {
        let request = JobRequest::Cancel {
            job_id: id(n),
            observer: owner(),
            permit_generation: None,
        };
        cancel_observed(&mut c, &request, &OwnerEvidence::gather(&owner()), 1001, &p).unwrap();
    }
    let count = policy::expire(&mut c, DECISION_RETENTION_MS + 1001).unwrap();
    assert_eq!(count, MAX_EXPIRY_BATCH);
    let read = policy::job(&c, &id(200), p.policy(), DECISION_RETENTION_MS + 1001).unwrap();
    assert_eq!(
        read.decision_availability,
        DecisionAvailability::Expired,
        "read enforces expiry before cleanup reaches it"
    );
    let mut stmt=c.prepare("EXPLAIN QUERY PLAN SELECT observation_json FROM policy_decisions WHERE job_id=?1 ORDER BY rowid DESC LIMIT 1").unwrap();
    let plan: Vec<String> = stmt
        .query_map([id(1).as_str()], |r| r.get(3))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert!(
        plan.iter()
            .any(|line| line.contains("idx_policy_decisions_job_time")),
        "{plan:?}"
    );
    assert_eq!(
        policy::job(&c, &id(999), p.policy(), 1000)
            .unwrap()
            .decision_availability,
        DecisionAvailability::NotFound
    );
}

#[test]
fn a_referenced_reservation_protects_only_the_latest_due_decision() {
    let mut c = database();
    let p = Provider::new(MemoryPressure::Normal);
    let root_generation = generation(add(&mut c, 1, &p));
    p.set(MemoryPressure::Critical);
    poll_job(&mut c, 1, &p);
    p.set(MemoryPressure::Normal);
    poll_job(&mut c, 1, &p);
    let snapshot = lineage_snapshot(&c, &context(1)).unwrap().unwrap();
    let proof = LineageProof::verified_with_recheck(&snapshot, &owner(), |_, _| true); // simulated native ancestry
    let nested = enqueue_observed(
        &mut c,
        &request(2, "CARGO_BUILD"),
        &OwnerEvidence::gather(&owner()),
        &arc_traffic::Policy::default(),
        &context(2),
        Some(&proof),
        1001,
        &p,
    )
    .unwrap();
    assert!(matches!(nested, arc_ipc::traffic::JobResult::Reused { .. }));
    release(&mut c, 1, root_generation);
    let deadline = 1002 + DECISION_RETENTION_MS;
    assert_eq!(
        policy::job(&c, &id(1), p.policy(), deadline)
            .unwrap()
            .decision_availability,
        DecisionAvailability::Available
    );
    policy::expire(&mut c, deadline).unwrap();
    let remaining: i64 = c
        .query_row(
            "SELECT count(*) FROM policy_decisions WHERE job_id=?1",
            [id(1).as_str()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(remaining, 1);
    complete(
        &mut c,
        &JobRequest::Complete {
            job_id: id(2),
            observer: owner(),
            permit_generation: root_generation,
            outcome: arc_ipc::jobs::Outcome::Exited { code: 0 },
        },
        deadline + 1,
    )
    .unwrap();
    policy::expire(&mut c, deadline + 1).unwrap();
    assert_eq!(
        policy::job(&c, &id(1), p.policy(), deadline + 1)
            .unwrap()
            .decision_availability,
        DecisionAvailability::Expired
    );
    assert_eq!(
        c.query_row(
            "SELECT count(*) FROM policy_decisions WHERE job_id=?1",
            [id(1).as_str()],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
}

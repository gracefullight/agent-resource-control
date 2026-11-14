use super::*;

#[test]
fn typecheck_cross_class_cpu_and_memory_contention_are_independent() {
    for memory in [false, true] {
        let db = Database::new();
        let mut c = db.open();
        let mut policy = Policy::default();
        policy.build.reservation = policy.test.reservation;
        if memory {
            policy.budget.cpu_millis = 4000;
            policy.budget.memory_bytes = policy.test.reservation.memory_bytes;
        } else {
            policy.budget.cpu_millis = policy.test.reservation.cpu_millis;
        }
        let first = add(&mut c, 1, "TEST", 100, &policy);
        let request = script_request(2, JobClass::Typecheck, Reason::BunScriptTypecheck);
        let queued = enqueue(
            &mut c,
            &request,
            &OwnerEvidence::gather(&identity()),
            &policy,
            &context(2),
            None,
            101,
        )
        .unwrap();
        assert!(matches!(queued, Reply::Queued { reason, .. }
            if reason == if memory { WaitReason::MemoryBudget } else { WaitReason::CpuBudget }));
        let read = diagnostics(&mut c, &id(2), &policy, 102).unwrap();
        assert_eq!(
            read.limits.typecheck.reservation,
            policy.typecheck.reservation
        );
        assert_eq!(
            read.admission.unwrap().wait_reason,
            Some(if memory {
                arc_policy::DecisionReason::MemoryBudget
            } else {
                arc_policy::DecisionReason::CpuBudget
            })
        );
        assert_eq!(active_reservations(&c).unwrap().typecheck, 0);
        complete_job(
            &mut c,
            1,
            generation(&first),
            Outcome::Exited { code: 0 },
            103,
        );
        let next = poll_job(&mut c, 2, 104, &policy);
        assert_eq!(active_reservations(&c).unwrap().typecheck, 1);
        complete_job(
            &mut c,
            2,
            generation(&next),
            Outcome::Exited { code: 17 },
            105,
        );
        assert_eq!(
            active_reservations(&c).unwrap(),
            arc_traffic::Active::default()
        );
    }
}

#[test]
fn typecheck_owner_loss_recovery_preserves_unknown_outcome_and_releases_once() {
    let db = Database::new();
    let mut c = db.open();
    let policy = Policy::default();
    let grant = submit(
        &mut c,
        1,
        JobClass::Typecheck,
        Reason::BunScriptTypecheck,
        100,
    );
    attach(
        &mut c,
        &Request::Attach {
            job_id: id(1),
            observer: identity(),
            child: identity(),
            permit_generation: generation(&grant),
        },
        &ChildEvidence::gather(&identity()),
        101,
    )
    .unwrap();
    submit(
        &mut c,
        2,
        JobClass::Typecheck,
        Reason::BunScriptTypecheck,
        102,
    );
    drop(c);
    let mut c = db.open();
    let job = get(&c, &id(1)).unwrap().unwrap();
    let observation = crate::jobs::reconcile::simulated_pair(
        &job,
        IdentityStatus::Missing,
        IdentityStatus::Missing,
        103,
    );
    crate::jobs::apply_observation(&mut c, &observation).unwrap();
    assert_eq!(
        crate::jobs::apply_observation(&mut c, &observation).unwrap(),
        crate::jobs::ReconcileResult::Stale
    );
    let abandoned = diagnostics(&mut c, &id(1), &policy, 104).unwrap();
    assert_eq!(
        abandoned.classification.unwrap().classification,
        JobClass::Typecheck
    );
    let abandoned = abandoned.admission.unwrap();
    assert_eq!(
        abandoned.state,
        arc_ipc::traffic::diagnostics::AdmissionState::Abandoned
    );
    assert!(abandoned.outcome.is_none() && !abandoned.permit_active);
    let next = poll_job(&mut c, 2, 105, &policy);
    complete_job(
        &mut c,
        2,
        generation(&next),
        Outcome::Exited { code: 0 },
        106,
    );
    assert_eq!(permit_counts(&c), (2, 0));
}

#[test]
fn concurrent_typecheck_retries_and_fifo_pollers_cannot_double_reserve() {
    use std::sync::{Arc, Barrier};
    let db = Database::new();
    let mut c = db.open();
    let barrier = Arc::new(Barrier::new(3));
    let threads: Vec<_> = [10, 20]
        .into_iter()
        .map(|salt| {
            let path = db.0.clone();
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                let mut c = crate::open_and_migrate(path).unwrap();
                barrier.wait();
                enqueue(
                    &mut c,
                    &script_request(1, JobClass::Typecheck, Reason::BunScriptTypecheck),
                    &OwnerEvidence::gather(&identity()),
                    &Policy::default(),
                    &context(salt),
                    None,
                    100,
                )
                .unwrap()
            })
        })
        .collect();
    barrier.wait();
    let replies: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    assert_eq!(replies[0], replies[1]);
    assert_eq!(permit_counts(&c), (1, 1));
    for n in [2, 3] {
        submit(
            &mut c,
            n,
            JobClass::Typecheck,
            Reason::BunScriptTypecheck,
            101,
        );
    }
    complete_job(
        &mut c,
        1,
        generation(&replies[0]),
        Outcome::Exited { code: 0 },
        102,
    );
    let barrier = Arc::new(Barrier::new(3));
    let threads: Vec<_> = [2, 3]
        .into_iter()
        .map(|n| {
            let path = db.0.clone();
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                let mut c = crate::open_and_migrate(path).unwrap();
                barrier.wait();
                (n, poll_job(&mut c, n, 103, &Policy::default()))
            })
        })
        .collect();
    barrier.wait();
    let replies: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    for (n, reply) in replies {
        if n == 2 {
            assert!(matches!(reply, Reply::Granted { .. }));
            complete_job(
                &mut c,
                n,
                generation(&reply),
                Outcome::Exited { code: 0 },
                104,
            );
        } else {
            assert!(matches!(reply, Reply::Queued { .. }));
        }
    }
    let next = poll_job(&mut c, 3, 105, &Policy::default());
    complete_job(
        &mut c,
        3,
        generation(&next),
        Outcome::Exited { code: 0 },
        106,
    );
    assert_eq!(permit_counts(&c), (3, 0));
}

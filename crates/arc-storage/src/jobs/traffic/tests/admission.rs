use super::*;

#[test]
fn historical_completed_without_exit_has_a_non_retryable_status() {
    let db = Database::new();
    let mut c = db.open();
    let policy = Policy::default();
    for (n, clear_kind) in [(1, true), (2, false)] {
        let grant = add(&mut c, n, "BUILD", 100 + u64::from(n) * 3, &policy);
        complete_job(
            &mut c,
            n,
            generation(&grant),
            Outcome::Exited { code: 17 },
            101 + u64::from(n) * 3,
        );
        c.execute(
            "UPDATE jobs SET state='COMPLETED',exit_code=NULL WHERE id=?1",
            [id(n).as_str()],
        )
        .unwrap();
        if clear_kind {
            c.execute(
                "UPDATE job_lifecycle SET outcome_kind=NULL WHERE job_id=?1",
                [id(n).as_str()],
            )
            .unwrap();
        }
        let job = get(&c, &id(n)).unwrap().unwrap();
        assert_eq!(job.phase, arc_ipc::jobs::Phase::OutcomeUnknown);
        assert_eq!(job.outcome, None);
        assert_eq!(status(&c, &id(n)), Err(JobError::UnsupportedStatus));
        assert_eq!(
            JobError::UnsupportedStatus.code(),
            arc_ipc::ERROR_UNSUPPORTED_CAPABILITY
        );
        let observer = identity();
        let owner = OwnerEvidence::gather(&observer);
        assert_eq!(
            poll(
                &mut c,
                &Request::Poll {
                    job_id: id(n),
                    observer: observer.clone(),
                },
                &owner,
                &policy,
                &context(n),
                200,
            ),
            Err(JobError::InvalidTransition)
        );
        assert_eq!(
            cancel(
                &mut c,
                &Request::Cancel {
                    job_id: id(n),
                    observer,
                    permit_generation: None,
                },
                201,
            ),
            Err(JobError::InvalidTransition)
        );
        assert_eq!(
            fallback(&mut c, &fallback_request(n, None, None, None), None, 202),
            Err(JobError::InvalidTransition)
        );
        assert_eq!(permit_counts(&c), (i64::from(n), 0));
    }
    drop(c);
    let c = db.open();
    for n in [1, 2] {
        assert_eq!(status(&c, &id(n)), Err(JobError::UnsupportedStatus));
    }
    // The exception is limited to historical COMPLETED rows. Invalid new
    // state/outcome combinations must still report a storage failure.
    c.execute(
        "UPDATE jobs SET state='FAILED' WHERE id=?1",
        [id(1).as_str()],
    )
    .unwrap();
    assert_eq!(status(&c, &id(1)), Err(JobError::StorageFailure));
}

#[test]
fn durable_fifo_bypass_and_idempotent_completion() {
    let db = Database::new();
    let mut c = db.open();
    let policy = Policy::default();
    let first = add(&mut c, 1, "BUILD", 100, &policy);
    let first_gen = generation(&first);
    let second = add(&mut c, 2, "BUILD", 100, &policy);
    let third = add(&mut c, 3, "TEST", 100, &policy);
    assert!(matches!(
        second,
        Reply::Queued {
            sequence: 2,
            reason: WaitReason::ClassWindow,
            ..
        }
    ));
    assert!(matches!(
        third,
        Reply::Queued {
            sequence: 3,
            reason: WaitReason::Fifo,
            ..
        }
    ));
    assert_eq!(queue_head(&c).unwrap().unwrap().job_id, id(2));
    assert_eq!(active_reservations(&c).unwrap().build, 1);
    assert_eq!(
        active_reservations(&c).unwrap().resources,
        policy.build.reservation
    );
    assert_eq!(add(&mut c, 1, "BUILD", 101, &policy), first);
    for (n, class, expected) in [
        (4, "REMOTE_EXEC", BypassReason::Remote),
        (5, "UNKNOWN", BypassReason::Unknown),
    ] {
        assert_eq!(
            add(&mut c, n, class, 102, &policy),
            Reply::Bypassed { reason: expected }
        );
        assert!(get(&c, &id(n)).unwrap().is_none());
        assert_eq!(status(&c, &id(n)).unwrap(), StatusResult::NotFound {});
    }
    assert_eq!(permit_counts(&c), (1, 1));
    drop(c);
    let mut c = db.open();
    assert_eq!(poll_job(&mut c, 2, 104, &policy), second);
    let completed = complete_job(&mut c, 1, first_gen, Outcome::Exited { code: 0 }, 105);
    assert_eq!(
        complete_job(&mut c, 1, first_gen, Outcome::Exited { code: 0 }, 106),
        completed
    );
    assert!(matches!(
        poll_job(&mut c, 3, 106, &policy),
        Reply::Queued {
            reason: WaitReason::Fifo,
            ..
        }
    ));
    let second_grant = poll_job(&mut c, 2, 107, &policy);
    assert_eq!(generation(&second_grant), 2);
    assert_eq!(poll_job(&mut c, 2, 108, &policy), second_grant);
    let second_done = complete_job(&mut c, 2, 2, Outcome::Exited { code: 3 }, 109);
    assert_eq!(
        second_done,
        Reply::Completed {
            queue_wait_ms: 7,
            outcome: Outcome::Exited { code: 3 }
        }
    );
    assert_eq!(generation(&poll_job(&mut c, 3, 110, &policy)), 3);
    assert_eq!(permit_counts(&c), (3, 1));
    let released: Vec<i64> = c
        .prepare("SELECT released_at FROM permits WHERE state='RELEASED' ORDER BY generation")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(released, [105, 109]);
}

#[test]
fn exact_resource_budgets_and_strict_head_are_atomic() {
    for wait_memory in [false, true] {
        let db = Database::new();
        let mut c = db.open();
        let mut policy = Policy::default();
        policy.budget.cpu_millis = if wait_memory { 3000 } else { 1500 };
        policy.budget.memory_bytes = if wait_memory {
            1536 * 1024 * 1024
        } else {
            4 * 1024 * 1024 * 1024
        };
        policy.build.reservation.cpu_millis = 1000;
        policy.build.reservation.memory_bytes = 1024 * 1024 * 1024;
        assert!(matches!(
            add(&mut c, 1, "TEST", 10, &policy),
            Reply::Granted { .. }
        ));
        let queued = add(&mut c, 2, "LINT", 10, &policy);
        assert!(
            matches!(
                queued,
                Reply::Queued {
                    reason: WaitReason::CpuBudget,
                    ..
                }
            ) && !wait_memory
                || matches!(
                    queued,
                    Reply::Queued {
                        reason: WaitReason::MemoryBudget,
                        ..
                    }
                ) && wait_memory
        );
        assert_eq!(permit_counts(&c), (1, 1));
        complete_job(&mut c, 1, 1, Outcome::Exited { code: 0 }, 11);
        assert!(matches!(
            poll_job(&mut c, 2, 12, &policy),
            Reply::Granted { .. }
        ));
        assert_eq!(permit_counts(&c), (2, 1));
    }
}

#[test]
fn conflicting_retries_and_rollbacks_cannot_duplicate_or_change_a_grant() {
    let db = Database::new();
    let mut c = db.open();
    let mut policy = Policy::default();
    policy.build.window = 2;
    policy.budget.cpu_millis = 4000;
    let first = add(&mut c, 1, "BUILD", 10, &policy);
    let owner = OwnerEvidence::gather(&identity());
    assert_eq!(
        enqueue(
            &mut c,
            &request(1, "TEST", None),
            &owner,
            &policy,
            &context(99),
            None,
            11
        ),
        Err(JobError::Conflict)
    );
    let mut wrong_owner = identity();
    wrong_owner.start_token = if wrong_owner.start_token == "1" {
        "2"
    } else {
        "1"
    }
    .into();
    let wrong = Request::Poll {
        job_id: id(1),
        observer: wrong_owner.clone(),
    };
    assert_eq!(
        poll(
            &mut c,
            &wrong,
            &OwnerEvidence::simulated(&wrong_owner, IdentityStatus::VerifiedMatch),
            &policy,
            &context(2),
            11
        ),
        Err(JobError::Conflict)
    );
    // The second insert reaches the permit INSERT and then violates the unique
    // context index. Both its job and permit must roll back with the transaction.
    assert_eq!(
        enqueue(
            &mut c,
            &request(2, "BUILD", None),
            &owner,
            &policy,
            &context(1),
            None,
            11
        ),
        Err(JobError::StorageFailure)
    );
    assert!(get(&c, &id(2)).unwrap().is_none());
    assert_eq!(permit_counts(&c), (1, 1));
    assert_eq!(poll_job(&mut c, 1, 12, &policy), first);
    assert_eq!(
        complete(
            &mut c,
            &Request::Complete {
                job_id: id(1),
                observer: identity(),
                outcome: Outcome::Exited { code: 0 },
                permit_generation: 999
            },
            13
        ),
        Err(JobError::Conflict)
    );
    assert_eq!(permit_counts(&c), (1, 1));
    assert!(matches!(
        add(&mut c, 2, "BUILD", 14, &policy),
        Reply::Granted { .. }
    ));
    assert_eq!(permit_counts(&c), (2, 2));
}

#[test]
fn concurrent_pollers_serialize_the_capacity_decision() {
    use std::sync::{Arc, Barrier};
    use std::thread;
    let db = Database::new();
    let mut c = db.open();
    let policy = Policy::default();
    add(&mut c, 1, "BUILD", 10, &policy);
    add(&mut c, 2, "BUILD", 11, &policy);
    add(&mut c, 3, "BUILD", 11, &policy);
    complete_job(&mut c, 1, 1, Outcome::Exited { code: 0 }, 12);
    let barrier = Arc::new(Barrier::new(3));
    let handles: Vec<_> = [2, 3]
        .into_iter()
        .map(|n| {
            let path = db.0.clone();
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                let mut connection = crate::open_and_migrate(path).unwrap();
                barrier.wait();
                (n, poll_job(&mut connection, n, 13, &Policy::default()))
            })
        })
        .collect();
    barrier.wait();
    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert!(results
        .iter()
        .any(|(id, result)| *id == 2 && matches!(result, Reply::Granted { .. })));
    assert!(results
        .iter()
        .any(|(id, result)| *id == 3 && matches!(result, Reply::Queued { .. })));
    assert_eq!(permit_counts(&c), (2, 1));
    drop(c);
    let c = db.open();
    assert_eq!(permit_counts(&c), (2, 1));
}

#[test]
fn preexisting_active_permit_without_exact_estimate_fails_closed() {
    let db = Database::new();
    let mut c = db.open();
    let policy = Policy::default();
    c.execute("INSERT INTO jobs(id,job_class,locality,enforcement,state,created_at) VALUES ('legacy','BUILD','HOST_LOCAL','OBSERVE','NEW',1)", []).unwrap();
    c.execute("INSERT INTO permits(id,job_id,state,cpu_cores_est,memory_bytes_est,reserved_at) VALUES ('legacy-permit','legacy','ACTIVE',1.0,1024,1)", []).unwrap();
    let request = request(1, "BUILD", None);
    assert_eq!(
        enqueue(
            &mut c,
            &request,
            &OwnerEvidence::gather(&identity()),
            &policy,
            &context(1),
            None,
            10
        ),
        Err(JobError::StorageFailure)
    );
    assert!(get(&c, &id(1)).unwrap().is_none());
    assert_eq!(
        add(&mut c, 2, "UNKNOWN", 10, &policy),
        Reply::Bypassed {
            reason: BypassReason::Unknown
        }
    );
    c.execute(
        "UPDATE permits SET state='RELEASED',released_at=11 WHERE id='legacy-permit'",
        [],
    )
    .unwrap();
    assert!(matches!(
        add(&mut c, 1, "BUILD", 12, &policy),
        Reply::Granted { .. }
    ));
}

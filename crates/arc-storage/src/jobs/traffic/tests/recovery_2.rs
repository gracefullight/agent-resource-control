use super::*;
use crate::jobs::reconcile::simulated_pair;

#[test]
fn recovery_rolls_back_unknown_outcome_and_release_together() {
    let db = Database::new();
    let mut c = db.open();
    let policy = Policy::default();
    add(&mut c, 1, "BUILD", 10, &policy);
    let child = identity();
    attach(
        &mut c,
        &Request::Attach {
            job_id: id(1),
            observer: identity(),
            child: child.clone(),
            permit_generation: 1,
        },
        &ChildEvidence::gather(&child),
        11,
    )
    .unwrap();
    let before = get(&c, &id(1)).unwrap().unwrap();
    let evidence = simulated_pair(
        &before,
        IdentityStatus::Missing,
        IdentityStatus::Missing,
        12,
    );
    c.execute_batch(
        "CREATE TEMP TRIGGER reject_recovery BEFORE UPDATE OF state ON permits \
        WHEN NEW.state='RELEASED' BEGIN SELECT RAISE(ABORT,'injected failure'); END;",
    )
    .unwrap();
    assert_eq!(
        crate::jobs::apply_observation(&mut c, &evidence),
        Err(JobError::StorageFailure)
    );
    assert_eq!(get(&c, &id(1)).unwrap().unwrap(), before);
    assert!(matches!(
        status(&c, &id(1)).unwrap(),
        StatusResult::Running { .. }
    ));
    assert_eq!(permit_counts(&c), (1, 1));
    c.execute_batch("DROP TRIGGER reject_recovery;").unwrap();
    crate::jobs::apply_observation(&mut c, &evidence).unwrap();
    assert_eq!(permit_counts(&c), (1, 0));
    assert_eq!(get(&c, &id(1)).unwrap().unwrap().outcome, None);
}

#[test]
fn observed_completion_wins_against_previously_captured_recovery() {
    let db = Database::new();
    let mut c = db.open();
    let policy = Policy::default();
    add(&mut c, 1, "BUILD", 10, &policy);
    let child = identity();
    attach(
        &mut c,
        &Request::Attach {
            job_id: id(1),
            observer: identity(),
            child: child.clone(),
            permit_generation: 1,
        },
        &ChildEvidence::gather(&child),
        11,
    )
    .unwrap();
    let before = get(&c, &id(1)).unwrap().unwrap();
    let stale = simulated_pair(
        &before,
        IdentityStatus::Missing,
        IdentityStatus::Missing,
        13,
    );
    complete_job(&mut c, 1, 1, Outcome::Exited { code: 42 }, 12);
    let completed = get(&c, &id(1)).unwrap().unwrap();
    assert_eq!(
        crate::jobs::apply_observation(&mut c, &stale).unwrap(),
        crate::jobs::ReconcileResult::Unchanged(Box::new(completed.clone()))
    );
    assert_eq!(completed.outcome, Some(Outcome::Exited { code: 42 }));
    assert_eq!(permit_counts(&c), (1, 0));
}

#[test]
fn live_owner_can_report_outcome_after_child_disappears() {
    let db = Database::new();
    let mut c = db.open();
    let policy = Policy::default();
    add(&mut c, 1, "BUILD", 10, &policy);
    let child = identity();
    attach(
        &mut c,
        &Request::Attach {
            job_id: id(1),
            observer: identity(),
            child: child.clone(),
            permit_generation: 1,
        },
        &ChildEvidence::gather(&child),
        11,
    )
    .unwrap();
    let job = get(&c, &id(1)).unwrap().unwrap();
    let evidence = simulated_pair(
        &job,
        IdentityStatus::VerifiedMatch,
        IdentityStatus::Missing,
        12,
    );
    crate::jobs::apply_observation(&mut c, &evidence).unwrap();
    assert_eq!(permit_counts(&c), (1, 1));
    assert_eq!(get(&c, &id(1)).unwrap().unwrap().outcome, None);
    assert_eq!(
        complete_job(&mut c, 1, 1, Outcome::Exited { code: 7 }, 13),
        Reply::Completed {
            queue_wait_ms: 0,
            outcome: Outcome::Exited { code: 7 },
        }
    );
    assert_eq!(permit_counts(&c), (1, 0));
}

#[test]
fn late_authenticated_completion_refines_unknown_without_releasing_a_new_permit() {
    let db = Database::new();
    let mut c = db.open();
    let policy = Policy::default();
    add(&mut c, 1, "BUILD", 10, &policy);
    let child = identity();
    attach(
        &mut c,
        &Request::Attach {
            job_id: id(1),
            observer: identity(),
            child: child.clone(),
            permit_generation: 1,
        },
        &ChildEvidence::gather(&child),
        11,
    )
    .unwrap();
    add(&mut c, 2, "BUILD", 12, &policy);
    let job = get(&c, &id(1)).unwrap().unwrap();
    crate::jobs::apply_observation(
        &mut c,
        &simulated_pair(&job, IdentityStatus::Missing, IdentityStatus::Missing, 13),
    )
    .unwrap();
    assert_eq!(generation(&poll_job(&mut c, 2, 14, &policy)), 2);
    let outcome = Outcome::Exited { code: 42 };
    assert_eq!(
        complete_job(&mut c, 1, 1, outcome.clone(), 15),
        Reply::Completed {
            queue_wait_ms: 0,
            outcome: outcome.clone()
        }
    );
    assert_eq!(
        complete_job(&mut c, 1, 1, outcome.clone(), 16),
        Reply::Completed {
            queue_wait_ms: 0,
            outcome
        }
    );
    assert_eq!(permit_counts(&c), (2, 1));
    assert_eq!(
        status(&c, &id(2)).unwrap(),
        StatusResult::Reserved {
            sequence: 2,
            queued_at_ms: 12,
            granted_at_ms: 14,
            reservation: policy.build.reservation,
        }
    );
    drop(c);
    let mut c = db.open();
    assert_eq!(permit_counts(&c), (2, 1));
    assert_eq!(
        complete(
            &mut c,
            &Request::Complete {
                job_id: id(1),
                observer: identity(),
                outcome: Outcome::Exited { code: 43 },
                permit_generation: 1,
            },
            17
        ),
        Err(JobError::Conflict)
    );
}

#[test]
fn late_fallback_outcome_refines_recovered_unknown() {
    let db = Database::new();
    let mut c = db.open();
    let policy = Policy::default();
    add(&mut c, 1, "BUILD", 10, &policy);
    let child = identity();
    fallback(
        &mut c,
        &fallback_request(1, None, Some(child.clone()), None),
        Some(&ChildEvidence::gather(&child)),
        11,
    )
    .unwrap();
    let job = get(&c, &id(1)).unwrap().unwrap();
    crate::jobs::apply_observation(
        &mut c,
        &simulated_pair(&job, IdentityStatus::Missing, IdentityStatus::Mismatch, 12),
    )
    .unwrap();
    let outcome = Outcome::Exited { code: 9 };
    assert_eq!(
        fallback(
            &mut c,
            &fallback_request(1, None, Some(child.clone()), Some(outcome.clone())),
            Some(&ChildEvidence::gather(&child)),
            13
        ),
        Ok(Reply::Completed {
            queue_wait_ms: 0,
            outcome: outcome.clone()
        })
    );
    assert_eq!(
        fallback(
            &mut c,
            &fallback_request(1, None, Some(child.clone()), Some(outcome.clone())),
            Some(&ChildEvidence::gather(&child)),
            14
        ),
        Ok(Reply::Completed {
            queue_wait_ms: 0,
            outcome
        })
    );
    assert_eq!(permit_counts(&c), (1, 0));
}

#[test]
fn delayed_grant_acknowledgement_allows_confirmed_cancellation_after_restart() {
    let db = Database::new();
    let mut c = db.open();
    let policy = Policy::default();
    let grant = add(&mut c, 1, "BUILD", 10, &policy);
    add(&mut c, 2, "BUILD", 11, &policy);
    assert_eq!(
        cancel_job(&mut c, 1, None, 12),
        Reply::ReservationRetained {}
    );
    assert_eq!(
        cancel_job(&mut c, 1, None, 13),
        Reply::ReservationRetained {}
    );
    drop(c);
    let mut c = db.open();
    assert!(matches!(
        poll_job(&mut c, 2, 14, &policy),
        Reply::Queued { .. }
    ));
    assert_eq!(
        cancel_job(&mut c, 1, Some(generation(&grant)), 15),
        Reply::Cancelled {}
    );
    assert_eq!(
        cancel_job(&mut c, 1, Some(generation(&grant)), 16),
        Reply::Cancelled {}
    );
    assert_eq!(permit_counts(&c), (1, 0));
    assert_eq!(generation(&poll_job(&mut c, 2, 17, &policy)), 2);
}

#[test]
fn permanently_lost_grant_requires_explicit_live_owner_no_spawn_confirmation() {
    let db = Database::new();
    let mut c = db.open();
    let policy = Policy::default();
    // The grant commits but its response never reaches the owner.
    add(&mut c, 1, "BUILD", 10, &policy);
    add(&mut c, 2, "BUILD", 11, &policy);
    let cancel = Request::Cancel {
        job_id: id(1),
        observer: identity(),
        permit_generation: None,
    };
    assert_eq!(
        super::cancel(&mut c, &cancel, 12).unwrap(),
        Reply::ReservationRetained {}
    );
    assert_eq!(
        poll_job(&mut c, 1, 13, &policy),
        Reply::ReservationRetained {}
    );
    drop(c);
    let mut c = db.open();
    let owner = OwnerEvidence::gather(&identity());
    let unavailable = OwnerEvidence::simulated(&identity(), IdentityStatus::Mismatch);
    assert_eq!(
        confirm_cancel_unspawned(&mut c, &cancel, &unavailable, 14),
        Err(JobError::IdentityUnavailable)
    );
    let mut reused_pid = identity();
    reused_pid.start_token = if reused_pid.start_token == "1" {
        "2"
    } else {
        "1"
    }
    .into();
    let wrong_owner = Request::Cancel {
        job_id: id(1),
        observer: reused_pid.clone(),
        permit_generation: None,
    };
    assert_eq!(
        confirm_cancel_unspawned(&mut c, &wrong_owner, &owner, 15),
        Err(JobError::InvalidRequest)
    );
    assert_eq!(
        confirm_cancel_unspawned(
            &mut c,
            &wrong_owner,
            &OwnerEvidence::simulated(&reused_pid, IdentityStatus::VerifiedMatch),
            15
        ),
        Err(JobError::Conflict)
    );
    let wrong_generation = Request::Cancel {
        job_id: id(1),
        observer: identity(),
        permit_generation: Some(9),
    };
    assert_eq!(
        confirm_cancel_unspawned(&mut c, &wrong_generation, &owner, 16),
        Err(JobError::Conflict)
    );
    assert_eq!(permit_counts(&c), (1, 1));
    assert!(matches!(
        poll_job(&mut c, 2, 17, &policy),
        Reply::Queued { .. }
    ));
    assert_eq!(
        confirm_cancel_unspawned(&mut c, &cancel, &owner, 18).unwrap(),
        Reply::Cancelled {}
    );
    assert_eq!(
        confirm_cancel_unspawned(&mut c, &cancel, &owner, 19).unwrap(),
        Reply::Cancelled {}
    );
    assert_eq!(add(&mut c, 1, "BUILD", 20, &policy), Reply::Cancelled {});
    let late_attach = Request::Attach {
        job_id: id(1),
        observer: identity(),
        child: identity(),
        permit_generation: 1,
    };
    assert_eq!(
        attach(
            &mut c,
            &late_attach,
            &ChildEvidence::gather(&identity()),
            21
        ),
        Err(JobError::InvalidTransition)
    );
    assert_eq!(permit_counts(&c), (1, 0));
    assert_eq!(generation(&poll_job(&mut c, 2, 22, &policy)), 2);
}

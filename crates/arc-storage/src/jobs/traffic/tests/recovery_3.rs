use super::*;

#[test]
fn explicit_no_spawn_confirmation_cannot_release_an_attached_child() {
    let db = Database::new();
    let mut c = db.open();
    let policy = Policy::default();
    add(&mut c, 1, "BUILD", 10, &policy);
    let child = identity();
    let fallback = fallback_request(1, None, Some(child.clone()), None);
    super::fallback(&mut c, &fallback, Some(&ChildEvidence::gather(&child)), 11).unwrap();
    let cancel = Request::Cancel {
        job_id: id(1),
        observer: identity(),
        permit_generation: None,
    };
    assert_eq!(
        confirm_cancel_unspawned(&mut c, &cancel, &OwnerEvidence::gather(&identity()), 12).unwrap(),
        Reply::ReservationRetained {}
    );
    assert_eq!(permit_counts(&c), (1, 1));
    drop(c);
    let mut c = db.open();
    assert_eq!(
        confirm_cancel_unspawned(&mut c, &cancel, &OwnerEvidence::gather(&identity()), 13).unwrap(),
        Reply::ReservationRetained {}
    );
    assert_eq!(permit_counts(&c), (1, 1));
    complete_job(&mut c, 1, 1, Outcome::Exited { code: 0 }, 14);
    assert_eq!(permit_counts(&c), (1, 0));
}

#[test]
fn cancellation_spawn_failure_and_lost_acknowledgements_reconcile_once() {
    let db = Database::new();
    let mut c = db.open();
    let policy = Policy::default();
    let first = add(&mut c, 1, "BUILD", 10, &policy);
    let gen = generation(&first);
    add(&mut c, 2, "BUILD", 11, &policy);
    assert_eq!(cancel_job(&mut c, 2, None, 12), Reply::Cancelled {});
    assert_eq!(cancel_job(&mut c, 2, None, 13), Reply::Cancelled {});
    assert_eq!(poll_job(&mut c, 2, 14, &policy), Reply::Cancelled {});
    // Unknown generation is not evidence that the grant did not commit.
    assert_eq!(
        cancel_job(&mut c, 1, None, 15),
        Reply::ReservationRetained {}
    );
    assert!(matches!(
        status(&c, &id(1)).unwrap(),
        StatusResult::Reserved { .. }
    ));
    drop(c);
    let mut c = db.open();
    assert_eq!(
        fallback(&mut c, &fallback_request(1, None, None, None), None, 16).unwrap(),
        Reply::ReservationRetained {}
    );
    assert_eq!(permit_counts(&c), (1, 1));
    let failure = Outcome::SpawnFailed {
        reason: SpawnReason::NotFound,
        os_code: Some(2),
    };
    let recovered = fallback(
        &mut c,
        &fallback_request(1, None, None, Some(failure.clone())),
        None,
        17,
    )
    .unwrap();
    assert_eq!(
        recovered,
        Reply::Completed {
            queue_wait_ms: 0,
            outcome: failure.clone()
        }
    );
    assert_eq!(
        fallback(
            &mut c,
            &fallback_request(1, None, None, Some(failure)),
            None,
            18
        )
        .unwrap(),
        recovered
    );
    assert_eq!(permit_counts(&c), (1, 0));
    assert_eq!(
        complete_job(
            &mut c,
            1,
            gen,
            Outcome::SpawnFailed {
                reason: SpawnReason::NotFound,
                os_code: Some(2)
            },
            19
        ),
        recovered
    );
    assert!(matches!(
        add(&mut c, 3, "BUILD", 20, &policy),
        Reply::Granted { .. }
    ));
    assert_eq!(cancel_job(&mut c, 3, Some(3), 21), Reply::Cancelled {});
    assert_eq!(cancel_job(&mut c, 3, Some(3), 22), Reply::Cancelled {});
    assert_eq!(permit_counts(&c), (2, 0));
}

#[test]
fn fallback_and_stale_owner_tombstones_never_revive_or_release_grants() {
    let db = Database::new();
    let mut c = db.open();
    let policy = Policy::default();
    add(&mut c, 1, "BUILD", 10, &policy);
    add(&mut c, 2, "BUILD", 11, &policy);
    let old = get(&c, &id(2)).unwrap().unwrap();
    let indeterminate = QueueObservation::simulated(&old, IdentityStatus::Indeterminate, 12);
    assert_eq!(
        reconcile_queued(&mut c, &indeterminate),
        Ok(ReconcileResult::Unchanged)
    );
    let missing = QueueObservation::simulated(&old, IdentityStatus::Mismatch, 13);
    assert_eq!(
        reconcile_queued(&mut c, &missing),
        Ok(ReconcileResult::Abandoned)
    );
    assert_eq!(add(&mut c, 2, "BUILD", 14, &policy), Reply::Abandoned {});
    assert_eq!(
        reconcile_queued(&mut c, &missing),
        Ok(ReconcileResult::Stale)
    );
    assert_eq!(permit_counts(&c), (1, 1));
    assert_eq!(
        fallback(&mut c, &fallback_request(4, None, None, None), None, 15).unwrap(),
        Reply::Abandoned {}
    );
    assert_eq!(add(&mut c, 4, "BUILD", 16, &policy), Reply::Abandoned {});
    assert_eq!(status(&c, &id(4)).unwrap(), StatusResult::Abandoned {});
    drop(c);
    let mut c = db.open();
    assert_eq!(permit_counts(&c), (1, 1));
    add(&mut c, 3, "BUILD", 17, &policy);
    let stale = get(&c, &id(3)).unwrap().unwrap();
    let stale_observation = QueueObservation::simulated(&stale, IdentityStatus::Missing, 19);
    complete_job(&mut c, 1, 1, Outcome::Exited { code: 0 }, 18);
    assert!(matches!(
        poll_job(&mut c, 3, 18, &policy),
        Reply::Granted { .. }
    ));
    assert_eq!(
        reconcile_queued(&mut c, &stale_observation),
        Ok(ReconcileResult::Stale)
    );
    assert_eq!(permit_counts(&c), (2, 1));
}

#[test]
fn attached_identity_cannot_be_replaced_and_legacy_transitions_are_blocked() {
    let db = Database::new();
    let mut c = db.open();
    let policy = Policy::default();
    add(&mut c, 1, "BUILD", 10, &policy);
    let legacy = StartRequest::Claim {
        job_id: id(1),
        observer: identity(),
    };
    let evidence = crate::jobs::StartEvidence::gather(&legacy);
    assert_eq!(
        crate::jobs::start(&mut c, &legacy, &evidence, 11),
        Err(JobError::InvalidTransition)
    );
    let req = Request::Attach {
        job_id: id(1),
        observer: identity(),
        child: identity(),
        permit_generation: 1,
    };
    let child_evidence = ChildEvidence::gather(&identity());
    assert_eq!(
        attach(&mut c, &req, &child_evidence, 12),
        Ok(Reply::Running {})
    );
    assert_eq!(
        attach(&mut c, &req, &child_evidence, 13),
        Ok(Reply::Running {})
    );
    let mut stale = identity();
    stale.start_token = if stale.start_token == "1" { "2" } else { "1" }.into();
    let wrong = Request::Attach {
        job_id: id(1),
        observer: identity(),
        child: stale.clone(),
        permit_generation: 1,
    };
    assert_eq!(
        attach(
            &mut c,
            &wrong,
            &ChildEvidence::simulated(&stale, IdentityStatus::Mismatch),
            14
        ),
        Err(JobError::Conflict)
    );
    assert_eq!(
        cancel_job(&mut c, 1, Some(1), 15),
        Reply::ReservationRetained {}
    );
    assert_eq!(permit_counts(&c), (1, 1));
    complete_job(&mut c, 1, 1, Outcome::Exited { code: 0 }, 16);
    assert_eq!(permit_counts(&c), (1, 0));
}

#[test]
fn lost_grant_with_identified_child_retains_capacity_through_restart() {
    let db = Database::new();
    let mut c = db.open();
    let policy = Policy::default();
    add(&mut c, 1, "BUILD", 10, &policy);
    add(&mut c, 2, "BUILD", 11, &policy);
    let child = identity();
    let fallback = fallback_request(1, None, Some(child.clone()), None);
    let evidence = ChildEvidence::gather(&child);
    assert_eq!(
        super::fallback(&mut c, &fallback, Some(&evidence), 12).unwrap(),
        Reply::ReservationRetained {}
    );
    assert!(matches!(
        status(&c, &id(1)).unwrap(),
        StatusResult::Reserved { .. }
    ));
    assert_eq!(
        super::fallback(&mut c, &fallback, Some(&evidence), 13).unwrap(),
        Reply::ReservationRetained {}
    );
    drop(c);
    let mut c = db.open();
    assert!(matches!(
        poll_job(&mut c, 2, 14, &policy),
        Reply::Queued { .. }
    ));
    assert_eq!(permit_counts(&c), (1, 1));
    let result = complete_job(&mut c, 1, 1, Outcome::Exited { code: 0 }, 15);
    assert_eq!(
        result,
        Reply::Completed {
            queue_wait_ms: 0,
            outcome: Outcome::Exited { code: 0 }
        }
    );
    assert_eq!(
        super::fallback(
            &mut c,
            &fallback_request(1, None, None, Some(Outcome::Exited { code: 1 })),
            None,
            16
        ),
        Err(JobError::Conflict)
    );
    assert_eq!(permit_counts(&c), (1, 0));
    assert_eq!(generation(&poll_job(&mut c, 2, 17, &policy)), 2);
}

#[test]
fn unverified_child_or_conflicting_retry_never_changes_admission() {
    let db = Database::new();
    let mut c = db.open();
    let policy = Policy::default();
    let owner = OwnerEvidence::gather(&identity());
    add(&mut c, 1, "BUILD", 10, &policy);
    let unavailable = OwnerEvidence::simulated(&identity(), IdentityStatus::Indeterminate);
    assert_eq!(
        enqueue(
            &mut c,
            &request(3, "UNKNOWN", None),
            &unavailable,
            &policy,
            &context(3),
            None,
            11,
        ),
        Ok(Reply::Bypassed {
            reason: BypassReason::Unknown
        })
    );
    let child = identity();
    let attach_request = Request::Attach {
        job_id: id(1),
        observer: identity(),
        child: child.clone(),
        permit_generation: 1,
    };
    let bad = ChildEvidence::simulated(&child, IdentityStatus::Mismatch);
    assert_eq!(
        attach(&mut c, &attach_request, &bad, 11),
        Err(JobError::IdentityUnavailable)
    );
    assert!(get(&c, &id(1)).unwrap().unwrap().child.is_none());
    assert_eq!(permit_counts(&c), (1, 1));
    assert_eq!(
        enqueue(
            &mut c,
            &request(1, "BUILD", Some(context(44))),
            &owner,
            &policy,
            &context(4),
            None,
            12
        ),
        Err(JobError::Conflict)
    );
    assert_eq!(permit_counts(&c), (1, 1));
}

#[test]
fn cancellation_before_enqueue_commits_a_tombstone_for_delayed_retries() {
    let db = Database::new();
    let mut c = db.open();
    let policy = Policy::default();
    assert_eq!(cancel_job(&mut c, 9, None, 10), Reply::Cancelled {});
    assert_eq!(cancel_job(&mut c, 9, None, 11), Reply::Cancelled {});
    assert_eq!(add(&mut c, 9, "BUILD", 12, &policy), Reply::Cancelled {});
    assert_eq!(poll_job(&mut c, 9, 13, &policy), Reply::Cancelled {});
    assert_eq!(
        super::fallback(&mut c, &fallback_request(9, None, None, None), None, 14).unwrap(),
        Reply::Cancelled {}
    );
    assert_eq!(status(&c, &id(9)).unwrap(), StatusResult::Cancelled {});
    assert!(get(&c, &id(9)).unwrap().is_none());
    assert_eq!(permit_counts(&c), (0, 0));
    let mut other = identity();
    other.start_token = if other.start_token == "1" { "2" } else { "1" }.into();
    assert_eq!(
        cancel(
            &mut c,
            &Request::Cancel {
                job_id: id(9),
                observer: other,
                permit_generation: None
            },
            15
        ),
        Err(JobError::Conflict)
    );
    drop(c);
    let mut c = db.open();
    assert_eq!(add(&mut c, 9, "BUILD", 16, &policy), Reply::Cancelled {});
    assert_eq!(permit_counts(&c), (0, 0));
}

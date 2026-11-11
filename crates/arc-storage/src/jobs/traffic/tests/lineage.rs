use super::*;

#[test]
fn queued_child_can_retry_with_verified_lineage_without_a_second_permit() {
    let db = Database::new();
    let mut c = db.open();
    let policy = Policy::default();
    let owner = OwnerEvidence::gather(&identity());
    add(&mut c, 1, "BUILD", 10, &policy);
    let nested = request(2, "BUILD", Some(context(1)));
    assert!(matches!(
        enqueue(&mut c, &nested, &owner, &policy, &context(2), None, 11).unwrap(),
        Reply::Queued { sequence: 2, .. }
    ));
    add(&mut c, 3, "BUILD", 12, &policy);
    let snapshot = lineage_snapshot(&c, &context(1)).unwrap().unwrap();
    let mut forged_anchor = snapshot.root.observer.clone();
    forged_anchor.start_token.push('9');
    let forged = LineageProof::verified(&snapshot, &forged_anchor);
    assert_eq!(
        enqueue(
            &mut c,
            &nested,
            &owner,
            &policy,
            &context(2),
            Some(&forged),
            13
        ),
        Err(JobError::IdentityUnavailable)
    );
    assert_eq!(permit_counts(&c), (1, 1));

    let stale_proof = LineageProof::verified(&snapshot, &snapshot.root.observer);
    let attach_request = Request::Attach {
        job_id: id(1),
        observer: identity(),
        child: identity(),
        permit_generation: 1,
    };
    attach(
        &mut c,
        &attach_request,
        &ChildEvidence::gather(&identity()),
        14,
    )
    .unwrap();
    assert_eq!(
        enqueue(
            &mut c,
            &nested,
            &owner,
            &policy,
            &context(2),
            Some(&stale_proof),
            15
        ),
        Err(JobError::IdentityUnavailable)
    );
    let current = lineage_snapshot(&c, &context(1)).unwrap().unwrap();
    let proof = LineageProof::verified(&current, &current.root.observer);
    let missing = OwnerEvidence::simulated(&identity(), IdentityStatus::Missing);
    assert_eq!(
        enqueue(
            &mut c,
            &nested,
            &missing,
            &policy,
            &context(2),
            Some(&proof),
            16
        ),
        Err(JobError::IdentityUnavailable)
    );
    let reused = Reply::Reused {
        parent_job_id: id(1),
        permit_generation: 1,
    };
    assert_eq!(
        enqueue(
            &mut c,
            &nested,
            &owner,
            &policy,
            &context(2),
            Some(&proof),
            17
        ),
        Ok(reused.clone())
    );
    assert_eq!(permit_counts(&c), (1, 1));
    drop(c);
    let mut c = db.open();
    assert_eq!(poll_job(&mut c, 2, 18, &policy), reused);
    complete_job(&mut c, 1, 1, Outcome::Exited { code: 0 }, 19);
    assert_eq!(permit_counts(&c), (1, 1));
    assert_eq!(
        enqueue(
            &mut c,
            &nested,
            &owner,
            &policy,
            &context(2),
            Some(&stale_proof),
            19
        ),
        Ok(reused)
    );
    assert_eq!(
        complete_job(&mut c, 2, 1, Outcome::Exited { code: 0 }, 20),
        Reply::Completed {
            queue_wait_ms: 6,
            outcome: Outcome::Exited { code: 0 }
        }
    );
    assert_eq!(permit_counts(&c), (1, 0));
    assert_eq!(generation(&poll_job(&mut c, 3, 21, &policy)), 3);
}

#[test]
fn active_descendant_can_reuse_permit_after_root_finishes() {
    let db = Database::new();
    let mut c = db.open();
    let policy = Policy::default();
    let owner = OwnerEvidence::gather(&identity());
    add(&mut c, 1, "BUILD", 10, &policy);
    let root = lineage_snapshot(&c, &context(1)).unwrap().unwrap();
    let proof = LineageProof::verified(&root, &root.root.observer);
    assert!(matches!(
        enqueue(
            &mut c,
            &request(2, "BUILD", Some(context(1))),
            &owner,
            &policy,
            &context(2),
            Some(&proof),
            11
        )
        .unwrap(),
        Reply::Reused { .. }
    ));
    complete_job(&mut c, 1, 1, Outcome::Exited { code: 0 }, 12);
    drop(c);
    let mut c = db.open();
    let descendant = lineage_snapshot(&c, &context(1))
        .unwrap()
        .expect("active descendant must remain an ancestry anchor");
    let proof = LineageProof::verified(&descendant, &descendant.root.observer);
    assert_eq!(
        enqueue(
            &mut c,
            &request(3, "BUILD", Some(context(1))),
            &owner,
            &policy,
            &context(3),
            Some(&proof),
            13
        )
        .unwrap(),
        Reply::Reused {
            parent_job_id: id(1),
            permit_generation: 1
        }
    );
    assert_eq!(permit_counts(&c), (1, 1));
    complete_job(&mut c, 2, 1, Outcome::Exited { code: 0 }, 14);
    assert_eq!(permit_counts(&c), (1, 1));
    complete_job(&mut c, 3, 1, Outcome::Exited { code: 0 }, 15);
    assert_eq!(permit_counts(&c), (1, 0));
}

#[test]
fn confirmed_root_cancellation_keeps_permit_for_active_descendants() {
    let db = Database::new();
    let mut c = db.open();
    let policy = Policy::default();
    let owner = OwnerEvidence::gather(&identity());
    add(&mut c, 1, "BUILD", 10, &policy);
    let root = lineage_snapshot(&c, &context(1)).unwrap().unwrap();
    let root_proof = LineageProof::verified(&root, &root.root.observer);
    enqueue(
        &mut c,
        &request(2, "BUILD", Some(context(1))),
        &owner,
        &policy,
        &context(2),
        Some(&root_proof),
        11,
    )
    .unwrap();
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
        confirm_cancel_unspawned(&mut c, &cancel, &owner, 13).unwrap(),
        Reply::Cancelled {}
    );
    assert_eq!(permit_counts(&c), (1, 1));
    assert_eq!(
        enqueue(
            &mut c,
            &request(3, "BUILD", Some(context(1))),
            &owner,
            &policy,
            &context(3),
            Some(&root_proof),
            14
        ),
        Err(JobError::IdentityUnavailable)
    );
    let descendant = lineage_snapshot(&c, &context(1)).unwrap().unwrap();
    let proof = LineageProof::verified(&descendant, &descendant.root.observer);
    assert!(matches!(
        enqueue(&mut c, &request(3, "BUILD", Some(context(1))), &owner, &policy, &context(3), Some(&proof), 15).unwrap(),
        Reply::Reused { parent_job_id, permit_generation: 1 } if parent_job_id == id(1)
    ));
    complete_job(&mut c, 2, 1, Outcome::Exited { code: 0 }, 16);
    assert_eq!(permit_counts(&c), (1, 1));
    complete_job(&mut c, 3, 1, Outcome::Exited { code: 0 }, 17);
    assert_eq!(permit_counts(&c), (1, 0));
}

#[test]
fn every_active_sibling_can_be_an_anchor_but_stale_and_forged_proofs_fail() {
    let db = Database::new();
    let mut c = db.open();
    let policy = Policy::default();
    let owner = OwnerEvidence::gather(&identity());
    add(&mut c, 1, "BUILD", 10, &policy);
    let root = lineage_snapshot(&c, &context(1)).unwrap().unwrap();
    let root_proof = LineageProof::verified(&root, &root.root.observer);
    for n in [2, 3] {
        assert!(matches!(
            enqueue(&mut c, &request(n, "BUILD", Some(context(1))), &owner, &policy, &context(n), Some(&root_proof), 10 + u64::from(n)).unwrap(),
            Reply::Reused { parent_job_id, permit_generation: 1 } if parent_job_id == id(1)
        ));
    }
    // Distinct identities model siblings; these observations are deliberately
    // simulated, not a substitute for the daemon's native ancestry check.
    for n in [2, 3] {
        let mut child = identity();
        child.start_token = format!("{}{}", child.start_token, n);
        let attach_request = Request::Attach {
            job_id: id(n),
            observer: identity(),
            child: child.clone(),
            permit_generation: 1,
        };
        attach(
            &mut c,
            &attach_request,
            &ChildEvidence::simulated(&child, IdentityStatus::VerifiedMatch),
            14 + u64::from(n),
        )
        .unwrap();
    }
    complete_job(&mut c, 1, 1, Outcome::Exited { code: 0 }, 18);
    let snapshots = lineage_snapshots(&c, &context(1)).unwrap();
    assert_eq!(
        snapshots
            .iter()
            .map(|s| s.root.job_id.clone())
            .collect::<Vec<_>>(),
        [id(2), id(3)]
    );
    assert!(snapshots
        .iter()
        .all(|s| s.permit_job_id == id(1) && s.generation == 1));
    let second = &snapshots[1];
    let anchor = second.root.child.as_ref().unwrap();
    let proof = LineageProof::verified(second, anchor);
    let mut forged_anchor = anchor.clone();
    forged_anchor.start_token.push('9');
    let forged_proof = LineageProof::verified(second, &forged_anchor);
    assert_eq!(
        enqueue(
            &mut c,
            &request(5, "BUILD", Some(context(1))),
            &owner,
            &policy,
            &context(5),
            Some(&forged_proof),
            19
        ),
        Err(JobError::IdentityUnavailable)
    );
    assert!(matches!(
        enqueue(
            &mut c,
            &request(6, "BUILD", Some(context(1))),
            &owner,
            &policy,
            &context(6),
            None,
            20
        )
        .unwrap(),
        Reply::Queued { .. }
    ));
    assert_eq!(
        enqueue(
            &mut c,
            &request(4, "BUILD", Some(context(1))),
            &owner,
            &policy,
            &context(4),
            Some(&proof),
            21
        )
        .unwrap(),
        Reply::Reused {
            parent_job_id: id(1),
            permit_generation: 1
        }
    );
    complete_job(&mut c, 3, 1, Outcome::Exited { code: 0 }, 22);
    assert_eq!(
        enqueue(
            &mut c,
            &request(5, "BUILD", Some(context(1))),
            &owner,
            &policy,
            &context(5),
            Some(&proof),
            23
        ),
        Err(JobError::IdentityUnavailable)
    );
    assert!(matches!(
        poll_job(&mut c, 6, 24, &policy),
        Reply::Queued { .. }
    ));
    complete_job(&mut c, 2, 1, Outcome::Exited { code: 0 }, 25);
    assert_eq!(permit_counts(&c), (1, 1));
    complete_job(&mut c, 4, 1, Outcome::Exited { code: 0 }, 26);
    assert_eq!(permit_counts(&c), (1, 0));
    assert_eq!(generation(&poll_job(&mut c, 6, 27, &policy)), 4);
}

#[test]
fn verified_lineage_reuses_one_permit_until_last_descendant_completes() {
    let db = Database::new();
    let mut c = db.open();
    let policy = Policy::default();
    add(&mut c, 1, "BUILD", 10, &policy);
    let parent_context = context(1);
    let snapshot = lineage_snapshot(&c, &parent_context).unwrap().unwrap();
    let proof = LineageProof::verified(&snapshot, &snapshot.root.observer);
    let nested = request(2, "BUILD", Some(parent_context.clone()));
    let owner = OwnerEvidence::gather(&identity());
    let reused = enqueue(
        &mut c,
        &nested,
        &owner,
        &policy,
        &context(2),
        Some(&proof),
        11,
    )
    .unwrap();
    assert_eq!(
        reused,
        Reply::Reused {
            parent_job_id: id(1),
            permit_generation: 1
        }
    );
    assert_eq!(
        enqueue(
            &mut c,
            &nested,
            &owner,
            &policy,
            &context(99),
            Some(&proof),
            12
        )
        .unwrap(),
        reused
    );
    assert_eq!(permit_counts(&c), (1, 1));
    // Possession of a copied or forged context without native ancestry proof
    // falls back to ordinary FIFO admission.
    let forged = request(3, "BUILD", Some(parent_context.clone()));
    assert!(matches!(
        enqueue(&mut c, &forged, &owner, &policy, &context(3), None, 12).unwrap(),
        Reply::Queued { .. }
    ));
    complete_job(&mut c, 1, 1, Outcome::Exited { code: 0 }, 13);
    assert_eq!(permit_counts(&c), (1, 1));
    assert!(matches!(
        poll_job(&mut c, 3, 14, &policy),
        Reply::Queued { .. }
    ));
    assert_eq!(
        complete_job(&mut c, 2, 1, Outcome::Exited { code: 0 }, 15),
        Reply::Completed {
            queue_wait_ms: 0,
            outcome: Outcome::Exited { code: 0 }
        }
    );
    assert_eq!(permit_counts(&c), (1, 0));
    assert_eq!(generation(&poll_job(&mut c, 3, 16, &policy)), 3);
    assert!(lineage_snapshot(&c, &parent_context).unwrap().is_none());
}

mod ancestry;

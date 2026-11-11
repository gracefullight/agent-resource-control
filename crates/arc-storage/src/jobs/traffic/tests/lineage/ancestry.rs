use super::*;

#[test]
fn stale_or_mismatched_lineage_and_native_identity_fail_closed() {
    let db = Database::new();
    let mut c = db.open();
    let policy = Policy::default();
    let owner = OwnerEvidence::gather(&identity());
    let bad_owner = OwnerEvidence::simulated(&identity(), IdentityStatus::Mismatch);
    assert_eq!(
        enqueue(
            &mut c,
            &request(1, "BUILD", None),
            &bad_owner,
            &policy,
            &context(1),
            None,
            10
        ),
        Err(JobError::IdentityUnavailable)
    );
    add(&mut c, 1, "BUILD", 10, &policy);
    let snap = lineage_snapshot(&c, &context(1)).unwrap().unwrap();
    let proof = LineageProof::verified(&snap, &snap.root.observer);
    // The native proof, not the caller's independently generated witness key,
    // authorizes reuse. The original key must remain bound to the request.
    let different_claim = request(3, "BUILD", Some(context(99)));
    let reused = enqueue(
        &mut c,
        &different_claim,
        &owner,
        &policy,
        &context(3),
        Some(&proof),
        11,
    )
    .unwrap();
    assert!(matches!(reused, Reply::Reused { .. }));
    assert_eq!(
        enqueue(
            &mut c,
            &different_claim,
            &owner,
            &policy,
            &context(3),
            Some(&proof),
            11
        ),
        Ok(reused)
    );
    let stored: String = c
        .query_row(
            "SELECT inherited_context FROM traffic_entries WHERE job_id=?1",
            [id(3).as_str()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(stored, context(99).as_str());
    assert_eq!(
        enqueue(
            &mut c,
            &request(3, "BUILD", Some(context(1))),
            &owner,
            &policy,
            &context(3),
            Some(&proof),
            11
        ),
        Err(JobError::Conflict)
    );
    let attached = Request::Attach {
        job_id: id(1),
        observer: identity(),
        child: identity(),
        permit_generation: 1,
    };
    attach(&mut c, &attached, &ChildEvidence::gather(&identity()), 12).unwrap();
    let stale = request(2, "BUILD", Some(context(1)));
    assert_eq!(
        enqueue(
            &mut c,
            &stale,
            &owner,
            &policy,
            &context(2),
            Some(&proof),
            13
        ),
        Err(JobError::IdentityUnavailable)
    );
    assert!(get(&c, &id(2)).unwrap().is_none());
    assert_eq!(permit_counts(&c), (1, 1));
}

#[test]
fn ancestry_discovery_rechecks_active_anchor_and_supports_legacy_poll() {
    let db = Database::new();
    let mut c = db.open();
    let policy = Policy::default();
    let owner = OwnerEvidence::gather(&identity());
    add(&mut c, 1, "BUILD", 10, &policy);
    let mut stale = identity();
    stale.start_token = (stale.start_token.parse::<u64>().unwrap() + 1).to_string();
    assert!(lineage_anchors(&c, &[stale]).unwrap().is_empty());
    let anchors = lineage_anchors(&c, &[identity()]).unwrap();
    assert_eq!(anchors.len(), 1);
    let (snapshot, anchor) = &anchors[0];
    let denied = LineageProof::verified_with_recheck(snapshot, anchor, |_, _| false);
    let nested = request(2, "BUILD", Some(context(99)));
    assert!(matches!(
        enqueue(&mut c, &nested, &owner, &policy, &context(2), None, 11).unwrap(),
        Reply::Queued { .. }
    ));
    let poll = Request::Poll {
        job_id: id(2),
        observer: identity(),
    };
    assert_eq!(
        poll_with_proof(
            &mut c,
            &poll,
            &owner,
            &policy,
            &context(2),
            Some(&denied),
            12
        ),
        Err(JobError::IdentityUnavailable)
    );
    let proof = LineageProof::verified(snapshot, anchor);
    assert!(
        matches!(poll_with_proof(&mut c, &poll, &owner, &policy, &context(2), Some(&proof), 13).unwrap(), Reply::Reused { parent_job_id, .. } if parent_job_id == id(1))
    );
    assert_eq!(permit_counts(&c), (1, 1));
    complete_job(&mut c, 2, 1, Outcome::Exited { code: 0 }, 14);
    complete_job(&mut c, 1, 1, Outcome::Exited { code: 0 }, 15);
    assert!(lineage_anchors(&c, &[identity()]).unwrap().is_empty());
    assert_eq!(permit_counts(&c), (1, 0));
}

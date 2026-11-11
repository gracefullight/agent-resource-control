use super::*;

#[test]
fn witness_recovery_requires_matching_key_and_definitive_owner_evidence() {
    let db = Database::new();
    let mut connection = db.open();
    let policy = Policy::default();
    let owner = identity();
    let request = request(1, "BUILD", Some(context(90)));
    let grant = enqueue(
        &mut connection,
        &request,
        &OwnerEvidence::gather(&owner),
        &policy,
        &context(1),
        None,
        10,
    )
    .unwrap();
    assert!(matches!(grant, Reply::Granted { .. }));
    let snapshot = get(&connection, &id(1)).unwrap().unwrap();
    let child = identity();
    let evidence = |key: LineageContext, owner_status, child_status| {
        WitnessEvidence::simulated(&snapshot, &key, &child, owner_status, child_status, 11)
    };
    for proof in [
        evidence(
            context(91),
            IdentityStatus::Missing,
            IdentityStatus::Missing,
        ),
        evidence(
            context(90),
            IdentityStatus::VerifiedMatch,
            IdentityStatus::Missing,
        ),
        evidence(
            context(90),
            IdentityStatus::Missing,
            IdentityStatus::Indeterminate,
        ),
    ] {
        assert!(!recover_witness(&mut connection, &proof).unwrap());
        assert_eq!(permit_counts(&connection), (1, 1));
        assert!(get(&connection, &id(1)).unwrap().unwrap().child.is_none());
    }
    let mut wrong_boot = child.clone();
    wrong_boot.boot_id = "00000000-0000-0000-0000-000000000000".into();
    let proof = WitnessEvidence::simulated(
        &snapshot,
        &context(90),
        &wrong_boot,
        IdentityStatus::Missing,
        IdentityStatus::Missing,
        11,
    );
    assert!(!recover_witness(&mut connection, &proof).unwrap());
    let live = evidence(
        context(90),
        IdentityStatus::Missing,
        IdentityStatus::VerifiedMatch,
    );
    assert!(recover_witness(&mut connection, &live).unwrap());
    assert_eq!(permit_counts(&connection), (1, 1));
    assert_eq!(
        get(&connection, &id(1)).unwrap().unwrap().child,
        Some(child)
    );
    assert!(
        !recover_witness(&mut connection, &live).unwrap(),
        "stale evidence must not apply twice"
    );
}

#[test]
fn reused_job_recovers_only_with_its_own_fresh_witness_key() {
    let db = Database::new();
    let mut connection = db.open();
    let policy = Policy::default();
    add(&mut connection, 1, "BUILD", 10, &policy);
    let root = lineage_snapshot(&connection, &context(1)).unwrap().unwrap();
    let proof = LineageProof::verified(&root, &root.root.observer);
    let nested = request(2, "BUILD", Some(context(99)));
    let owner = OwnerEvidence::gather(&identity());
    let reply = enqueue(
        &mut connection,
        &nested,
        &owner,
        &policy,
        &context(2),
        Some(&proof),
        11,
    )
    .unwrap();
    assert!(matches!(&reply, Reply::Reused { parent_job_id, .. } if *parent_job_id == id(1)));
    assert_eq!(
        enqueue(
            &mut connection,
            &nested,
            &owner,
            &policy,
            &context(2),
            Some(&proof),
            11
        )
        .unwrap(),
        reply
    );
    let snapshot = get(&connection, &id(2)).unwrap().unwrap();
    let old_root_key = WitnessEvidence::simulated(
        &snapshot,
        &context(1),
        &identity(),
        IdentityStatus::Missing,
        IdentityStatus::VerifiedMatch,
        12,
    );
    assert!(!recover_witness(&mut connection, &old_root_key).unwrap());
    assert_eq!(permit_counts(&connection), (1, 1));
    let own_key = WitnessEvidence::simulated(
        &snapshot,
        &context(99),
        &identity(),
        IdentityStatus::Missing,
        IdentityStatus::VerifiedMatch,
        12,
    );
    assert!(recover_witness(&mut connection, &own_key).unwrap());
    assert!(!recover_witness(&mut connection, &own_key).unwrap());
    assert_eq!(permit_counts(&connection), (1, 1));
    complete_job(&mut connection, 1, 1, Outcome::Exited { code: 0 }, 13);
    assert_eq!(permit_counts(&connection), (1, 1));
    complete_job(&mut connection, 2, 1, Outcome::Exited { code: 0 }, 14);
    assert_eq!(permit_counts(&connection), (1, 0));
}

#[test]
fn witness_for_an_exited_child_releases_once_and_records_unknown() {
    let db = Database::new();
    let mut connection = db.open();
    let request = request(1, "BUILD", Some(context(90)));
    enqueue(
        &mut connection,
        &request,
        &OwnerEvidence::gather(&identity()),
        &Policy::default(),
        &context(1),
        None,
        10,
    )
    .unwrap();
    let snapshot = get(&connection, &id(1)).unwrap().unwrap();
    let proof = WitnessEvidence::simulated(
        &snapshot,
        &context(90),
        &identity(),
        IdentityStatus::Mismatch,
        IdentityStatus::Missing,
        11,
    );
    assert!(recover_witness(&mut connection, &proof).unwrap());
    assert!(!recover_witness(&mut connection, &proof).unwrap());
    assert_eq!(permit_counts(&connection), (1, 0));
    assert_eq!(
        status(&connection, &id(1)).unwrap(),
        StatusResult::Abandoned {}
    );
    let recovered = get(&connection, &id(1)).unwrap().unwrap();
    assert_eq!(recovered.phase, arc_ipc::jobs::Phase::OutcomeUnknown);
    assert!(recovered.outcome.is_none());
}

#[test]
fn unspawned_witness_releases_only_for_the_original_dead_owner_and_key() {
    let db = Database::new();
    let mut connection = db.open();
    let first_request = request(1, "BUILD", Some(context(90)));
    enqueue(
        &mut connection,
        &first_request,
        &OwnerEvidence::gather(&identity()),
        &Policy::default(),
        &context(1),
        None,
        10,
    )
    .unwrap();
    let snapshot = get(&connection, &id(1)).unwrap().unwrap();
    for proof in [
        UnspawnedEvidence::simulated(&snapshot, &context(91), IdentityStatus::Missing, 11),
        UnspawnedEvidence::simulated(&snapshot, &context(90), IdentityStatus::VerifiedMatch, 11),
        UnspawnedEvidence::simulated(&snapshot, &context(90), IdentityStatus::Indeterminate, 11),
    ] {
        assert!(!recover_unspawned(&mut connection, &proof).unwrap());
        assert_eq!(permit_counts(&connection), (1, 1));
    }
    let proof = UnspawnedEvidence::simulated(&snapshot, &context(90), IdentityStatus::Missing, 12);
    assert!(recover_unspawned(&mut connection, &proof).unwrap());
    assert!(!recover_unspawned(&mut connection, &proof).unwrap());
    assert_eq!(
        status(&connection, &id(1)).unwrap(),
        StatusResult::Cancelled {}
    );
    assert_eq!(permit_counts(&connection), (1, 0));

    enqueue(
        &mut connection,
        &request(2, "BUILD", Some(context(92))),
        &OwnerEvidence::gather(&identity()),
        &Policy::default(),
        &context(2),
        None,
        13,
    )
    .unwrap();
    let attach_request = Request::Attach {
        job_id: id(2),
        observer: identity(),
        child: identity(),
        permit_generation: 2,
    };
    attach(
        &mut connection,
        &attach_request,
        &ChildEvidence::gather(&identity()),
        14,
    )
    .unwrap();
    let attached = get(&connection, &id(2)).unwrap().unwrap();
    let forged = UnspawnedEvidence::simulated(&attached, &context(92), IdentityStatus::Missing, 15);
    assert!(!recover_unspawned(&mut connection, &forged).unwrap());
    assert_eq!(permit_counts(&connection), (2, 1));
}

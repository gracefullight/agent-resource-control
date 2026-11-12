use super::*;

#[test]
fn dead_owner_unknown_child_is_retained_then_released_once_after_durable_grace() {
    let db = Database::new();
    let mut c = db.open();
    let policy = Policy::default();
    add(&mut c, 1, "BUILD", 10, &policy);
    assert!(matches!(
        add(&mut c, 2, "BUILD", 11, &policy),
        Reply::Queued { .. }
    ));
    let job = get(&c, &id(1)).unwrap().unwrap();
    let live = UncertainEvidence::simulated(&job, IdentityStatus::VerifiedMatch, None, 12, 100);
    assert_eq!(
        reconcile_uncertain(&mut c, &live).unwrap(),
        UncertainResult::Unchanged
    );
    let unknown = UncertainEvidence::simulated(&job, IdentityStatus::Missing, None, 20, 100);
    assert_eq!(
        reconcile_uncertain(&mut c, &unknown).unwrap(),
        UncertainResult::Retained
    );
    assert_eq!(
        reconcile_uncertain(&mut c, &unknown).unwrap(),
        UncertainResult::Unchanged
    );
    assert_eq!(
        get(&c, &id(1)).unwrap().unwrap().phase,
        arc_ipc::jobs::Phase::OutcomeUnknown
    );
    assert_eq!(permit_counts(&c), (1, 1));
    drop(c);
    let mut c = db.open();
    let job = get(&c, &id(1)).unwrap().unwrap();
    let before = UncertainEvidence::simulated(&job, IdentityStatus::Missing, None, 119, 100);
    assert_eq!(
        reconcile_uncertain(&mut c, &before).unwrap(),
        UncertainResult::Retained
    );
    assert!(matches!(
        poll_job(&mut c, 2, 119, &policy),
        Reply::Queued { .. }
    ));
    let job = get(&c, &id(1)).unwrap().unwrap();
    let after = UncertainEvidence::simulated(&job, IdentityStatus::Missing, None, 120, 100);
    assert_eq!(
        reconcile_uncertain(&mut c, &after).unwrap(),
        UncertainResult::Released
    );
    assert_eq!(
        reconcile_uncertain(&mut c, &after).unwrap(),
        UncertainResult::Unchanged
    );
    assert_eq!(permit_counts(&c), (1, 0));
    assert_eq!(status(&c, &id(1)).unwrap(), StatusResult::Abandoned {});
    let events: Vec<(String, i64)> = c
        .prepare("SELECT event_type,created_at FROM audit_events ORDER BY created_at,id")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(
        events,
        vec![
            ("TRAFFIC_UNKNOWN_OWNER".into(), 20),
            ("TRAFFIC_POLICY_TERMINAL".into(), 120),
            ("TRAFFIC_GRACE_EXPIRED".into(), 120)
        ]
    );
    assert_eq!(generation(&poll_job(&mut c, 2, 121, &policy)), 2);
}

#[test]
fn live_or_definitively_exited_attached_child_cannot_use_unknown_recovery() {
    let db = Database::new();
    let mut c = db.open();
    add(&mut c, 1, "BUILD", 10, &Policy::default());
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
    for status in [
        IdentityStatus::VerifiedMatch,
        IdentityStatus::Missing,
        IdentityStatus::Mismatch,
    ] {
        let proof =
            UncertainEvidence::simulated(&job, IdentityStatus::Missing, Some(status), 1000, 1);
        assert_eq!(
            reconcile_uncertain(&mut c, &proof).unwrap(),
            UncertainResult::Unchanged
        );
    }
    assert_eq!(permit_counts(&c), (1, 1));
    let first = UncertainEvidence::simulated(
        &job,
        IdentityStatus::Missing,
        Some(IdentityStatus::Indeterminate),
        1001,
        1,
    );
    assert_eq!(
        reconcile_uncertain(&mut c, &first).unwrap(),
        UncertainResult::Retained
    );
    let job = get(&c, &id(1)).unwrap().unwrap();
    let later = UncertainEvidence::simulated(
        &job,
        IdentityStatus::Missing,
        Some(IdentityStatus::Indeterminate),
        1002,
        1,
    );
    assert_eq!(
        reconcile_uncertain(&mut c, &later).unwrap(),
        UncertainResult::Released
    );
    assert_eq!(permit_counts(&c), (1, 0));
}

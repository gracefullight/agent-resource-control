use super::*;
use crate::jobs::reconcile::simulated_pair;

#[test]
#[cfg(any(target_os = "linux", target_os = "macos"))]
fn lost_owner_and_exited_attached_child_release_on_observation_after_reopen() {
    struct Process(std::process::Child);
    impl Process {
        fn start() -> Self {
            Self(
                std::process::Command::new("/bin/sleep")
                    .arg("30")
                    .spawn()
                    .unwrap(),
            )
        }
        fn identity(&self) -> ProcessIdentity {
            ProcessIdentity::capture(self.0.id()).unwrap()
        }
        fn stop(&mut self) {
            self.0.kill().unwrap();
            self.0.wait().unwrap();
        }
    }
    impl Drop for Process {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    let mut owner = Process::start();
    let mut child = Process::start();
    let observer = owner.identity();
    let child_identity = child.identity();
    let db = Database::new();
    let mut c = db.open();
    let policy = Policy::default();
    let enqueue_request = Request::Enqueue {
        job_id: id(1),
        observer: observer.clone(),
        classification: classification("BUILD"),
        lineage_context: None,
    };
    let grant = enqueue(
        &mut c,
        &enqueue_request,
        &OwnerEvidence::gather(&observer),
        &policy,
        &context(1),
        None,
        10,
    )
    .unwrap();
    let attach_request = Request::Attach {
        job_id: id(1),
        observer,
        child: child_identity.clone(),
        permit_generation: generation(&grant),
    };
    attach(
        &mut c,
        &attach_request,
        &ChildEvidence::gather(&child_identity),
        11,
    )
    .unwrap();
    assert!(matches!(
        add(&mut c, 2, "BUILD", 12, &policy),
        Reply::Queued { .. }
    ));
    drop(c);
    owner.stop();
    let mut c = db.open();
    let live = get(&c, &id(1)).unwrap().unwrap();
    let live_observation = crate::jobs::observe(&live, 13);
    assert!(!live_observation.traffic_recovery_ready());
    crate::jobs::apply_observation(&mut c, &live_observation).unwrap();
    assert_eq!(permit_counts(&c), (1, 1));
    assert!(matches!(
        poll_job(&mut c, 2, 14, &policy),
        Reply::Queued { .. }
    ));

    let snapshot = get(&c, &id(1)).unwrap().unwrap();
    let stale = crate::jobs::observe(&snapshot, 15);
    child.stop();
    let ended = crate::jobs::observe(&snapshot, 16);
    assert!(ended.traffic_recovery_ready());
    crate::jobs::apply_observation(&mut c, &ended).unwrap();
    let unknown = get(&c, &id(1)).unwrap().unwrap();
    assert_eq!(unknown.phase, arc_ipc::jobs::Phase::OutcomeUnknown);
    assert_eq!(unknown.outcome, None);
    assert_eq!(unknown.completed_at_ms, None);
    assert_eq!(permit_counts(&c), (1, 0));
    assert_eq!(
        crate::jobs::apply_observation(&mut c, &stale).unwrap(),
        crate::jobs::ReconcileResult::Stale
    );
    drop(c);
    let mut c = db.open();
    assert_eq!(permit_counts(&c), (1, 0));
    assert_eq!(generation(&poll_job(&mut c, 2, 17, &policy)), 2);
    assert_eq!(permit_counts(&c), (2, 1));
}

#[test]
fn recovery_requires_definitive_owner_and_child_evidence() {
    let db = Database::new();
    let mut c = db.open();
    let policy = Policy::default();
    add(&mut c, 1, "BUILD", 10, &policy);
    let child = identity();
    let attach_request = Request::Attach {
        job_id: id(1),
        observer: identity(),
        child: child.clone(),
        permit_generation: 1,
    };
    attach(&mut c, &attach_request, &ChildEvidence::gather(&child), 11).unwrap();
    add(&mut c, 2, "BUILD", 12, &policy);
    for (time, owner, child) in [
        (13, IdentityStatus::VerifiedMatch, IdentityStatus::Missing),
        (14, IdentityStatus::Indeterminate, IdentityStatus::Mismatch),
        (15, IdentityStatus::Missing, IdentityStatus::VerifiedMatch),
        (16, IdentityStatus::Mismatch, IdentityStatus::Indeterminate),
    ] {
        let job = get(&c, &id(1)).unwrap().unwrap();
        let evidence = simulated_pair(&job, owner, child, time);
        assert!(!evidence.traffic_recovery_ready());
        crate::jobs::apply_observation(&mut c, &evidence).unwrap();
        assert_eq!(permit_counts(&c), (1, 1));
        assert!(matches!(
            poll_job(&mut c, 2, time, &policy),
            Reply::Queued { .. }
        ));
    }

    // A mismatched start token means the recorded PID was reused, not that the
    // original child is still executing under the new identity.
    let job = get(&c, &id(1)).unwrap().unwrap();
    let evidence = simulated_pair(&job, IdentityStatus::Missing, IdentityStatus::Mismatch, 17);
    assert!(evidence.traffic_recovery_ready());
    crate::jobs::apply_observation(&mut c, &evidence).unwrap();
    let unknown = get(&c, &id(1)).unwrap().unwrap();
    assert_eq!(unknown.phase, arc_ipc::jobs::Phase::OutcomeUnknown);
    assert_eq!(unknown.outcome, None);
    assert_eq!(unknown.completed_at_ms, None);
    assert_eq!(status(&c, &id(1)).unwrap(), StatusResult::Abandoned {});
    let values: (Option<i64>, Option<String>, i64) = c
        .query_row(
            "SELECT j.exit_code,l.outcome_kind,p.released_at FROM jobs j \
         JOIN job_lifecycle l ON l.job_id=j.id JOIN permits p ON p.job_id=j.id WHERE j.id=?1",
            [id(1).as_str()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(values, (None, None, 17));
    assert_eq!(
        crate::jobs::apply_observation(&mut c, &evidence).unwrap(),
        crate::jobs::ReconcileResult::Stale
    );
    let again = simulated_pair(
        &unknown,
        IdentityStatus::Missing,
        IdentityStatus::Mismatch,
        18,
    );
    crate::jobs::apply_observation(&mut c, &again).unwrap();
    let release: i64 = c
        .query_row(
            "SELECT released_at FROM permits WHERE job_id=?1",
            [id(1).as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(release, 17);
    drop(c);
    let mut c = db.open();
    assert_eq!(permit_counts(&c), (1, 0));
    assert_eq!(generation(&poll_job(&mut c, 2, 19, &policy)), 2);
}

#[test]
fn recovered_root_reservation_waits_for_every_recorded_lineage_member() {
    let db = Database::new();
    let mut c = db.open();
    let policy = Policy::default();
    let owner = OwnerEvidence::gather(&identity());
    let child = identity();
    add(&mut c, 1, "BUILD", 10, &policy);
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
    let anchor = lineage_snapshot(&c, &context(1)).unwrap().unwrap();
    let proof = LineageProof::verified(&anchor, &anchor.root.observer);
    for (n, now) in [(2, 12), (4, 13)] {
        assert!(matches!(
            enqueue(
                &mut c,
                &request(n, "BUILD", Some(context(1))),
                &owner,
                &policy,
                &context(n),
                Some(&proof),
                now
            ),
            Ok(Reply::Reused { .. })
        ));
    }
    attach(
        &mut c,
        &Request::Attach {
            job_id: id(2),
            observer: identity(),
            child: child.clone(),
            permit_generation: 1,
        },
        &ChildEvidence::gather(&child),
        14,
    )
    .unwrap();
    add(&mut c, 3, "BUILD", 15, &policy);

    let first = recovery_candidates(&c, 0, 1).unwrap();
    assert_eq!((first[0].sequence, &first[0].job.job_id), (1, &id(1)));
    let next = recovery_candidates(&c, first[0].sequence, 64).unwrap();
    assert_eq!(next.len(), 2);
    assert_eq!((next[0].sequence, &next[0].job.job_id), (2, &id(2)));
    assert_eq!((next[1].sequence, &next[1].job.job_id), (3, &id(4)));
    assert_eq!(next[1].witness_context, Some(context(1)));
    assert!(recovery_candidates(&c, 3, 64).unwrap().is_empty());
    assert!(matches!(
        recovery_candidates(&c, 0, 0),
        Err(JobError::InvalidRequest)
    ));
    assert!(matches!(
        recovery_candidates(&c, 0, 65),
        Err(JobError::InvalidRequest)
    ));
    assert!(matches!(
        recovery_candidates(&c, u64::MAX, 1),
        Err(JobError::InvalidRequest)
    ));

    let root = get(&c, &id(1)).unwrap().unwrap();
    crate::jobs::apply_observation(
        &mut c,
        &simulated_pair(&root, IdentityStatus::Missing, IdentityStatus::Missing, 16),
    )
    .unwrap();
    assert_eq!(permit_counts(&c), (1, 1));
    let remaining = lineage_snapshot(&c, &context(1)).unwrap().unwrap();
    assert_eq!(remaining.root.job_id, id(2));
    let nested = get(&c, &id(2)).unwrap().unwrap();
    crate::jobs::apply_observation(
        &mut c,
        &simulated_pair(
            &nested,
            IdentityStatus::Mismatch,
            IdentityStatus::Indeterminate,
            17,
        ),
    )
    .unwrap();
    assert_eq!(permit_counts(&c), (1, 1));
    let nested = get(&c, &id(2)).unwrap().unwrap();
    crate::jobs::apply_observation(
        &mut c,
        &simulated_pair(
            &nested,
            IdentityStatus::Mismatch,
            IdentityStatus::Mismatch,
            18,
        ),
    )
    .unwrap();
    assert_eq!(permit_counts(&c), (1, 1));
    // Member 4 is now a witness candidate, but discovery alone cannot release
    // its uncertain, unattached grant.
    let uncertain = recovery_candidates(&c, 0, 64).unwrap();
    assert_eq!(uncertain.len(), 1);
    assert_eq!(uncertain[0].job.job_id, id(4));
    assert_eq!(permit_counts(&c), (1, 1));
    drop(c);
    let mut c = db.open();
    assert_eq!(permit_counts(&c), (1, 1));
    assert_eq!(cancel_job(&mut c, 4, Some(1), 19), Reply::Cancelled {});
    assert_eq!(permit_counts(&c), (1, 0));
    assert_eq!(generation(&poll_job(&mut c, 3, 20, &policy)), 4);
}

#[test]
fn completed_root_releases_after_lost_descendant_is_reconciled() {
    let db = Database::new();
    let mut c = db.open();
    let policy = Policy::default();
    let owner = OwnerEvidence::gather(&identity());
    add(&mut c, 1, "BUILD", 10, &policy);
    let root = lineage_snapshot(&c, &context(1)).unwrap().unwrap();
    let proof = LineageProof::verified(&root, &root.root.observer);
    enqueue(
        &mut c,
        &request(2, "BUILD", Some(context(1))),
        &owner,
        &policy,
        &context(2),
        Some(&proof),
        11,
    )
    .unwrap();
    let child = identity();
    attach(
        &mut c,
        &Request::Attach {
            job_id: id(2),
            observer: identity(),
            child: child.clone(),
            permit_generation: 1,
        },
        &ChildEvidence::gather(&child),
        12,
    )
    .unwrap();
    complete_job(&mut c, 1, 1, Outcome::Exited { code: 0 }, 13);
    add(&mut c, 3, "BUILD", 14, &policy);
    assert_eq!(permit_counts(&c), (1, 1));
    drop(c);
    let mut c = db.open();
    let nested = get(&c, &id(2)).unwrap().unwrap();
    crate::jobs::apply_observation(
        &mut c,
        &simulated_pair(
            &nested,
            IdentityStatus::Missing,
            IdentityStatus::Mismatch,
            15,
        ),
    )
    .unwrap();
    assert_eq!(
        get(&c, &id(2)).unwrap().unwrap().phase,
        arc_ipc::jobs::Phase::OutcomeUnknown
    );
    assert_eq!(permit_counts(&c), (1, 0));
    assert_eq!(generation(&poll_job(&mut c, 3, 16, &policy)), 3);
}

#[test]
fn lost_owner_without_attached_child_cannot_release_uncertain_grant() {
    let db = Database::new();
    let mut c = db.open();
    let policy = Policy::default();
    add(&mut c, 1, "BUILD", 10, &policy);
    fallback(&mut c, &fallback_request(1, None, None, None), None, 11).unwrap();
    let job = get(&c, &id(1)).unwrap().unwrap();
    let evidence = crate::jobs::reconcile::simulated(&job, IdentityStatus::Missing, 12);
    assert!(!evidence.traffic_recovery_ready());
    crate::jobs::apply_observation(&mut c, &evidence).unwrap();
    assert_eq!(
        get(&c, &id(1)).unwrap().unwrap().phase,
        arc_ipc::jobs::Phase::OutcomeUnknown
    );
    assert_eq!(permit_counts(&c), (1, 1));
    let candidates = recovery_candidates(&c, 0, 64).unwrap();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].job.job_id, id(1));
}

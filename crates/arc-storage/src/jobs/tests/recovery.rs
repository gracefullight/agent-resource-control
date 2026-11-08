use super::{
    apply_observation, claimed, complete, completion, get, id, mutate, observe, observer, recent,
    register, start, Database, IdentityStatus, Outcome, Phase, ReconcileResult, StartDisposition,
    StartEvidence, StartRequest, StatusResult,
};
use crate::jobs::{reconcile::simulated, JobError};
use arc_ipc::jobs::{LifecycleReason, ObservationStatus};
use std::sync::{Arc, Barrier};

#[test]
fn reconciliation_never_replays_or_invents_exit_and_terminal_outcome_wins_races() {
    let db = Database::new();
    let mut c = db.open();
    let launching = claimed(&mut c, 1);
    for (n, status, reason) in [
        (2, IdentityStatus::Missing, LifecycleReason::ObserverMissing),
        (
            3,
            IdentityStatus::Mismatch,
            LifecycleReason::IdentityMismatch,
        ),
        (
            4,
            IdentityStatus::Indeterminate,
            LifecycleReason::IdentityUnavailable,
        ),
    ] {
        let j = claimed(&mut c, n);
        apply_observation(&mut c, &simulated(&j, status, 102)).unwrap();
        let unknown = get(&c, &id(n)).unwrap().unwrap();
        assert_eq!(unknown.phase, Phase::OutcomeUnknown);
        assert_eq!(unknown.reason, reason);
        assert_eq!(unknown.outcome, None);
        assert_eq!(unknown.completed_at_ms, None);
        assert_eq!(unknown.child, None);
    }
    let old_probe = observe(&launching, 110);
    let terminal = complete(&mut c, &completion(1, Outcome::Exited { code: 17 }), 111)
        .unwrap()
        .job;
    assert_eq!(
        apply_observation(&mut c, &old_probe).unwrap(),
        ReconcileResult::Unchanged(Box::new(terminal))
    );
    let registered = mutate(&mut c, &register(5), 100).job;
    apply_observation(
        &mut c,
        &simulated(&registered, IdentityStatus::Missing, 101),
    )
    .unwrap();
    let still_registered = get(&c, &id(5)).unwrap().unwrap();
    assert_eq!(still_registered.phase, Phase::Registered);
    assert_eq!(
        still_registered.reason,
        LifecycleReason::ObserverMissingNotClaimed
    );
    assert_eq!(still_registered.launch_claimed_at_ms, None);
}
#[test]
fn attached_identity_uncertainty_can_refine_without_rebinding_and_stale_updates_lose() {
    let db = Database::new();
    let mut c = db.open();
    claimed(&mut c, 1);
    let attach = StartRequest::Attach {
        job_id: id(1),
        observer: observer(),
        child: observer(),
    };
    let e = StartEvidence::simulated(&attach, IdentityStatus::Missing);
    let unknown = start(&mut c, &attach, &e, 102).unwrap().job;
    assert_eq!(unknown.phase, Phase::OutcomeUnknown);
    assert_eq!(unknown.child_verified_at_ms, None);
    assert_eq!(unknown.observation.status, ObservationStatus::Missing);
    let stale = observe(&unknown, 105);
    let evidence = observe(&unknown, 104);
    let result = apply_observation(&mut c, &evidence).unwrap();
    let ReconcileResult::Applied(running) = result else {
        panic!("expected fresh observation");
    };
    assert_eq!(running.phase, Phase::Running);
    assert_eq!(running.child_verified_at_ms, Some(104));
    assert_eq!(
        apply_observation(&mut c, &stale).unwrap(),
        ReconcileResult::Stale
    );
    for status in [
        IdentityStatus::Missing,
        IdentityStatus::Mismatch,
        IdentityStatus::Indeterminate,
    ] {
        let j = get(&c, &id(1)).unwrap().unwrap();
        apply_observation(&mut c, &simulated(&j, status, j.updated_at_ms + 1)).unwrap();
        let j = get(&c, &id(1)).unwrap().unwrap();
        assert_eq!(j.phase, Phase::OutcomeUnknown);
        assert_eq!(j.child, running.child);
        assert_eq!(j.child_verified_at_ms, Some(104));
        assert_eq!(j.outcome, None);
    }
    drop(c);
    let mut c = db.open();
    let j = get(&c, &id(1)).unwrap().unwrap();
    apply_observation(&mut c, &observe(&j, 110)).unwrap();
    let recovered = get(&c, &id(1)).unwrap().unwrap();
    assert_eq!(recovered.phase, Phase::Running);
    assert_eq!(recovered.child, running.child);
    assert_eq!(
        c.query_row("SELECT COUNT(*) FROM permits", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
}
#[test]
fn concurrent_stable_key_registration_claim_and_completion_have_one_winner() {
    let db = Database::new();
    drop(db.open());
    let barrier = Arc::new(Barrier::new(8));
    let threads: Vec<_> = (0..8)
        .map(|_| {
            let path = db.0.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let mut c = crate::open_and_migrate(path).unwrap();
                let r = register(1);
                let e = StartEvidence::gather(&r);
                barrier.wait();
                let a = start(&mut c, &r, &e, 100).unwrap().disposition;
                let claim = StartRequest::Claim {
                    job_id: id(1),
                    observer: observer(),
                };
                let b = mutate(&mut c, &claim, 101).disposition;
                let end = complete(&mut c, &completion(1, Outcome::Exited { code: 17 }), 102)
                    .unwrap()
                    .disposition;
                (a, b, end)
            })
        })
        .collect();
    let outcomes: Vec<_> = threads.into_iter().map(|t| t.join().unwrap()).collect();
    assert_eq!(
        outcomes
            .iter()
            .filter(|o| o.0 == StartDisposition::Created)
            .count(),
        1
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|o| o.1 == StartDisposition::Applied)
            .count(),
        1
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|o| o.2 == arc_ipc::jobs::CompleteDisposition::Applied)
            .count(),
        1
    );
    let c = db.open();
    assert_eq!(get(&c, &id(1)).unwrap().unwrap().revision, 3);
    assert_eq!(
        c.query_row("SELECT COUNT(*) FROM jobs", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
}
#[test]
fn bounded_recent_only_returns_lifecycle_rows_in_stable_order_without_probes() {
    let db = Database::new();
    let mut c = db.open();
    c.execute("INSERT INTO jobs (id,job_class,locality,enforcement,state,created_at) VALUES ('legacy','UNKNOWN','UNKNOWN','OBSERVE','NEW',100)", []).unwrap();
    for n in 1..=52 {
        mutate(&mut c, &register(n), 100);
    }
    let StatusResult::Recent {
        jobs,
        truncated,
        liveness_fresh,
        ..
    } = recent(&c, 101).unwrap()
    else {
        panic!("expected recent");
    };
    assert_eq!(jobs.len(), 50);
    assert!(truncated);
    assert!(!liveness_fresh);
    assert_eq!(jobs[0].job_id, id(52));
    assert_eq!(jobs[49].job_id, id(3));
    assert!(jobs.iter().all(|j| j.phase == Phase::Registered));
    let plan: Vec<String> = c
        .prepare(&format!(
            "EXPLAIN QUERY PLAN {}",
            crate::jobs::reconcile::RECENT_SQL
        ))
        .unwrap()
        .query_map([], |r| r.get(3))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(plan.iter().any(|p| p.contains("idx_jobs_lifecycle_recent")));
    assert!(plan.iter().any(|p| p.contains("MATERIALIZE candidates")));
    assert!(plan.iter().any(|p| p.contains("SEARCH j USING INDEX")));
}
#[test]
fn rollback_and_redacted_storage_errors_preserve_atomicity_and_secret_absence() {
    let db = Database::new();
    let mut c = db.open();
    c.execute_batch("CREATE TEMP TRIGGER fail_detail BEFORE INSERT ON job_lifecycle BEGIN SELECT RAISE(ABORT,'argv-env-secret-sentinel'); END;").unwrap();
    let r = register(1);
    let error = start(&mut c, &r, &StartEvidence::gather(&r), 100).unwrap_err();
    assert_eq!(error, JobError::StorageFailure);
    assert!(!error.to_string().contains("secret"));
    assert_eq!(
        c.query_row("SELECT COUNT(*) FROM jobs", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
    c.execute_batch("DROP TRIGGER fail_detail;").unwrap();
    claimed(&mut c, 1);
    complete(&mut c, &completion(1, Outcome::Exited { code: 17 }), 102).unwrap();
    let request = serde_json::json!({"action":"CLAIM","job_id":id(1),"observer":observer(),"argv":["argv-env-secret-sentinel"]});
    assert!(serde_json::from_value::<StartRequest>(request).is_err());
    for table in ["jobs", "job_lifecycle"] {
        let mut stmt = c.prepare(&format!("SELECT * FROM {table}")).unwrap();
        let n = stmt.column_count();
        let data = stmt
            .query_map([], |row| {
                (0..n)
                    .map(|i| row.get::<_, rusqlite::types::Value>(i))
                    .collect::<Result<Vec<_>, _>>()
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let text = format!("{data:?}");
        assert!(!text.contains("argv-env-secret-sentinel"));
        assert!(!text.contains(&std::env::current_dir().unwrap().display().to_string()));
    }
    for table in ["permits", "resource_leases", "policy_decisions"] {
        assert_eq!(
            c.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
}

use super::*;

#[test]
fn diagnostic_running_completed_and_retained_permits_are_historical_facts() {
    use arc_ipc::traffic::diagnostics::AdmissionState as S;
    let db = Database::new();
    let mut c = db.open();
    let policy = Policy::default();
    submit(
        &mut c,
        1,
        JobClass::Typecheck,
        Reason::BunScriptTypecheck,
        100,
    );
    let anchor = lineage_snapshot(&c, &context(1)).unwrap().unwrap();
    let proof = LineageProof::verified(&anchor, &anchor.root.observer);
    let mut nested = script_request(2, JobClass::Typecheck, Reason::BunScriptTypecheck);
    if let Request::Enqueue {
        lineage_context, ..
    } = &mut nested
    {
        *lineage_context = Some(context(1));
    }
    enqueue(
        &mut c,
        &nested,
        &OwnerEvidence::gather(&identity()),
        &policy,
        &context(2),
        Some(&proof),
        101,
    )
    .unwrap();
    attach(
        &mut c,
        &Request::Attach {
            job_id: id(1),
            observer: identity(),
            child: identity(),
            permit_generation: 1,
        },
        &ChildEvidence::gather(&identity()),
        102,
    )
    .unwrap();
    let running = diagnostics(&mut c, &id(1), &policy, 103).unwrap();
    assert!(!running.liveness_fresh);
    assert_eq!(running.admission.unwrap().state, S::Running);
    complete_job(&mut c, 1, 1, Outcome::Exited { code: 17 }, 104);
    let terminal = diagnostics(&mut c, &id(1), &policy, 105)
        .unwrap()
        .admission
        .unwrap();
    assert_eq!(terminal.state, S::Completed);
    assert!(terminal.permit_active);
    assert_eq!(terminal.outcome, Some(Outcome::Exited { code: 17 }));
    complete_job(&mut c, 2, 1, Outcome::Exited { code: 0 }, 106);
    assert!(
        !diagnostics(&mut c, &id(1), &policy, 107)
            .unwrap()
            .admission
            .unwrap()
            .permit_active
    );
}

#[test]
fn missing_decision_is_unknown_and_historical_missing_outcome_is_unconfirmed() {
    use arc_ipc::traffic::diagnostics::AdmissionState as S;
    let db = Database::new();
    let mut c = db.open();
    let policy = Policy::default();
    submit(
        &mut c,
        1,
        JobClass::Typecheck,
        Reason::BunScriptTypecheck,
        100,
    );
    submit(
        &mut c,
        2,
        JobClass::Typecheck,
        Reason::BunScriptTypecheck,
        101,
    );
    c.execute(
        "DELETE FROM policy_decisions WHERE job_id=?1",
        [id(2).as_str()],
    )
    .unwrap();
    let queued = diagnostics(&mut c, &id(2), &policy, 102)
        .unwrap()
        .admission
        .unwrap();
    assert_eq!(queued.state, S::Queued);
    assert!(queued.wait_reason.is_none() && queued.last_decision.is_none());
    complete_job(&mut c, 1, 1, Outcome::Exited { code: 17 }, 103);
    c.execute(
        "UPDATE jobs SET state='COMPLETED',exit_code=NULL WHERE id=?1",
        [id(1).as_str()],
    )
    .unwrap();
    c.execute(
        "UPDATE job_lifecycle SET outcome_kind=NULL WHERE job_id=?1",
        [id(1).as_str()],
    )
    .unwrap();
    let historical = diagnostics(&mut c, &id(1), &policy, 104)
        .unwrap()
        .admission
        .unwrap();
    assert_eq!(historical.state, S::Unconfirmed);
    assert!(historical.outcome.is_none() && !historical.permit_active);
}

#[test]
fn corrupt_permit_or_timestamps_produce_unavailable_instead_of_fabricated_diagnostics() {
    for statement in [
        "UPDATE permits SET state='CORRUPT'",
        "UPDATE permits SET cpu_millis_est=NULL",
        "UPDATE traffic_entries SET granted_at_ms=200",
    ] {
        let db = Database::new();
        let mut c = db.open();
        let policy = Policy::default();
        submit(
            &mut c,
            1,
            JobClass::Typecheck,
            Reason::BunScriptTypecheck,
            100,
        );
        c.execute_batch(statement).unwrap();
        assert_eq!(
            diagnostics(&mut c, &id(1), &policy, 101),
            Err(JobError::StorageFailure)
        );
    }
}

#[test]
fn diagnostics_keep_recorded_typecheck_truth_through_cancel_complete_and_reopen() {
    use arc_ipc::traffic::diagnostics::AdmissionState as S;
    let db = Database::new();
    let policy = Policy::default();
    let mut c = db.open();
    let absent = diagnostics(&mut c, &id(1), &policy, 99).unwrap();
    assert!(absent.classification.is_none() && absent.admission.is_none());
    let first = submit(
        &mut c,
        1,
        JobClass::Typecheck,
        Reason::BunScriptTypecheck,
        100,
    );
    submit(
        &mut c,
        2,
        JobClass::Typecheck,
        Reason::BunScriptTypecheck,
        101,
    );
    let before = c.total_changes();
    let reserved = diagnostics(&mut c, &id(1), &policy, 102).unwrap();
    assert_eq!(
        before,
        c.total_changes(),
        "diagnostics must not write or reconcile"
    );
    assert!(!reserved.liveness_fresh);
    assert_eq!(
        reserved.classification.unwrap().reason,
        Reason::BunScriptTypecheck
    );
    let admission = reserved.admission.unwrap();
    assert_eq!(admission.state, S::Reserved);
    assert!(admission.permit_active);
    assert_eq!(admission.root_job_id, Some(id(1)));
    assert_eq!(admission.reservation, Some(policy.typecheck.reservation));
    let queued = diagnostics(&mut c, &id(2), &policy, 102)
        .unwrap()
        .admission
        .unwrap();
    assert_eq!(queued.state, S::Queued);
    assert_eq!(
        queued.wait_reason,
        Some(arc_policy::DecisionReason::ClassWindow)
    );
    assert!(queued.permit_generation.is_none());
    assert_eq!(cancel_job(&mut c, 2, None, 103), Reply::Cancelled {});
    assert_eq!(
        diagnostics(&mut c, &id(2), &policy, 104)
            .unwrap()
            .admission
            .unwrap()
            .state,
        S::Cancelled
    );
    complete_job(
        &mut c,
        1,
        generation(&first),
        Outcome::Exited { code: 17 },
        105,
    );
    drop(c);
    let mut c = db.open();
    let done = diagnostics(&mut c, &id(1), &policy, 106)
        .unwrap()
        .admission
        .unwrap();
    assert_eq!(done.state, S::Completed);
    assert_eq!(done.outcome, Some(Outcome::Exited { code: 17 }));
    assert_eq!(done.queue_wait_ms, Some(0));
    assert!(!done.permit_active);
    assert_eq!(done.permit_generation, Some(generation(&first)));
    assert_eq!(permit_counts(&c), (1, 0));
}

#[test]
fn typecheck_lineage_uses_one_root_permit_and_retains_terminal_root_diagnostics() {
    use arc_ipc::traffic::diagnostics::AdmissionState as S;
    let db = Database::new();
    let mut c = db.open();
    let policy = Policy::default();
    let first = submit(
        &mut c,
        1,
        JobClass::Typecheck,
        Reason::BunScriptTypecheck,
        100,
    );
    let anchor = lineage_snapshot(&c, &context(1)).unwrap().unwrap();
    let proof = LineageProof::verified(&anchor, &anchor.root.observer);
    let mut nested = script_request(2, JobClass::Lint, Reason::BunScriptLint);
    if let Request::Enqueue {
        lineage_context, ..
    } = &mut nested
    {
        *lineage_context = Some(context(1));
    }
    assert!(matches!(
        enqueue(
            &mut c,
            &nested,
            &OwnerEvidence::gather(&identity()),
            &policy,
            &context(2),
            Some(&proof),
            101
        )
        .unwrap(),
        Reply::Reused {
            permit_generation: 1,
            ..
        }
    ));
    let active = active_reservations(&c).unwrap();
    assert_eq!((active.typecheck, active.lint), (1, 0));
    assert_eq!(active.resources, policy.typecheck.reservation);
    assert_eq!(
        cancel_job(&mut c, 1, None, 102),
        Reply::ReservationRetained {}
    );
    let retained = diagnostics(&mut c, &id(1), &policy, 103)
        .unwrap()
        .admission
        .unwrap();
    assert_eq!(retained.state, S::Unconfirmed);
    assert!(retained.permit_active);
    confirm_cancel_unspawned(
        &mut c,
        &Request::Cancel {
            job_id: id(1),
            observer: identity(),
            permit_generation: None,
        },
        &OwnerEvidence::gather(&identity()),
        104,
    )
    .unwrap();
    let cancelled = diagnostics(&mut c, &id(1), &policy, 105)
        .unwrap()
        .admission
        .unwrap();
    assert_eq!(cancelled.state, S::Cancelled);
    assert!(cancelled.permit_active);
    assert_eq!(cancelled.reservation, Some(policy.typecheck.reservation));
    let child = diagnostics(&mut c, &id(2), &policy, 106)
        .unwrap()
        .admission
        .unwrap();
    assert_eq!(child.root_job_id, Some(id(1)));
    complete_job(
        &mut c,
        2,
        generation(&first),
        Outcome::Exited { code: 0 },
        107,
    );
    for n in [1, 2] {
        assert!(
            !diagnostics(&mut c, &id(n), &policy, 108)
                .unwrap()
                .admission
                .unwrap()
                .permit_active
        );
    }
    assert_eq!(
        active_reservations(&c).unwrap(),
        arc_traffic::Active::default()
    );
    assert_eq!(permit_counts(&c), (1, 0));
}

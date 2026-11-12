use super::*;

fn reset(c: &Connection, r: &JobMeterReport) {
    c.execute(
        "UPDATE job_meter_summaries SET relation='unverified',root_job_id=NULL,child_bound=0
        WHERE job_id=?1",
        [&r.job_id],
    )
    .unwrap();
}
fn job(c: &Connection, r: &JobMeterReport) -> JobSnapshot {
    crate::jobs::get(c, &JobId::try_from(r.job_id.clone()).unwrap())
        .unwrap()
        .unwrap()
}
fn bind_fixture(
    c: &Connection,
    r: &JobMeterReport,
    ancestors: Vec<ProcessIdentity>,
    complete: bool,
) {
    let proof = LineageEvidence::simulated(&r.observer, ancestors, complete, Some(r.child.clone()));
    bind(c, &job(c, r), &proof).unwrap();
}

#[test]
fn native_ancestry_binds_cross_mode_nested_roots_and_excludes_unrelated_peers() {
    let mut c = db();
    let root = seed(&c, 1);
    reset(&c, &root);
    bind_fixture(&c, &root, vec![], true);
    c.execute(
        "UPDATE jobs SET state='RUNNING',finished_at=NULL,exit_code=NULL WHERE id=?1",
        [&root.job_id],
    )
    .unwrap();
    c.execute("UPDATE job_lifecycle SET outcome_kind=NULL,reason='CHILD_VERIFIED',observation_reason='CHILD_VERIFIED'
        WHERE job_id=?1", [&root.job_id]).unwrap();
    let mut nested = seed(&c, 2);
    reset(&c, &nested);
    bind_fixture(
        &c,
        &nested,
        vec![root.child.clone(), root.observer.clone()],
        true,
    );
    assert_eq!(
        binding(&c, &nested.job_id).unwrap().unwrap().root,
        Some(root.job_id.clone())
    );
    nested.accounting_relation = AccountingRelation::IncludedInRoot {
        root_job_id: root.job_id.clone(),
    };
    report(&mut c, &nested, 300).unwrap();
    assert!(!nested.accounting_relation.is_additive());
    let unrelated = seed(&c, 3);
    reset(&c, &unrelated);
    bind_fixture(&c, &unrelated, vec![], true);
    assert_eq!(
        binding(&c, &unrelated.job_id).unwrap().unwrap().root,
        Some(unrelated.job_id)
    );
    // Accepted root/scope evidence cannot be rewritten by a later bind.
    bind_fixture(&c, &nested, vec![], true);
    assert_eq!(
        binding(&c, &nested.job_id).unwrap().unwrap().root,
        Some(root.job_id)
    );
}

#[test]
fn missing_lineage_and_contradictory_traffic_root_never_promote_to_additive() {
    let mut c = db();
    let mut r = seed(&c, 1);
    reset(&c, &r);
    bind_fixture(&c, &r, vec![], false);
    assert_eq!(report(&mut c, &r, 300), Err(MeterError::IdentityMismatch));
    r.accounting_relation = AccountingRelation::Unverified {};
    r.completeness = Completeness::Partial;
    r.missing_coverage = vec![MissingCoverageReason::LineageUnverified];
    report(&mut c, &r, 300).unwrap();
    let root = seed(&c, 2);
    let other = seed(&c, 3);
    reset(&c, &other);
    c.execute("INSERT INTO traffic_entries(job_id,state,queued_at_ms,granted_at_ms,generation,root_job_id)
        VALUES(?1,'COMPLETED',100,150,1,?2)", params![other.job_id,root.job_id]).unwrap();
    bind_fixture(&c, &other, vec![], true);
    assert_eq!(binding(&c, &other.job_id).unwrap().unwrap().root, None);
    assert_eq!(
        report(&mut c, &other, 300),
        Err(MeterError::IdentityMismatch)
    );
}

#[test]
fn mismatched_evidence_and_reparenting_fail_closed() {
    let c = db();
    let r = seed(&c, 1);
    let bad = LineageEvidence::simulated(&fixture(2).observer, vec![], true, Some(r.child.clone()));
    assert_eq!(
        bind(&c, &job(&c, &r), &bad),
        Err(crate::jobs::JobError::InvalidRequest)
    );
    bind_fixture(&c, &r, vec![], false);
    assert_eq!(binding(&c, &r.job_id).unwrap().unwrap().root, None);
}

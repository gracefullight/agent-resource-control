//! Cancellation after a pause retires the old generation without weakening
//! legacy generation checks or allowing a stale retry to revive terminal work.
use arc_ipc::{
    governor::*,
    jobs::{JobId, ProcessIdentity},
    traffic::{JobRequest, JobResult, LineageContext},
};
use arc_storage::{
    governor,
    jobs::{
        traffic::{self, governor as execution},
        JobError,
    },
    MIGRATIONS,
};
use rusqlite::Connection;

#[test]
fn converted_reservation_requires_owner_cancellation_not_a_retired_permit_generation() {
    let mut c = Connection::open_in_memory().unwrap();
    c.execute_batch("PRAGMA foreign_keys=ON").unwrap();
    for migration in MIGRATIONS {
        c.execute_batch(migration.sql).unwrap();
    }
    let owner = ProcessIdentity::capture(std::process::id()).unwrap();
    let evidence = traffic::OwnerEvidence::gather(&owner);
    let id = JobId::try_from("a".repeat(32)).unwrap();
    let context = LineageContext::try_from("b".repeat(64)).unwrap();
    let provider = arc_policy::evaluator::MissingObservation;
    let request=JobRequest::Enqueue {job_id:id.clone(),observer:owner.clone(),classification:serde_json::from_value(serde_json::json!({"classification":"BUILD","locality":"HOST_LOCAL","reason":"CARGO_BUILD","uncertainty":"NONE"})).unwrap(),lineage_context:None};
    let GovernorJobResult::Traffic {
        result: JobResult::Granted {
            permit_generation, ..
        },
    } = execution::enqueue(
        &mut c,
        &request,
        &evidence,
        &arc_traffic::Policy::default(),
        &context,
        None,
        10,
        &provider,
    )
    .unwrap()
    else {
        panic!("reservation")
    };
    let pause = SetGovernorSwitchRequest {
        key: GovernorSwitchKey::GovernorEnabled,
        enabled: false,
        expected_revision: 0,
        mutation_id: format!("0-{}", "c".repeat(32)),
    };
    let tx = c.transaction().unwrap();
    governor::set_switch(&tx, &pause, 20).unwrap();
    tx.commit().unwrap();
    let mut cancel = JobRequest::Cancel {
        job_id: id.clone(),
        observer: owner,
        permit_generation: Some(permit_generation),
    };
    assert_eq!(
        traffic::cancel_observed(&mut c, &cancel, &evidence, 30, &provider),
        Err(JobError::Conflict)
    );
    let JobRequest::Cancel {
        permit_generation: generation,
        ..
    } = &mut cancel
    else {
        unreachable!()
    };
    *generation = None;
    assert_eq!(
        traffic::cancel_observed(&mut c, &cancel, &evidence, 31, &provider).unwrap(),
        JobResult::Cancelled {}
    );
    let resume = SetGovernorSwitchRequest {
        key: GovernorSwitchKey::GovernorEnabled,
        enabled: true,
        expected_revision: 1,
        mutation_id: format!("1-{}", "d".repeat(32)),
    };
    let tx = c.transaction().unwrap();
    governor::set_switch(&tx, &resume, 40).unwrap();
    tx.commit().unwrap();
    for now in [41, 42] {
        assert_eq!(
            traffic::cancel_observed(&mut c, &cancel, &evidence, now, &provider).unwrap(),
            JobResult::Cancelled {}
        );
    }
    assert!(matches!(
        traffic::status(&c, &id).unwrap(),
        arc_ipc::traffic::StatusResult::Cancelled {}
    ));
    let counts:(u32,u32,u32)=c.query_row("SELECT (SELECT count(*) FROM permits),(SELECT count(*) FROM permits WHERE state='ACTIVE'),(SELECT count(*) FROM job_lifecycle WHERE child_pid IS NOT NULL)",[],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    assert_eq!(counts, (1, 0, 0));
}

use super::{
    apply_observation, claimed, complete, completion, get, id, observer, recent, start, Database,
    IdentityStatus, JobError, Outcome, Phase, ReconcileResult, StartEvidence, StartRequest,
    StatusResult,
};
use arc_ipc::jobs::{
    CompleteDisposition, LifecycleReason, ObservationStatus, ObservationTarget, SpawnReason,
};
use rusqlite::{types::Value, Connection};

fn stored_record(c: &Connection, n: u32) -> Vec<Value> {
    let mut statement = c
        .prepare("SELECT j.*, l.* FROM jobs j JOIN job_lifecycle l ON l.job_id=j.id WHERE j.id=?1")
        .unwrap();
    let column_count = statement.column_count();
    statement
        .query_row([id(n).as_str()], |row| {
            (0..column_count).map(|index| row.get(index)).collect()
        })
        .unwrap()
}

#[test]
fn historical_unknown_reconciliation_preserves_the_complete_stored_record() {
    let db = Database::new();
    let mut c = db.open();
    for (n, clear_kind) in [(1, true), (2, false)] {
        claimed(&mut c, n);
        complete(&mut c, &completion(n, Outcome::Exited { code: 17 }), 102).unwrap();
        c.execute(
            "UPDATE jobs SET state='COMPLETED', exit_code=NULL WHERE id=?1",
            [id(n).as_str()],
        )
        .unwrap();
        if clear_kind {
            c.execute(
                "UPDATE job_lifecycle SET outcome_kind=NULL WHERE job_id=?1",
                [id(n).as_str()],
            )
            .unwrap();
        }
        let (state, finished, code, kind): (String, Option<i64>, Option<u8>, Option<String>) = c
            .query_row(
                "SELECT j.state,j.finished_at,j.exit_code,l.outcome_kind FROM jobs j JOIN job_lifecycle l ON l.job_id=j.id WHERE j.id=?1",
                [id(n).as_str()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .unwrap();
        assert_eq!(state, "COMPLETED");
        assert_eq!(finished, Some(102));
        assert_eq!(code, None);
        assert_eq!(
            kind.as_deref(),
            if clear_kind { None } else { Some("EXITED") }
        );
        let before = stored_record(&c, n);
        let snapshot = get(&c, &id(n)).unwrap().unwrap();
        assert_eq!(snapshot.phase, Phase::OutcomeUnknown);
        assert_eq!(snapshot.outcome, None);
        let observation =
            crate::jobs::reconcile::simulated(&snapshot, IdentityStatus::Missing, 103);
        assert_eq!(
            apply_observation(&mut c, &observation).unwrap(),
            ReconcileResult::Unchanged(Box::new(snapshot))
        );
        assert_eq!(stored_record(&c, n), before);

        // A delayed attachment must not rewrite the synthetic unknown snapshot.
        let attach = StartRequest::Attach {
            job_id: id(n),
            observer: observer(),
            child: observer(),
        };
        let evidence = StartEvidence::gather(&attach);
        assert_eq!(
            start(&mut c, &attach, &evidence, 104).unwrap_err(),
            JobError::InvalidTransition
        );
        assert_eq!(stored_record(&c, n), before);
    }
}

#[test]
fn historical_unknown_preserves_verified_child_without_inventing_identity_failure() {
    let db = Database::new();
    let mut c = db.open();
    for (n, clear_kind) in [(1, true), (2, false)] {
        claimed(&mut c, n);
        let attach = StartRequest::Attach {
            job_id: id(n),
            observer: observer(),
            child: observer(),
        };
        start(&mut c, &attach, &StartEvidence::gather(&attach), 102).unwrap();
        complete(&mut c, &completion(n, Outcome::Exited { code: 17 }), 103).unwrap();
        c.execute(
            "UPDATE jobs SET state='COMPLETED', exit_code=NULL WHERE id=?1",
            [id(n).as_str()],
        )
        .unwrap();
        if clear_kind {
            c.execute(
                "UPDATE job_lifecycle SET outcome_kind=NULL WHERE job_id=?1",
                [id(n).as_str()],
            )
            .unwrap();
        }
        let before = stored_record(&c, n);
        let job = get(&c, &id(n)).unwrap().unwrap();
        assert!(job.is_valid());
        assert_eq!(job.phase, Phase::OutcomeUnknown);
        assert_eq!(job.outcome, None);
        assert_eq!(job.observation.target, ObservationTarget::Child);
        assert_eq!(job.observation.status, ObservationStatus::VerifiedMatch);
        assert_eq!(job.reason, LifecycleReason::None);
        assert_eq!(job.observation.reason, LifecycleReason::None);
        let StatusResult::Recent { jobs, .. } = recent(&c, 104).unwrap() else {
            panic!("recent")
        };
        let summary = jobs.iter().find(|j| j.job_id == id(n)).unwrap();
        assert_eq!(summary.phase, Phase::OutcomeUnknown);
        assert_eq!(summary.reason, LifecycleReason::None);
        let observation = crate::jobs::reconcile::simulated(&job, IdentityStatus::Missing, 104);
        apply_observation(&mut c, &observation).unwrap();
        assert_eq!(stored_record(&c, n), before);
    }
}

#[test]
fn exit_states_are_durable_and_retries_keep_protocol_one_completed() {
    let db = Database::new();
    let mut c = db.open();
    for (n, code, state) in [(1, 0, "SUCCEEDED"), (2, 17, "FAILED")] {
        claimed(&mut c, n);
        let request = completion(n, Outcome::Exited { code });
        let first = complete(&mut c, &request, 102).unwrap();
        assert_eq!(first.disposition, CompleteDisposition::Applied);
        assert_eq!(first.job.phase, Phase::Completed);
        assert_eq!(
            serde_json::to_value(&first.job).unwrap()["phase"],
            "COMPLETED"
        );
        assert_eq!(
            complete(&mut c, &request, 103).unwrap().disposition,
            CompleteDisposition::Unchanged
        );
        assert_eq!(
            c.query_row(
                "SELECT state FROM jobs WHERE id=?1",
                [id(n).as_str()],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            state
        );
    }
    drop(c);
    let c = db.open();
    for (n, code) in [(1, 0), (2, 17)] {
        assert_eq!(
            get(&c, &id(n)).unwrap().unwrap().outcome,
            Some(Outcome::Exited { code })
        );
    }
    let StatusResult::Recent { jobs, .. } = recent(&c, 104).unwrap() else {
        panic!("recent")
    };
    assert!(jobs.iter().all(|job| job.phase == Phase::Completed));
}

#[test]
fn historical_completed_uses_only_recorded_exit_and_new_states_reject_mismatches() {
    let db = Database::new();
    let mut c = db.open();
    for (n, code) in [(1, 0), (2, 17), (3, 17), (4, 17), (5, 17)] {
        claimed(&mut c, n);
        complete(&mut c, &completion(n, Outcome::Exited { code }), 102).unwrap();
        c.execute(
            "UPDATE jobs SET state='COMPLETED' WHERE id=?1",
            [id(n).as_str()],
        )
        .unwrap();
    }
    // Older rows may have only an exit code, or no recorded outcome at all.
    c.execute(
        "UPDATE job_lifecycle SET outcome_kind=NULL WHERE job_id=?1",
        [id(3).as_str()],
    )
    .unwrap();
    c.execute(
        "UPDATE job_lifecycle SET outcome_kind=NULL WHERE job_id=?1",
        [id(4).as_str()],
    )
    .unwrap();
    c.execute(
        "UPDATE jobs SET exit_code=NULL WHERE id=?1",
        [id(4).as_str()],
    )
    .unwrap();
    c.execute(
        "UPDATE jobs SET exit_code=NULL WHERE id=?1",
        [id(5).as_str()],
    )
    .unwrap();
    drop(c);
    let c = db.open();
    for (n, code) in [(1, 0), (2, 17), (3, 17)] {
        let job = get(&c, &id(n)).unwrap().unwrap();
        assert_eq!(job.phase, Phase::Completed);
        assert_eq!(job.outcome, Some(Outcome::Exited { code }));
    }
    for n in [4, 5] {
        let unknown = get(&c, &id(n)).unwrap().unwrap();
        assert_eq!(unknown.phase, Phase::OutcomeUnknown);
        assert_eq!(unknown.outcome, None);
    }
    let StatusResult::Recent { jobs, .. } = recent(&c, 104).unwrap() else {
        panic!("recent")
    };
    for n in [4, 5] {
        assert_eq!(
            jobs.iter().find(|j| j.job_id == id(n)).unwrap().phase,
            Phase::OutcomeUnknown
        );
    }

    for (n, state) in [(1, "FAILED"), (2, "SUCCEEDED"), (4, "FAILED")] {
        c.execute(
            "UPDATE jobs SET state=?2 WHERE id=?1",
            (id(n).as_str(), state),
        )
        .unwrap();
        assert_eq!(get(&c, &id(n)), Err(JobError::StorageFailure));
        assert_eq!(recent(&c, 104), Err(JobError::StorageFailure));
        c.execute(
            "UPDATE jobs SET state='COMPLETED' WHERE id=?1",
            [id(n).as_str()],
        )
        .unwrap();
    }
}

#[test]
fn non_exit_outcomes_keep_their_distinct_states() {
    let db = Database::new();
    let mut c = db.open();
    for (n, outcome, state) in [
        (
            1,
            Outcome::Signaled {
                signal: 15,
                core_dumped: false,
            },
            "SIGNALED",
        ),
        (
            2,
            Outcome::SpawnFailed {
                reason: SpawnReason::NotFound,
                os_code: Some(2),
            },
            "SPAWN_FAILED",
        ),
    ] {
        claimed(&mut c, n);
        let job = complete(&mut c, &completion(n, outcome.clone()), 102)
            .unwrap()
            .job;
        assert_eq!(job.outcome, Some(outcome));
        assert_eq!(
            c.query_row(
                "SELECT state FROM jobs WHERE id=?1",
                [id(n).as_str()],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            state
        );
    }
}

#[test]
fn protocol_one_phase_decoder_rejects_new_database_states() {
    for stored in ["SUCCEEDED", "FAILED"] {
        assert!(serde_json::from_value::<Phase>(serde_json::json!(stored)).is_err());
    }
    assert_eq!(
        serde_json::from_value::<Phase>(serde_json::json!("COMPLETED")).unwrap(),
        Phase::Completed
    );
}

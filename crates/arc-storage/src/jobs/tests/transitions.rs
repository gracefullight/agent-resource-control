use super::{
    claimed, complete, completion, get, id, mutate, observer, register, start, Database,
    IdentityStatus, JobError, Outcome, Phase, StartDisposition, StartEvidence, StartRequest,
};
use arc_ipc::jobs::{CompleteDisposition, SpawnReason};

#[test]
fn register_claim_attach_complete_and_identical_retries_preserve_revision_and_times() {
    let db = Database::new();
    let mut c = db.open();
    let request = register(1);
    let registered = mutate(&mut c, &request, 100);
    assert_eq!(registered.disposition, StartDisposition::Created);
    let retried = mutate(&mut c, &request, 105);
    assert_eq!(retried.disposition, StartDisposition::Unchanged);
    assert_eq!(registered.job, retried.job);
    let request = StartRequest::Claim {
        job_id: id(1),
        observer: observer(),
    };
    let launching = mutate(&mut c, &request, 106);
    assert_eq!(launching.job.phase, Phase::Launching);
    assert_eq!(mutate(&mut c, &request, 107).job, launching.job);
    let attach = StartRequest::Attach {
        job_id: id(1),
        observer: observer(),
        child: observer(),
    };
    let running = mutate(&mut c, &attach, 108);
    assert_eq!(running.job.phase, Phase::Running);
    assert_eq!(running.job.child_verified_at_ms, Some(108));
    assert_eq!(mutate(&mut c, &attach, 109).job, running.job);
    let end = completion(1, Outcome::Exited { code: 17 });
    let completed = complete(&mut c, &end, 110).unwrap();
    assert_eq!(completed.disposition, CompleteDisposition::Applied);
    assert_eq!(completed.job.phase, Phase::Completed);
    assert_eq!(
        complete(&mut c, &end, 111).unwrap().disposition,
        CompleteDisposition::Unchanged
    );
    assert_eq!(complete(&mut c, &end, 112).unwrap().job, completed.job);
    assert_eq!(mutate(&mut c, &attach, 113).job, completed.job);
    assert_eq!(mutate(&mut c, &request, 114).job, completed.job);
    assert_eq!(mutate(&mut c, &register(1), 115).job, completed.job);
    drop(c);
    assert_eq!(get(&db.open(), &id(1)).unwrap().unwrap(), completed.job);
}
#[test]
fn conflicts_and_illegal_transitions_rollback_both_tables() {
    let db = Database::new();
    let mut c = db.open();
    let original = claimed(&mut c, 1);
    let mut bad = register(1);
    if let StartRequest::Register { observer, .. } = &mut bad {
        observer.start_token = "1".into();
    }
    assert_eq!(
        start(&mut c, &bad, &StartEvidence::gather(&bad), 105),
        Err(JobError::Conflict)
    );
    let mut bad = register(1);
    if let StartRequest::Register { classification, .. } = &mut bad {
        classification.reason = arc_ipc::Reason::UnresolvedCommand;
    }
    assert_eq!(
        start(&mut c, &bad, &StartEvidence::gather(&bad), 105),
        Err(JobError::Conflict)
    );
    assert_eq!(get(&c, &id(1)).unwrap().unwrap(), original);
    let attach = StartRequest::Attach {
        job_id: id(1),
        observer: observer(),
        child: observer(),
    };
    let attached = mutate(&mut c, &attach, 106).job;
    let mut bad = attach.clone();
    if let StartRequest::Attach { child, .. } = &mut bad {
        child.start_token = "1".into();
    }
    assert_eq!(
        start(&mut c, &bad, &StartEvidence::gather(&bad), 107),
        Err(JobError::Conflict)
    );
    let spawn = completion(
        1,
        Outcome::SpawnFailed {
            reason: SpawnReason::Other,
            os_code: None,
        },
    );
    assert_eq!(
        complete(&mut c, &spawn, 108),
        Err(JobError::InvalidTransition)
    );
    assert_eq!(get(&c, &id(1)).unwrap().unwrap(), attached);
    let end = completion(1, Outcome::Exited { code: 17 });
    let terminal = complete(&mut c, &end, 109).unwrap().job;
    assert_eq!(
        complete(&mut c, &completion(1, Outcome::Exited { code: 18 }), 110),
        Err(JobError::Conflict)
    );
    let mut wrong = end;
    wrong.observer.start_token = "1".into();
    assert_eq!(complete(&mut c, &wrong, 111), Err(JobError::Conflict));
    assert_eq!(get(&c, &id(1)).unwrap().unwrap(), terminal);
    mutate(&mut c, &register(2), 100);
    assert_eq!(
        complete(&mut c, &completion(2, Outcome::Exited { code: 17 }), 101),
        Err(JobError::InvalidTransition)
    );
    let claim = StartRequest::Claim {
        job_id: id(99),
        observer: observer(),
    };
    assert_eq!(
        start(&mut c, &claim, &StartEvidence::gather(&claim), 105),
        Err(JobError::InvalidTransition)
    );
}
#[test]
fn fast_exit_without_attachment_and_spawn_failures_are_real_outcome_types() {
    let db = Database::new();
    let mut c = db.open();
    for (n, outcome, phase) in [
        (1, Outcome::Exited { code: 17 }, Phase::Completed),
        (
            2,
            Outcome::Signaled {
                signal: 15,
                core_dumped: false,
            },
            Phase::Signaled,
        ),
        (
            3,
            Outcome::SpawnFailed {
                reason: SpawnReason::NotFound,
                os_code: Some(2),
            },
            Phase::SpawnFailed,
        ),
    ] {
        claimed(&mut c, n);
        let request = completion(n, outcome);
        let result = complete(&mut c, &request, 102).unwrap();
        assert_eq!(result.job.phase, phase);
        assert_eq!(result.job.child_verified_at_ms, None);
        assert_eq!(complete(&mut c, &request, 103).unwrap().job, result.job);
        let attach = StartRequest::Attach {
            job_id: id(n),
            observer: observer(),
            child: observer(),
        };
        assert_eq!(
            start(&mut c, &attach, &StartEvidence::gather(&attach), 104),
            Err(JobError::InvalidTransition)
        );
    }
}
#[test]
fn evidence_validation_and_time_validation_fail_without_writes() {
    let db = Database::new();
    let mut c = db.open();
    let r = register(1);
    for status in [
        IdentityStatus::Missing,
        IdentityStatus::Mismatch,
        IdentityStatus::Indeterminate,
    ] {
        let e = StartEvidence::simulated(&r, status);
        assert_eq!(
            start(&mut c, &r, &e, 100),
            Err(JobError::IdentityUnavailable)
        );
    }
    assert!(get(&c, &id(1)).unwrap().is_none());
    assert_eq!(
        start(&mut c, &r, &StartEvidence::gather(&r), u64::MAX),
        Err(JobError::InvalidRequest)
    );
    let j = claimed(&mut c, 1);
    assert_eq!(
        complete(&mut c, &completion(1, Outcome::Exited { code: 17 }), 99),
        Err(JobError::InvalidRequest)
    );
    assert_eq!(get(&c, &id(1)).unwrap().unwrap(), j);
}

#[test]
fn native_zombie_identity_is_not_accepted_as_a_live_observer() {
    let db = Database::new();
    let mut c = db.open();
    let mut child = std::process::Command::new("/usr/bin/true").spawn().unwrap();
    let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
    let waited = unsafe {
        libc::waitid(
            libc::P_PID,
            child.id(),
            &mut info,
            libc::WEXITED | libc::WNOWAIT,
        )
    };
    let identity = arc_core::process_identity::ProcessIdentity::capture(child.id());
    let request = identity.map(|observer| StartRequest::Register {
        job_id: id(1),
        observer,
        classification: super::arc_detector_classification(),
    });
    let evidence = request.as_ref().ok().map(StartEvidence::gather);
    child.wait().unwrap();
    assert_eq!(waited, 0);
    let request = request.unwrap();
    let evidence = evidence.unwrap();
    assert_eq!(
        start(&mut c, &request, &evidence, 100),
        Err(JobError::IdentityUnavailable)
    );
    assert!(get(&c, &id(1)).unwrap().is_none());
}

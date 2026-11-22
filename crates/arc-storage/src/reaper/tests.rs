use super::*;
use crate::jobs::{complete, start, StartEvidence};
use arc_ipc::jobs::{CompleteRequest, Outcome, StartRequest};
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

struct Database(PathBuf);
impl Database {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "arc-reaper-store-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&dir).unwrap();
        Self(dir.join("test.sqlite3"))
    }
    fn open(&self) -> Connection {
        crate::open_and_migrate(&self.0).unwrap()
    }
}
impl Drop for Database {
    fn drop(&mut self) {
        fs::remove_dir_all(self.0.parent().unwrap()).unwrap();
    }
}

fn id(n: u32) -> JobId {
    JobId::try_from(format!("{n:032x}")).unwrap()
}
fn observer() -> ProcessIdentity {
    ProcessIdentity::capture(std::process::id()).unwrap()
}
// Storage records identities; it performs no ownership probe. The runner's
// live parent stands in for an attached child without spawning a process.
fn child() -> ProcessIdentity {
    ProcessIdentity::capture(unsafe { libc::getppid() } as u32).unwrap()
}
fn session() -> ProcessIdentity {
    let mut session = observer();
    session.pid = 1;
    session
}
fn running(c: &mut Connection, n: u32) {
    for request in [
        StartRequest::Register {
            job_id: id(n),
            observer: observer(),
            classification: serde_json::from_str(r#"{"classification":"UNKNOWN","locality":"UNKNOWN","reason":"UNSUPPORTED_SYNTAX","uncertainty":"OBSERVE"}"#).unwrap(),
            usage: None,
        },
        StartRequest::Claim {
            job_id: id(n),
            observer: observer(),
        },
        StartRequest::Attach {
            job_id: id(n),
            observer: observer(),
            child: child(),
        },
    ] {
        start(c, &request, &StartEvidence::gather(&request), 100).unwrap();
    }
}
fn registration(n: u32) -> Registration {
    Registration {
        job_id: id(n),
        observer: observer(),
        child: ChildEvidence {
            process: child(),
            parent_pid: std::process::id(),
            executable: "/private/fixture/secret-name".into(),
            executable_device: 7,
            executable_inode: 9,
            real_uid: 501,
            effective_uid: 501,
        },
        session: session(),
    }
}
fn decision(n: u32, state: DecisionState, reason: &'static str, at: u64) -> Decision {
    Decision {
        job_id: id(n),
        state,
        reason,
        observed_at_ms: at,
        evidence: serde_json::json!({"level": "PROVEN"}),
        tracking: Tracking::Keep,
    }
}
fn finish(c: &mut Connection, n: u32) {
    let request = CompleteRequest {
        job_id: id(n),
        observer: observer(),
        outcome: Outcome::Signaled {
            signal: 15,
            core_dumped: false,
        },
    };
    complete(c, &request, 900).unwrap();
}
fn states(c: &Connection) -> Vec<(String, String)> {
    c.prepare("SELECT state,error_code FROM gc_operations ORDER BY rowid")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

#[test]
fn registration_requires_the_recorded_running_child_and_is_idempotent() {
    let database = Database::new();
    let mut c = database.open();
    assert_eq!(
        register(&mut c, &registration(1), 200),
        Err(JobError::InvalidRequest)
    );
    running(&mut c, 1);
    let mut forged = registration(1);
    forged.child.process.start_token = "1".into();
    assert_eq!(
        register(&mut c, &forged, 200),
        Err(JobError::InvalidRequest)
    );
    let mut forged = registration(1);
    forged.observer = child();
    assert_eq!(
        register(&mut c, &forged, 200),
        Err(JobError::InvalidRequest)
    );
    register(&mut c, &registration(1), 200).unwrap();
    register(&mut c, &registration(1), 201).unwrap();
    let mut changed = registration(1);
    changed.child.executable_inode = 10;
    assert_eq!(register(&mut c, &changed, 202), Err(JobError::Conflict));

    let found = inventory(&c, 300, 8).unwrap();
    assert_eq!(found.len(), 1);
    let resource = found[0].resource.as_ref().unwrap();
    assert_eq!(resource.ownership_level, LEVEL_PROVEN);
    assert_eq!(resource.state, STATE_TRACKED);
    assert_eq!(resource.session, Some(session()));
    assert_eq!(resource.session_ended_at_ms, None);
    assert_eq!(resource.active_references, 1);
    assert!(!resource.protected);

    running(&mut c, 2);
    finish(&mut c, 2);
    assert_eq!(
        register(&mut c, &registration(2), 300),
        Err(JobError::InvalidRequest)
    );
}

#[test]
fn a_signal_claim_is_fenced_by_ownership_leases_protection_and_the_running_job() {
    let database = Database::new();
    let mut c = database.open();
    running(&mut c, 1);
    running(&mut c, 2);
    let claim = |n| decision(n, DecisionState::Terminating, "SIGTERM_REQUESTED", 400);

    // Unknown ownership is inventoried once, protected, and can never be claimed.
    assert_eq!(inventory(&c, 300, 8).unwrap().len(), 2);
    assert!(decide(
        &mut c,
        &decision(2, DecisionState::Blocked, "UNKNOWN_OWNERSHIP", 300)
    )
    .unwrap());
    assert_eq!(decide(&mut c, &claim(2)), Err(JobError::InvalidTransition));
    let found = inventory(&c, 300, 8).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].job.job_id, id(1));
    assert!(found[0].resource.is_none());

    register(&mut c, &registration(1), 200).unwrap();
    // The owner session's lease is still active.
    assert_eq!(decide(&mut c, &claim(1)), Err(JobError::InvalidTransition));
    assert_eq!(end_session(&mut c, &id(1), 350).unwrap(), 350);
    assert_eq!(end_session(&mut c, &id(1), 399).unwrap(), 350);
    assert_eq!(
        end_session(&mut c, &id(3), 399),
        Err(JobError::InvalidTransition)
    );

    // Another holder's unexpired lease is an active reference; it expires.
    c.execute(
        "INSERT INTO resource_leases(id,resource_id,acquired_at,expires_at,state) VALUES('other','process:00000000000000000000000000000001',1,401,'ACTIVE')",
        [],
    )
    .unwrap();
    assert_eq!(decide(&mut c, &claim(1)), Err(JobError::InvalidTransition));
    c.execute("UPDATE resource_objects SET protected=1", [])
        .unwrap();
    let late = decision(1, DecisionState::Terminating, "SIGTERM_REQUESTED", 401);
    assert_eq!(decide(&mut c, &late), Err(JobError::InvalidTransition));
    c.execute("UPDATE resource_objects SET protected=0", [])
        .unwrap();
    assert!(decide(&mut c, &late).unwrap());
    // Each claim rechecks the fence; an identical decision is not duplicated.
    assert!(!decide(&mut c, &late).unwrap());
    assert_eq!(
        inventory(&c, 500, 8).unwrap()[0]
            .resource
            .as_ref()
            .unwrap()
            .state,
        STATE_TERMINATING
    );
}

#[test]
fn reclaimed_requires_a_recorded_outcome_and_every_decision_is_audited() {
    let database = Database::new();
    let mut c = database.open();
    running(&mut c, 1);
    register(&mut c, &registration(1), 200).unwrap();
    let reclaimed = decision(1, DecisionState::Reclaimed, "TERMINATION_CONFIRMED", 950);
    // Not claimed, and no consuming wait has recorded an outcome.
    assert_eq!(decide(&mut c, &reclaimed), Err(JobError::InvalidTransition));
    assert!(decide(
        &mut c,
        &decision(1, DecisionState::RetryLater, "SESSION_GRACE_PENDING", 360)
    )
    .unwrap());
    end_session(&mut c, &id(1), 350).unwrap();
    assert!(decide(
        &mut c,
        &decision(1, DecisionState::Terminating, "SIGTERM_REQUESTED", 400)
    )
    .unwrap());
    assert_eq!(decide(&mut c, &reclaimed), Err(JobError::InvalidTransition));
    finish(&mut c, 1);
    // A finished job can no longer be claimed for another signal.
    assert_eq!(
        decide(
            &mut c,
            &decision(1, DecisionState::Terminating, "SIGKILL_REQUESTED", 940)
        ),
        Err(JobError::InvalidTransition)
    );
    assert!(decide(&mut c, &reclaimed).unwrap());
    assert!(inventory(&c, 960, 8).unwrap().is_empty());
    assert_eq!(
        states(&c),
        [
            ("RETRY_LATER".to_owned(), "SESSION_GRACE_PENDING".to_owned()),
            ("TERMINATING".to_owned(), "SIGTERM_REQUESTED".to_owned()),
            ("RECLAIMED".to_owned(), "TERMINATION_CONFIRMED".to_owned()),
        ]
    );
    let audits: Vec<(String, String, String, i64)> = c
        .prepare("SELECT severity,event_type,payload_redacted_json,created_at FROM audit_events ORDER BY rowid")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(audits.len(), 3);
    for (severity, event, payload, _) in &audits {
        assert_eq!((severity.as_str(), event.as_str()), ("INFO", AUDIT_EVENT));
        assert!(!payload.contains("secret-name"), "{payload}");
    }
    let last: serde_json::Value = serde_json::from_str(&audits[2].2).unwrap();
    assert_eq!(
        last,
        serde_json::json!({"job_id": id(1).as_str(), "state": "RECLAIMED",
            "reason": "TERMINATION_CONFIRMED", "observed_at_ms": 950, "ownership_level": "PROVEN"})
    );
    assert_eq!(audits[2].3, 950);
    let (ended, evidence): (i64, String) = c
        .query_row(
            "SELECT ended_at,ownership_evidence FROM gc_operations WHERE state='RECLAIMED'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(ended, 950);
    assert!(!evidence.contains("secret-name"));
}

#[test]
fn unconfirmed_termination_is_a_partial_failure_and_a_finished_job_is_released() {
    let database = Database::new();
    let mut c = database.open();
    for n in [1, 2] {
        running(&mut c, n);
        register(&mut c, &registration(n), 200).unwrap();
    }
    let partial = decision(
        1,
        DecisionState::PartialFailure,
        "TERMINATION_UNCONFIRMED",
        800,
    );
    assert_eq!(decide(&mut c, &partial), Err(JobError::InvalidTransition));
    end_session(&mut c, &id(1), 350).unwrap();
    decide(
        &mut c,
        &decision(1, DecisionState::Terminating, "SIGTERM_REQUESTED", 400),
    )
    .unwrap();
    assert!(decide(&mut c, &partial).unwrap());
    let severity: String = c
        .query_row(
            "SELECT severity FROM audit_events ORDER BY rowid DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(severity, "WARN");

    finish(&mut c, 2);
    let mut released = decision(2, DecisionState::Skipped, "JOB_COMPLETED", 950);
    released.tracking = Tracking::Release;
    assert!(decide(&mut c, &released).unwrap());
    assert!(inventory(&c, 960, 8).unwrap().is_empty());
    assert_eq!(
        decide(
            &mut c,
            &decision(9, DecisionState::Blocked, "UNKNOWN_OWNERSHIP", 1)
        ),
        Err(JobError::InvalidRequest)
    );
    assert_eq!(
        inventory(&c, MAX_TIMESTAMP + 1, 8),
        Err(JobError::InvalidRequest)
    );
}

#[test]
fn a_claim_that_sent_no_signal_is_withdrawn_and_no_other_state_is() {
    let database = Database::new();
    let mut c = database.open();
    running(&mut c, 1);
    register(&mut c, &registration(1), 200).unwrap();
    end_session(&mut c, &id(1), 350).unwrap();
    let state = |c: &Connection| -> String {
        c.query_row("SELECT state FROM resource_objects", [], |row| row.get(0))
            .unwrap()
    };
    let refused = |reason, at| {
        let mut refused = decision(1, DecisionState::Blocked, reason, at);
        refused.tracking = Tracking::Withdraw;
        refused
    };
    decide(
        &mut c,
        &decision(1, DecisionState::Terminating, "SIGTERM_REQUESTED", 400),
    )
    .unwrap();
    assert_eq!(state(&c), STATE_TERMINATING);
    assert!(decide(&mut c, &refused("WRAPPER_IDENTITY_MISMATCH", 400)).unwrap());
    assert_eq!(state(&c), STATE_TRACKED);

    // A withdrawn claim is no termination: once the job ends by itself it can
    // be recorded neither as reclaimed nor as an unconfirmed termination.
    finish(&mut c, 1);
    for (state, reason) in [
        (DecisionState::Reclaimed, "TERMINATION_CONFIRMED"),
        (DecisionState::PartialFailure, "TERMINATION_UNATTRIBUTED"),
    ] {
        assert_eq!(
            decide(&mut c, &decision(1, state, reason, 950)),
            Err(JobError::InvalidTransition)
        );
    }
    let mut released = decision(1, DecisionState::Skipped, "JOB_COMPLETED", 950);
    released.tracking = Tracking::Release;
    assert!(decide(&mut c, &released).unwrap());
    // Only a claim can be withdrawn: a released resource is not tracked again.
    assert!(decide(&mut c, &refused("WRAPPER_FOREGROUND", 960)).unwrap());
    assert_eq!(state(&c), "RELEASED");
    assert!(inventory(&c, 970, 8).unwrap().is_empty());
}

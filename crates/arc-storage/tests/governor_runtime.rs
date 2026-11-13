//! Native identities plus deterministic SQLite disposition/receipt transitions.
use arc_ipc::{
    governor::*,
    jobs::{JobId, Outcome, ProcessIdentity},
    traffic::{JobRequest, JobResult as Legacy, LineageContext},
};
use arc_storage::{
    governor as state,
    jobs::{
        traffic::{self, governor as execution},
        JobError,
    },
    MIGRATIONS,
};
use rusqlite::{Connection, TransactionBehavior};
#[path = "../../arc-policy/tests/support/memory_observation.rs"]
mod observation;
fn db() -> Connection {
    let c = Connection::open_in_memory().unwrap();
    c.execute_batch("PRAGMA foreign_keys=ON").unwrap();
    for m in MIGRATIONS {
        c.execute_batch(m.sql).unwrap()
    }
    c
}
fn intent(base: u64, enabled: bool, nonce: u32) -> SetGovernorSwitchRequest {
    SetGovernorSwitchRequest {
        key: GovernorSwitchKey::GovernorEnabled,
        enabled,
        expected_revision: base,
        mutation_id: format!("{base}-{nonce:032x}"),
    }
}
fn set(c: &mut Connection, q: &SetGovernorSwitchRequest, now: u64) -> GovernorMutationResult {
    let tx = c
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .unwrap();
    let r = state::set_switch(&tx, q, now).unwrap();
    tx.commit().unwrap();
    assert!(r.is_valid_for(q));
    r
}
fn owner() -> ProcessIdentity {
    ProcessIdentity::capture(std::process::id()).unwrap()
}
fn id(n: u32) -> JobId {
    JobId::try_from(format!("{n:032x}")).unwrap()
}
fn enqueue(
    c: &mut Connection,
    n: u32,
    provider: &dyn arc_policy::ObservationProvider,
) -> GovernorJobResult {
    let request=JobRequest::Enqueue{job_id:id(n),observer:owner(),classification:serde_json::from_value(serde_json::json!({"classification":"BUILD","locality":"HOST_LOCAL","reason":"CARGO_BUILD","uncertainty":"NONE"})).unwrap(),lineage_context:None};
    execution::enqueue(
        c,
        &request,
        &traffic::OwnerEvidence::gather(&owner()),
        &arc_traffic::Policy::default(),
        &LineageContext::try_from(format!("{n:064x}")).unwrap(),
        None,
        10,
        provider,
    )
    .unwrap()
}
fn count(c: &Connection, sql: &str) -> i64 {
    c.query_row(sql, [], |r| r.get(0)).unwrap()
}
#[test]
fn default_noop_replay_conflict_rollback_and_historical_readback() {
    let mut c = db();
    let q = intent(0, false, 1);
    assert_eq!(
        state::get_switch(&c).unwrap(),
        GovernorSwitchState::default()
    );
    {
        let tx = c.transaction().unwrap();
        state::set_switch(&tx, &q, 42).unwrap();
    }
    assert!(state::get_switch(&c).unwrap().enabled);
    assert_eq!(
        count(&c, "SELECT count(*) FROM governor_mutation_receipts"),
        0
    );
    let first = set(&mut c, &q, 42);
    assert!(matches!(
        first,
        GovernorMutationResult::Committed {
            changed: true,
            replayed: false,
            ..
        }
    ));
    assert!(matches!(
        set(&mut c, &q, 100),
        GovernorMutationResult::Committed { replayed: true, .. }
    ));
    let mut conflicting = q.clone();
    conflicting.enabled = true;
    assert!(matches!(
        set(&mut c, &conflicting, 100),
        GovernorMutationResult::Conflict {
            reason: GovernorConflictReason::MutationIdConflict,
            ..
        }
    ));
    assert!(matches!(
        set(&mut c, &intent(0, true, 2), 101),
        GovernorMutationResult::Conflict {
            reason: GovernorConflictReason::RevisionConflict,
            ..
        }
    ));
    let unchanged = set(&mut c, &intent(1, false, 3), 102);
    assert!(matches!(
        unchanged,
        GovernorMutationResult::Committed { changed: false, .. }
    ));
    assert_eq!(unchanged.switch(), first.switch());
    set(&mut c, &intent(1, true, 4), 103);
    let read = state::read_switch(
        &c,
        &GetGovernorSwitchRequest {
            key: GovernorSwitchKey::GovernorEnabled,
            mutation_id: Some(q.mutation_id),
        },
    )
    .unwrap();
    assert_eq!(read.switch.revision, 2);
    assert!(read.switch.enabled);
    assert_eq!(read.mutation_result.unwrap().switch(), first.switch());
    let mut invalid = intent(2, false, 5);
    invalid.mutation_id = "invalid".into();
    let tx = c.transaction().unwrap();
    assert_eq!(
        state::set_switch(&tx, &invalid, 104),
        Err(JobError::InvalidRequest)
    );
}
#[test]
fn bounded_receipts_never_evict_current_base_and_evicted_ids_cannot_reapply() {
    let mut c = db();
    for n in 0..MAX_MUTATION_RECEIPTS {
        assert!(matches!(
            set(&mut c, &intent(0, true, n as u32), 1),
            GovernorMutationResult::Committed { changed: false, .. }
        ))
    }
    assert!(matches!(
        set(&mut c, &intent(0, true, 2000), 2),
        GovernorMutationResult::Rejected {
            reason: GovernorRejectionReason::ReceiptCapacity,
            ..
        }
    ));
    assert!(matches!(
        set(&mut c, &intent(0, false, 2001), 3),
        GovernorMutationResult::Committed { changed: true, .. }
    ));
    assert_eq!(
        count(&c, "SELECT count(*) FROM governor_mutation_receipts"),
        MAX_MUTATION_RECEIPTS as i64
    );
    assert!(matches!(
        set(&mut c, &intent(0, true, 0), 4),
        GovernorMutationResult::Conflict {
            reason: GovernorConflictReason::RevisionConflict,
            ..
        }
    ));
    assert!(matches!(
        set(&mut c, &intent(0, true, 1), 5),
        GovernorMutationResult::Committed { replayed: true, .. }
    ));
}
#[test]
fn revision_exhaustion_rejects_changes_but_accepts_noops() {
    let mut c = db();
    c.execute("INSERT INTO policy_states(key,enabled,policy_revision,updated_at) VALUES('governor.enabled',1,?1,1)",[MAX_SAFE_INTEGER as i64]).unwrap();
    assert!(matches!(
        set(&mut c, &intent(MAX_SAFE_INTEGER, false, 1), 2),
        GovernorMutationResult::Rejected {
            reason: GovernorRejectionReason::RevisionExhausted,
            ..
        }
    ));
    assert!(matches!(
        set(&mut c, &intent(MAX_SAFE_INTEGER, true, 2), 2),
        GovernorMutationResult::Committed { changed: false, .. }
    ));
}
#[test]
fn pause_converts_reserved_and_queued_once_resume_uses_fresh_generation() {
    let mut c = db();
    let normal = observation::Provider::new(arc_core::meter::MemoryPressure::Normal);
    let GovernorJobResult::Traffic {
        result: Legacy::Granted {
            permit_generation: old,
            ..
        },
    } = enqueue(&mut c, 1, &normal)
    else {
        panic!()
    };
    assert!(matches!(
        enqueue(&mut c, 2, &normal),
        GovernorJobResult::Traffic {
            result: Legacy::Queued { .. }
        }
    ));
    set(&mut c, &intent(0, false, 1), 20);
    assert_eq!(
        count(&c, "SELECT count(*) FROM permits WHERE state='ACTIVE'"),
        0
    );
    assert_eq!(
        count(
            &c,
            "SELECT count(*) FROM governor_jobs WHERE mode='UNRESERVED'"
        ),
        2
    );
    let tx = c.transaction().unwrap();
    assert!(execution::prepare(&tx, &id(1), 21).unwrap());
    tx.commit().unwrap();
    assert_eq!(count(&c, "SELECT count(*) FROM permits"), 1);
    set(&mut c, &intent(1, true, 2), 22);
    let poll = JobRequest::Poll {
        job_id: id(1),
        observer: owner(),
    };
    let result = execution::poll(
        &mut c,
        &poll,
        &traffic::OwnerEvidence::gather(&owner()),
        &arc_traffic::Policy::default(),
        &LineageContext::try_from(format!("{:064x}", 3)).unwrap(),
        None,
        23,
        &normal,
    )
    .unwrap();
    let GovernorJobResult::Traffic {
        result: Legacy::Granted {
            permit_generation: new,
            ..
        },
    } = result
    else {
        panic!("{result:?}")
    };
    assert!(new > old);
    assert_eq!(count(&c, "SELECT count(*) FROM permits"), 1);
    assert_eq!(
        count(&c, "SELECT count(*) FROM permits WHERE state='ACTIVE'"),
        1
    );
    let request = arc_ipc::policy::launch::LaunchRequest {
        job_id: id(1),
        observer: owner(),
        permit_generation: old,
    };
    let tx = c.transaction().unwrap();
    assert_eq!(
        traffic::launch(
            &tx,
            &request,
            &traffic::OwnerEvidence::gather(&owner()),
            None,
            &normal.policy(),
            24
        ),
        Err(JobError::InvalidTransition)
    );
}
#[test]
fn off_bypasses_fresh_critical_claim_is_one_shot_and_resume_restores_hold() {
    let mut c = db();
    let critical = observation::Provider::new(arc_core::meter::MemoryPressure::Critical);
    assert!(matches!(
        enqueue(&mut c, 1, &critical),
        GovernorJobResult::Traffic {
            result: Legacy::Queued { .. }
        }
    ));
    set(&mut c, &intent(0, false, 1), 20);
    assert!(matches!(
        enqueue(&mut c, 2, &critical),
        GovernorJobResult::Unreserved { .. }
    ));
    let token = format!("{:032x}", 2);
    let tx = c.transaction().unwrap();
    assert!(matches!(
        execution::claim(
            &tx,
            &id(2),
            &owner(),
            &traffic::OwnerEvidence::gather(&owner()),
            &token,
            21
        )
        .unwrap(),
        GovernorLaunchResult::Unreserved { .. }
    ));
    tx.commit().unwrap();
    let tx = c.transaction().unwrap();
    assert!(matches!(
        execution::claim(
            &tx,
            &id(2),
            &owner(),
            &traffic::OwnerEvidence::gather(&owner()),
            &format!("{:032x}", 3),
            22
        )
        .unwrap(),
        GovernorLaunchResult::Unconfirmed {}
    ));
    tx.commit().unwrap();
    set(&mut c, &intent(1, true, 2), 23);
    assert_eq!(
        execution::disposition(&c, &id(2))
            .unwrap()
            .unwrap()
            .release_id
            .as_ref(),
        Some(&token)
    );
    assert!(execution::unspawned(
        &mut c,
        &id(2),
        &owner(),
        &format!("{:032x}", 3),
        &traffic::OwnerEvidence::gather(&owner()),
        24
    )
    .is_err());
    execution::unspawned(
        &mut c,
        &id(2),
        &owner(),
        &token,
        &traffic::OwnerEvidence::gather(&owner()),
        24,
    )
    .unwrap();
    assert!(
        !execution::disposition(&c, &id(2))
            .unwrap()
            .unwrap()
            .unreserved
    );
    assert!(matches!(
        enqueue(&mut c, 3, &critical),
        GovernorJobResult::Traffic {
            result: Legacy::Queued { .. }
        }
    ));
    assert_eq!(count(&c, "SELECT count(*) FROM permits"), 0);
}
#[test]
fn delivered_reserved_release_is_preserved_and_cancel_before_enqueue_is_terminal() {
    let mut c = db();
    let normal = observation::Provider::new(arc_core::meter::MemoryPressure::Normal);
    let GovernorJobResult::Traffic {
        result: Legacy::Granted {
            permit_generation, ..
        },
    } = enqueue(&mut c, 1, &normal)
    else {
        panic!()
    };
    let tx = c.transaction().unwrap();
    let request = arc_ipc::policy::launch::LaunchRequest {
        job_id: id(1),
        observer: owner(),
        permit_generation,
    };
    assert!(matches!(
        traffic::launch(
            &tx,
            &request,
            &traffic::OwnerEvidence::gather(&owner()),
            None,
            &normal.policy(),
            11
        )
        .unwrap(),
        arc_ipc::policy::launch::LaunchResult::Ready { .. }
    ));
    tx.commit().unwrap();
    set(&mut c, &intent(0, false, 1), 12);
    assert!(
        !execution::disposition(&c, &id(1))
            .unwrap()
            .unwrap()
            .unreserved
    );
    assert_eq!(
        count(&c, "SELECT count(*) FROM permits WHERE state='ACTIVE'"),
        1
    );
    traffic::cancel_observed(
        &mut c,
        &JobRequest::Cancel {
            job_id: id(2),
            observer: owner(),
            permit_generation: None,
        },
        &traffic::OwnerEvidence::gather(&owner()),
        13,
        &normal,
    )
    .unwrap();
    assert!(matches!(
        enqueue(&mut c, 2, &normal),
        GovernorJobResult::Traffic {
            result: Legacy::Cancelled {}
        }
    ));
    assert!(execution::disposition(&c, &id(2)).unwrap().is_none());
}
#[path = "governor_runtime/lifecycle.rs"]
mod lifecycle;

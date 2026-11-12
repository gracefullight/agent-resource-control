use super::observation::*;
use arc_core::{meter::*, process_identity::ProcessIdentity};
use arc_ipc::{
    jobs::{JobId, Outcome},
    policy::launch::*,
    traffic::{JobRequest, JobResult as Reply, LineageContext, WaitReason},
};
use arc_policy::*;
use arc_storage::{
    jobs::{self, traffic::*},
    policy,
};
use arc_traffic::Policy;
use rusqlite::Connection;

pub fn database() -> Connection {
    let c = Connection::open_in_memory().unwrap();
    for m in arc_storage::MIGRATIONS {
        c.execute_batch(m.sql).unwrap();
    }
    c
}
pub fn owner() -> ProcessIdentity {
    ProcessIdentity::capture(std::process::id()).unwrap()
}
pub fn id(n: u32) -> JobId {
    JobId::try_from(format!("{n:032x}")).unwrap()
}
pub fn context(n: u32) -> LineageContext {
    LineageContext::try_from(format!("{n:064x}")).unwrap()
}
pub fn request(n: u32, reason: &str) -> JobRequest {
    let class = match reason {
        "CARGO_BUILD" => "BUILD",
        "CARGO_TEST" | "BUN_TEST" => "TEST",
        _ => "LINT",
    };
    JobRequest::Enqueue{job_id:id(n),observer:owner(),lineage_context:None,
        classification:serde_json::from_value(serde_json::json!({"classification":class,"locality":"HOST_LOCAL","reason":reason,"uncertainty":"NONE"})).unwrap()}
}
pub fn add(c: &mut Connection, n: u32, source: &dyn ObservationProvider) -> Reply {
    enqueue_observed(
        c,
        &request(n, "CARGO_BUILD"),
        &OwnerEvidence::gather(&owner()),
        &Policy::default(),
        &context(n),
        None,
        1000,
        source,
    )
    .unwrap()
}
pub fn poll_job(c: &mut Connection, n: u32, source: &dyn ObservationProvider) -> Reply {
    poll_observed(
        c,
        &JobRequest::Poll {
            job_id: id(n),
            observer: owner(),
        },
        &OwnerEvidence::gather(&owner()),
        &Policy::default(),
        &context(n),
        None,
        1001,
        source,
    )
    .unwrap()
}
pub fn counts(c: &Connection) -> (i64, i64) {
    c.query_row(
        "SELECT count(*),sum(state='ACTIVE') FROM permits",
        [],
        |r| Ok((r.get(0)?, r.get::<_, Option<i64>>(1)?.unwrap_or(0))),
    )
    .unwrap()
}
pub fn generation(reply: Reply) -> u64 {
    match reply {
        Reply::Granted {
            permit_generation, ..
        }
        | Reply::Reused {
            permit_generation, ..
        } => permit_generation,
        _ => panic!("expected grant {reply:?}"),
    }
}
pub fn release(c: &mut Connection, n: u32, generation: u64) {
    complete(
        c,
        &JobRequest::Complete {
            job_id: id(n),
            observer: owner(),
            permit_generation: generation,
            outcome: Outcome::Exited { code: 17 },
        },
        1002,
    )
    .unwrap();
}
pub fn launch_at(
    c: &mut Connection,
    n: u32,
    generation: u64,
    p: &Provider,
) -> Result<LaunchResult, jobs::JobError> {
    policy::transaction(c, p, |tx, effective| {
        launch(
            tx,
            &LaunchRequest {
                job_id: id(n),
                observer: owner(),
                permit_generation: generation,
            },
            &OwnerEvidence::gather(&owner()),
            None,
            effective,
            1001,
        )
    })
}
#[test]
fn every_supported_class_holds_with_free_slots_and_duplicate_polls_do_not_append() {
    let mut c = database();
    let p = Provider::new(MemoryPressure::Critical);
    for (n, reason) in [
        "CARGO_BUILD",
        "CARGO_TEST",
        "BUN_TEST",
        "CARGO_CLIPPY",
        "CARGO_FMT",
    ]
    .into_iter()
    .enumerate()
    {
        let n = n as u32 + 1;
        let reply = enqueue_observed(
            &mut c,
            &request(n, reason),
            &OwnerEvidence::gather(&owner()),
            &Policy::default(),
            &context(n),
            None,
            1000,
            &p,
        )
        .unwrap();
        assert!(matches!(
            reply,
            Reply::Queued {
                reason: WaitReason::MemoryBudget,
                ..
            }
        ));
        let read = policy::job(&c, &id(n), p.policy(), 1000).unwrap();
        assert_eq!(
            read.last_decision.unwrap().reason,
            DecisionReason::MemoryPressure
        );
    }
    assert_eq!(counts(&c), (0, 0));
    let before: i64 = c
        .query_row("SELECT count(*) FROM policy_decisions", [], |r| r.get(0))
        .unwrap();
    for _ in 0..100 {
        p.set(MemoryPressure::Critical);
        assert!(matches!(poll_job(&mut c, 1, &p), Reply::Queued { .. }));
    }
    let after: i64 = c
        .query_row("SELECT count(*) FROM policy_decisions", [], |r| r.get(0))
        .unwrap();
    assert_eq!(before, after);
    p.set(MemoryPressure::Normal);
    assert!(matches!(
        poll_job(&mut c, 2, &p),
        Reply::Queued {
            reason: WaitReason::Fifo,
            ..
        }
    ));
    assert!(matches!(poll_job(&mut c, 1, &p), Reply::Granted { .. }));
    assert_eq!(counts(&c), (1, 1));
}
#[test]
fn critical_after_reservation_holds_launch_child_completion_cannot_release_queue() {
    let mut c = database();
    let p = Provider::new(MemoryPressure::Normal);
    let first_generation = generation(add(&mut c, 1, &p));
    p.set(MemoryPressure::Critical);
    assert!(matches!(
        launch_at(&mut c, 1, first_generation, &p).unwrap(),
        LaunchResult::Held { .. }
    ));
    assert!(matches!(add(&mut c, 2, &p), Reply::Queued { .. }));
    release(&mut c, 1, first_generation);
    assert_eq!(
        jobs::get(&c, &id(1)).unwrap().unwrap().outcome,
        Some(Outcome::Exited { code: 17 })
    );
    for _ in 0..10 {
        assert!(matches!(
            poll_job(&mut c, 2, &p),
            Reply::Queued {
                reason: WaitReason::MemoryBudget,
                ..
            }
        ));
    }
    assert_eq!(counts(&c), (1, 0));
    p.set(MemoryPressure::Normal);
    let held_generation = generation(poll_job(&mut c, 2, &p));
    assert!(matches!(
        launch_at(&mut c, 2, held_generation, &p).unwrap(),
        LaunchResult::Ready { .. }
    ));
    assert_eq!(
        launch_at(&mut c, 2, held_generation, &p),
        Err(jobs::JobError::InvalidTransition)
    );
    assert_eq!(counts(&c), (2, 1));
}
#[test]
fn queued_cancellation_and_lost_grant_cancellation_never_regrant() {
    let mut c = database();
    let p = Provider::new(MemoryPressure::Critical);
    add(&mut c, 1, &p);
    let cancel = JobRequest::Cancel {
        job_id: id(1),
        observer: owner(),
        permit_generation: None,
    };
    assert!(matches!(
        cancel_observed(&mut c, &cancel, &OwnerEvidence::gather(&owner()), 1001, &p).unwrap(),
        Reply::Cancelled {}
    ));
    assert_eq!(
        policy::job(&c, &id(1), p.policy(), 1001)
            .unwrap()
            .last_decision
            .unwrap()
            .kind,
        DecisionKind::Cancel
    );
    p.set(MemoryPressure::Normal);
    assert!(matches!(poll_job(&mut c, 1, &p), Reply::Cancelled {}));
    let generation = generation(add(&mut c, 2, &p));
    let cancel = JobRequest::Cancel {
        job_id: id(2),
        observer: owner(),
        permit_generation: Some(generation),
    };
    cancel_observed(&mut c, &cancel, &OwnerEvidence::gather(&owner()), 1001, &p).unwrap();
    assert!(matches!(
        launch_at(&mut c, 2, generation, &p).unwrap(),
        LaunchResult::Cancelled {}
    ));
    assert_eq!(counts(&c), (1, 0));
}
#[test]
fn sampler_fallbacks_release_a_stale_critical_hold_without_inventing_normal() {
    for mutate in [
        |s: &mut State| s.sample = None,
        |s: &mut State| s.status = SamplerStatus::Failed,
        |s: &mut State| s.status = SamplerStatus::Stopped,
        |s: &mut State| s.supported = false,
        |s: &mut State| s.time(4001),
        |s: &mut State| s.published.generation += 1,
        |s: &mut State| s.now = None,
        |s: &mut State| {
            s.sample.as_mut().unwrap().host.memory_pressure.reading = Reading::Denied {}
        },
    ] {
        let mut c = database();
        let p = Provider::new(MemoryPressure::Critical);
        add(&mut c, 1, &p);
        mutate(&mut p.0.lock().unwrap());
        let effective = p.policy();
        assert_eq!(effective.mode, PolicyMode::L0);
        assert_ne!(effective.observation.pressure, Some(MemoryPressure::Normal));
        assert!(matches!(poll_job(&mut c, 1, &p), Reply::Granted { .. }));
        let record = policy::job(&c, &id(1), effective.clone(), 1001)
            .unwrap()
            .last_decision
            .unwrap();
        assert_eq!(
            record.observation.availability,
            effective.observation.availability
        );
    }
}
#[test]
fn audit_failure_rolls_back_permit_queue_and_policy_state_together() {
    let mut c = database();
    let p = Provider::new(MemoryPressure::Normal);
    c.execute_batch("CREATE TEMP TRIGGER fail_audit BEFORE INSERT ON policy_decisions BEGIN SELECT RAISE(ABORT,'injected audit failure'); END;").unwrap();
    assert!(enqueue_observed(
        &mut c,
        &request(1, "CARGO_BUILD"),
        &OwnerEvidence::gather(&owner()),
        &Policy::default(),
        &context(1),
        None,
        1000,
        &p
    )
    .is_err());
    assert_eq!(counts(&c), (0, 0));
    for table in [
        "jobs",
        "traffic_entries",
        "policy_states",
        "policy_decisions",
    ] {
        assert_eq!(
            c.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
}
#[test]
fn claim_strings_and_failed_lineage_recheck_never_exempt_a_new_root() {
    let mut c = database();
    let p = Provider::new(MemoryPressure::Normal);
    let root = context(1);
    let generation = generation(add(&mut c, 1, &p));
    let snapshot = lineage_snapshot(&c, &root).unwrap().unwrap();
    let mut nested = request(2, "CARGO_BUILD");
    if let JobRequest::Enqueue {
        lineage_context, ..
    } = &mut nested
    {
        *lineage_context = Some(root.clone());
    }
    p.set(MemoryPressure::Critical);
    let evidence = OwnerEvidence::gather(&owner());
    let forged = LineageProof::verified_with_recheck(&snapshot, &owner(), |_, _| false);
    assert_eq!(
        enqueue_observed(
            &mut c,
            &nested,
            &evidence,
            &Policy::default(),
            &context(2),
            Some(&forged),
            1001,
            &p
        ),
        Err(jobs::JobError::IdentityUnavailable)
    );
    assert!(matches!(
        enqueue_observed(
            &mut c,
            &nested,
            &evidence,
            &Policy::default(),
            &context(2),
            None,
            1001,
            &p
        )
        .unwrap(),
        Reply::Queued { .. }
    ));
    let proof = LineageProof::verified_with_recheck(&snapshot, &owner(), |_, _| true); // explicitly simulated native ancestry
    assert!(matches!(
        poll_observed(
            &mut c,
            &JobRequest::Poll {
                job_id: id(2),
                observer: owner()
            },
            &evidence,
            &Policy::default(),
            &context(2),
            Some(&proof),
            1001,
            &p
        )
        .unwrap(),
        Reply::Reused { .. }
    ));
    let result = policy::transaction(&mut c, &p, |tx, effective| {
        launch(
            tx,
            &LaunchRequest {
                job_id: id(2),
                observer: owner(),
                permit_generation: generation,
            },
            &evidence,
            Some(&proof),
            effective,
            1001,
        )
    })
    .unwrap();
    assert!(matches!(result, LaunchResult::Reused { .. }));
    assert_eq!(counts(&c), (1, 1));
}
#[test]
fn remote_unknown_and_unsupported_do_not_create_queue_permits_or_pressure_holds() {
    let mut c = database();
    let p = Provider::new(MemoryPressure::Critical);
    for (n, class, locality, reason, uncertainty) in [
        (1, "REMOTE_EXEC", "REMOTE", "SSH_REMOTE_COMMAND", "NONE"),
        (2, "UNKNOWN", "UNKNOWN", "UNSUPPORTED_SYNTAX", "OBSERVE"),
        (3, "BUILD", "HOST_LOCAL", "COMPOUND_LOCAL", "NONE"),
    ] {
        let input=JobRequest::Enqueue{job_id:id(n),observer:owner(),lineage_context:None,
            classification:serde_json::from_value(serde_json::json!({"classification":class,"locality":locality,"reason":reason,"uncertainty":uncertainty})).unwrap()};
        assert!(matches!(
            enqueue_observed(
                &mut c,
                &input,
                &OwnerEvidence::gather(&owner()),
                &Policy::default(),
                &context(n),
                None,
                1000,
                &p
            )
            .unwrap(),
            Reply::Bypassed { .. }
        ));
        assert!(jobs::get(&c, &id(n)).unwrap().is_none());
    }
    assert_eq!(counts(&c), (0, 0));
}

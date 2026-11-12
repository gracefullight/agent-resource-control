use super::{admission::*, observation::*};
use arc_core::meter::MemoryPressure;
use arc_ipc::{
    jobs::Outcome,
    policy::launch::{LaunchRequest, LaunchResult},
    traffic::{JobRequest, JobResult},
};
use arc_policy::{DecisionKind, DecisionReason};
use arc_storage::{
    jobs::{self, traffic::*},
    policy,
};
use std::{
    io::Write,
    process::{Command, Stdio},
};

struct Child(std::process::Child);
impl Drop for Child {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn actual_running_child_keeps_its_observed_outcome_under_injected_critical() {
    let mut c = database();
    let p = Provider::new(MemoryPressure::Normal);
    let generation = generation(add(&mut c, 1, &p));
    let mut child = Child(
        Command::new("/bin/sh")
            .args(["-c", "read line; exit 17"])
            .stdin(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let identity = arc_core::process_identity::ProcessIdentity::capture(child.0.id()).unwrap();
    let attached = attach(
        &mut c,
        &JobRequest::Attach {
            job_id: id(1),
            observer: owner(),
            child: identity.clone(),
            permit_generation: generation,
        },
        &ChildEvidence::gather(&identity),
        1001,
    )
    .unwrap();
    assert!(matches!(attached, JobResult::Running {}));
    p.set(MemoryPressure::Critical);
    assert!(matches!(poll_job(&mut c, 1, &p), JobResult::Running {}));
    let recorded = policy::job(&c, &id(1), p.policy(), 1001)
        .unwrap()
        .last_decision
        .unwrap();
    assert_eq!(
        (recorded.kind, recorded.reason),
        (DecisionKind::Preserve, DecisionReason::Running)
    );
    assert!(matches!(add(&mut c, 2, &p), JobResult::Queued { .. }));
    child
        .0
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"finish\n")
        .unwrap();
    let status = child.0.wait().unwrap();
    assert_eq!(status.code(), Some(17));
    complete(
        &mut c,
        &JobRequest::Complete {
            job_id: id(1),
            observer: owner(),
            permit_generation: generation,
            outcome: Outcome::Exited {
                code: u8::try_from(status.code().unwrap()).unwrap(),
            },
        },
        1002,
    )
    .unwrap();
    assert_eq!(
        jobs::get(&c, &id(1)).unwrap().unwrap().outcome,
        Some(Outcome::Exited { code: 17 })
    );
    assert!(matches!(poll_job(&mut c, 2, &p), JobResult::Queued { .. }));
    assert_eq!(counts(&c), (1, 0));
}
#[test]
fn launch_rechecks_owner_generation_state_and_persistent_single_release() {
    let mut c = database();
    let p = Provider::new(MemoryPressure::Normal);
    let generation = generation(add(&mut c, 1, &p));
    assert_eq!(
        launch_at(&mut c, 1, generation + 1, &p),
        Err(jobs::JobError::InvalidTransition)
    );
    assert!(matches!(
        launch_at(&mut c, 999, 1, &p).unwrap(),
        LaunchResult::NotFound {}
    ));
    let mut wrong = owner();
    wrong.start_token.push('1');
    let result = policy::transaction(&mut c, &p, |tx, effective| {
        launch(
            tx,
            &LaunchRequest {
                job_id: id(1),
                observer: wrong.clone(),
                permit_generation: generation,
            },
            &OwnerEvidence::gather(&wrong),
            None,
            effective,
            1001,
        )
    });
    assert_eq!(result, Err(jobs::JobError::Conflict));
    assert!(matches!(
        launch_at(&mut c, 1, generation, &p).unwrap(),
        LaunchResult::Ready { .. }
    ));
    release(&mut c, 1, generation);
    assert_eq!(
        launch_at(&mut c, 1, generation, &p),
        Err(jobs::JobError::InvalidTransition)
    );
}
#[test]
fn policy_repository_rejects_nontransactional_recording_and_bounds_legacy_reads() {
    let c = database();
    let p = Provider::new(MemoryPressure::Normal);
    assert_eq!(
        policy::record_effective(&c, &p.policy(), 1000),
        Err(jobs::JobError::InvalidTransition)
    );
    c.execute("INSERT INTO jobs(id,job_class,locality,enforcement,state,created_at) VALUES(?1,'UNKNOWN','UNKNOWN','OBSERVE','OUTCOME_UNKNOWN',1000)",[id(1).as_str()]).unwrap();
    let read = policy::job(&c, &id(1), p.policy(), 1000).unwrap();
    assert_eq!(
        read.decision_availability,
        arc_ipc::policy::DecisionAvailability::NotRecorded
    );
    assert!(read.last_decision.is_none());
}

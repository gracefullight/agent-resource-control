use super::*;
use arc_ipc::{
    jobs::{JobId, Outcome, ProcessIdentity},
    traffic::{JobRequest, JobResult, StatusResult},
};

fn status(f: &Fixture, id: &JobId) -> StatusResult {
    let response = traffic(f, &JobRequest::StatusV2 { job_id: id.clone() });
    assert_eq!(response.status, arc_ipc::STATUS_OK, "{response:?}");
    serde_json::from_value(response.result.unwrap()).unwrap()
}

#[test]
fn status_reconciles_a_dead_queued_owner_without_an_admission() {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let projects = projects(&f.root, "status_dead_queue");
    let mut first = spawn_build(&f, &projects[0], true);
    eventually(|| projects[0].join("started").exists());
    let mut second = spawn_build(&f, &projects[1], true);
    eventually(|| entry(&f, second.child.id()).is_some_and(|e| e.0 == "QUEUED"));
    let id = JobId::try_from(
        database(&f)
            .query_row(
                "SELECT job_id FROM job_lifecycle WHERE observer_pid=?1",
                [second.child.id()],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
    )
    .unwrap();
    second.child.kill().unwrap();
    second.child.wait().unwrap();
    assert!(matches!(status(&f, &id), StatusResult::Abandoned {}));
    assert_eq!(active(&f).1, 1);
    assert!(!projects[1].join("started").exists());
    fs::write(projects[0].join("release"), "release").unwrap();
    finish(&mut first);
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn status_reports_unattached_grant_as_unconfirmed_and_keeps_the_permit() {
    #[derive(serde::Deserialize)]
    #[serde(tag = "state", deny_unknown_fields)]
    enum OldTrafficStatus {
        #[serde(rename = "RESERVED")]
        Reserved {
            sequence: u64,
            queued_at_ms: u64,
            granted_at_ms: u64,
            reservation: arc_ipc::traffic::Resources,
        },
    }
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let id = JobId::try_from("f123456789abcdef0123456789abcdef".to_owned()).unwrap();
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let enqueue = JobRequest::Enqueue {
        job_id: id.clone(),
        observer: observer.clone(),
        classification: cargo_class("BUILD", "CARGO_BUILD"),
        lineage_context: None,
    };
    let JobResult::Granted {
        permit_generation, ..
    } = traffic_result(traffic(&f, &enqueue))
    else {
        panic!("expected grant")
    };
    assert!(matches!(status(&f, &id), StatusResult::Unconfirmed {}));
    assert_eq!(active(&f).1, 1);
    let legacy = traffic(&f, &JobRequest::Status { job_id: id.clone() });
    let OldTrafficStatus::Reserved {
        sequence,
        queued_at_ms,
        granted_at_ms,
        reservation,
    } = serde_json::from_value(legacy.result.unwrap()).unwrap();
    assert!(sequence > 0 && granted_at_ms >= queued_at_ms);
    assert!(reservation.cpu_millis > 0 && reservation.memory_bytes > 0);
    assert_eq!(active(&f).1, 1);
    let cancel = JobRequest::Cancel {
        job_id: id,
        observer,
        permit_generation: Some(permit_generation),
    };
    assert!(matches!(
        traffic_result(traffic(&f, &cancel)),
        JobResult::Cancelled {}
    ));
    assert_eq!(active(&f).1, 0);
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn status_recovers_exited_owner_and_child_without_an_admission() {
    use arc_core::process_identity::{probe_live, IdentityStatus};
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let project = projects(&f.root, "status_recovery");
    let mut wrapper = spawn_build(&f, &project[0], true);
    eventually(|| {
        project[0].join("started").exists()
            && entry(&f, wrapper.child.id()).is_some_and(|e| e.1.is_some())
    });
    let child =
        ProcessIdentity::capture(entry(&f, wrapper.child.id()).unwrap().1.unwrap()).unwrap();
    let id = JobId::try_from(
        database(&f)
            .query_row(
                "SELECT job_id FROM job_lifecycle WHERE observer_pid=?1",
                [wrapper.child.id()],
                |row| row.get::<_, String>(0),
            )
            .unwrap(),
    )
    .unwrap();
    wrapper.child.kill().unwrap();
    wrapper.child.wait().unwrap();
    assert_eq!(probe_live(&child), IdentityStatus::VerifiedMatch);
    assert!(matches!(status(&f, &id), StatusResult::Running { .. }));
    assert_eq!(active(&f).1, 1);
    fs::write(project[0].join("release"), "release").unwrap();
    eventually(|| {
        matches!(
            probe_live(&child),
            IdentityStatus::Missing | IdentityStatus::Mismatch
        )
    });
    assert!(matches!(status(&f, &id), StatusResult::Abandoned {}));
    assert_eq!(active(&f).1, 0);
    assert_eq!(invocations(&project[0]), 1);
    let output = Command::new(binary("arcctl"))
        .args(["job", "status", id.as_str()])
        .env(arc_core::ARC_SOCKET_PATH, &f.socket)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("State: outcome unknown"));
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn status_identifies_a_retained_live_child_as_running() {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let id = JobId::try_from("e123456789abcdef0123456789abcdef".to_owned()).unwrap();
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let enqueue = JobRequest::Enqueue {
        job_id: id.clone(),
        observer: observer.clone(),
        classification: cargo_class("BUILD", "CARGO_BUILD"),
        lineage_context: None,
    };
    let JobResult::Granted {
        permit_generation, ..
    } = traffic_result(traffic(&f, &enqueue))
    else {
        panic!("expected grant")
    };
    let mut child = OwnedChild::new(Command::new("/bin/sleep").arg("30").spawn().unwrap());
    let child_identity = ProcessIdentity::capture(child.child.id()).unwrap();
    let fallback = JobRequest::Fallback {
        job_id: id.clone(),
        observer: observer.clone(),
        permit_generation: None,
        child: Some(child_identity),
        outcome: None,
    };
    assert_eq!(
        traffic_result(traffic(&f, &fallback)),
        JobResult::ReservationRetained {}
    );
    assert!(matches!(status(&f, &id), StatusResult::Running { .. }));
    assert_eq!(active(&f).1, 1);
    child.child.kill().unwrap();
    child.child.wait().unwrap();
    let complete = JobRequest::Complete {
        job_id: id.clone(),
        observer,
        permit_generation,
        outcome: Outcome::Signaled {
            signal: libc::SIGKILL as u8,
            core_dumped: false,
        },
    };
    assert!(matches!(
        traffic_result(traffic(&f, &complete)),
        JobResult::Completed { .. }
    ));
    assert_eq!(active(&f).1, 0);
    daemon.stop(libc::SIGTERM);
    f.report();
}

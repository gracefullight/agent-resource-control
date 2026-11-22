use super::{classification, fixture, protocol, run, support, write, Fixture};
use arc_ipc::{
    governor::{GovernorJobRequest, GovernorJobResult, OPERATION_JOB},
    jobs::{JobId, ProcessIdentity},
    traffic::{JobRequest, JobResult},
};
use serde_json::json;

#[test]
fn selected_remote_buildx_bypasses_held_build_permit_and_runs_once() {
    let _serial = support::serial();
    let f = Fixture::new();
    fixture(&f);
    // Buildx current selection applies to the active Docker context.
    write(
        &f.root.join(".docker/buildx/current"),
        json!({"Key":"remote","Name":"remote-builder","Global":false}).to_string(),
    );
    let mut daemon = f.spawn("test");
    let id = JobId::try_from("e".repeat(32)).unwrap();
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let local = arc_ipc::Classification {
        classification: arc_ipc::JobClass::Build,
        locality: arc_ipc::Locality::HostLocal,
        reason: arc_ipc::Reason::CargoBuild,
        uncertainty: arc_ipc::Uncertainty::None,
    };
    let request = GovernorJobRequest::Traffic {
        request: JobRequest::Enqueue {
            job_id: id.clone(),
            observer: observer.clone(),
            classification: local,
            lineage_context: None,
            project: None,
            event: None,
            usage: None,
        },
    };
    let grant = protocol::call(&f, OPERATION_JOB, json!(request));
    let GovernorJobResult::Traffic {
        result: JobResult::Granted {
            permit_generation, ..
        },
    } = serde_json::from_value(grant.result.unwrap()).unwrap()
    else {
        panic!("Build permit not granted");
    };
    let probe_id = JobId::try_from("f".repeat(32)).unwrap();
    let probe = GovernorJobRequest::Traffic {
        request: JobRequest::Enqueue {
            job_id: probe_id.clone(),
            observer: observer.clone(),
            classification: local,
            lineage_context: None,
            project: None,
            event: None,
            usage: None,
        },
    };
    let queued = protocol::call(&f, OPERATION_JOB, json!(probe));
    assert!(matches!(
        serde_json::from_value(queued.result.unwrap()),
        Ok(GovernorJobResult::Traffic {
            result: JobResult::Queued { .. }
        })
    ));
    let cancel_probe = GovernorJobRequest::Traffic {
        request: JobRequest::Cancel {
            job_id: probe_id,
            observer: observer.clone(),
            permit_generation: None,
        },
    };
    let cancelled = protocol::call(&f, OPERATION_JOB, json!(cancel_probe));
    assert!(matches!(
        serde_json::from_value(cancelled.result.unwrap()),
        Ok(GovernorJobResult::Traffic {
            result: JobResult::Cancelled {}
        })
    ));
    let db = rusqlite::Connection::open(&f.database).unwrap();
    let counts = || {
        db.query_row("SELECT (SELECT count(*) FROM traffic_entries), (SELECT count(*) FROM permits), (SELECT count(*) FROM jobs WHERE queue_at IS NOT NULL OR reserved_at IS NOT NULL)", [], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, i64>(2)?))).unwrap()
    };
    let before = counts();
    assert_eq!(before.1, 1, "Build permit must be held");
    let marker = f.root.join("buildx-call");
    let (job, _) = run(&f, "docker", &["buildx", "build", "."], &[], &marker);
    classification(&job, "REMOTE_EXEC");
    assert_eq!(job["classification"]["reason"], "REMOTE_ENDPOINT_EXEC");
    assert_eq!(job["outcome"]["code"], 17);
    assert!(job["child"].is_object());
    assert_eq!(
        counts(),
        before,
        "remote child created a queue entry or reservation"
    );
    drop(db);
    let cancel = GovernorJobRequest::Traffic {
        request: JobRequest::Cancel {
            job_id: id,
            observer,
            permit_generation: Some(permit_generation),
        },
    };
    let result = protocol::call(&f, OPERATION_JOB, json!(cancel));
    assert!(matches!(
        serde_json::from_value(result.result.unwrap()),
        Ok(GovernorJobResult::Traffic {
            result: JobResult::Cancelled {}
        })
    ));
    daemon.stop(libc::SIGTERM);
    f.report();
}

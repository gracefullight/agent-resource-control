//! What each reader of a stored Local Container record receives, and what
//! `arcctl classify` shows. Only a negotiated operation names the location.
use super::*;
use arc_ipc::{
    compute_location::{carries_container, OPERATION_CLASSIFY as NEGOTIATED_CLASSIFY},
    governor::GovernorJobRequest,
    jobs::{StatusResult, OPERATION_JOB_STATUS},
    traffic::{diagnostics, JobRequest, LineageContext},
    Classification,
};

fn ok(f: &Fixture, operation: &str, payload: Value) -> Value {
    let response = protocol::call(f, operation, payload);
    assert_eq!(response.status, "ok", "{operation}: {response:?}");
    response.result.unwrap()
}

fn random_id() -> String {
    arc_core::process_identity::random_job_id().unwrap()
}

pub(super) fn local_container_projection_for_earlier_peers() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    let engine = LocalEngine::proven(&f, &s);
    let env = engine.env();
    let mut daemon = f.spawn_isolated("test");

    // The record under test is stored by a build on the production route.
    let (job, id) = run(&s, &f, Route::Exec, &BUILDX, &env);
    let (container, projected) = (
        container_record("LOCAL_CONTAINER"),
        container_record("HOST_LOCAL"),
    );
    assert_eq!(job["classification"], container);
    assert_eq!(stored(&f, &id).1, "LOCAL_CONTAINER");
    let by_id = json!(StatusRequest::ById { job_id: id.clone() });
    let recent = json!(StatusRequest::Recent {});

    // Protocol-1 peer, which is also the desktop bridge's job reader: its own
    // vocabulary, decodable by its typed reader.
    let reply = ok(&f, OPERATION_JOB_STATUS, by_id.clone());
    assert!(!carries_container(&reply), "{reply}");
    let StatusResult::Found { job: legacy } = serde_json::from_value(reply).unwrap() else {
        panic!("protocol-1 status lost the job");
    };
    assert_eq!(legacy.classification, legacy.classification.legacy());
    assert_eq!(
        json!(legacy.classification),
        json!({"classification":"UNKNOWN","locality":"UNKNOWN",
            "reason":"UNRESOLVED_COMMAND","uncertainty":"OBSERVE"})
    );
    // Locality readers that predate the location: the HOST_LOCAL projection.
    let reply = ok(&f, OPERATION_JOB_STATUS_LOCALITY, by_id.clone());
    assert!(!carries_container(&reply), "{reply}");
    assert_eq!(reply["job"]["classification"], projected);
    let StatusResult::Found { job: earlier } = serde_json::from_value(reply).unwrap() else {
        panic!("locality status lost the job");
    };
    assert!(earlier.classification.is_consistent());
    for operation in [OPERATION_JOB_STATUS, OPERATION_JOB_STATUS_LOCALITY] {
        let reply = ok(&f, operation, recent.clone());
        assert!(!carries_container(&reply), "{operation}: {reply}");
        let StatusResult::Recent { jobs, .. } = serde_json::from_value(reply).unwrap() else {
            panic!("{operation}: recent listing");
        };
        assert_eq!(jobs[0].job_id, id, "{operation}");
    }
    // Traffic diagnostics, the desktop's admission reader, on every generation
    // of that operation: a valid local Build with its recorded reservation.
    for operation in [
        diagnostics::OPERATION_JOB,
        diagnostics::OPERATION_PYTHON_JOB,
        arc_ipc::traffic::native::OPERATION_DIAGNOSTICS_JOB,
    ] {
        let reply = ok(&f, operation, json!({ "job_id": id }));
        assert!(!carries_container(&reply), "{operation}: {reply}");
        assert_eq!(reply["classification"], projected, "{operation}");
        let typed = diagnostics::decode_result(&reply, &id).expect(operation);
        let admission = typed.admission.expect("recorded admission");
        assert!(admission.reservation.is_some() && !admission.permit_active);
    }
    // Only the negotiated reader names the location.
    assert!(carries_container(&ok(&f, NEGOTIATED_STATUS, by_id)));
    assert_eq!(recorded(&f, NEGOTIATED_STATUS, &id), container);

    // The same decision for a peer that classifies. Injected facts prove the
    // daemon's reply vocabulary only; the capture itself is proven above.
    let context = json!({
        "captured_at_ms": arc_ipc::locality::now_ms().unwrap(),
        "environment": {},
        "facts": [
            {"kind":"DOCKER_CONTEXT","selector":"@current","endpoints":[engine.host]},
            {"kind":"DOCKER_BUILDER","selector":"@current","endpoints":[engine.host]},
            {"kind":"DOCKER_CONTEXT","selector":"@local-proof/0","endpoints":[engine.host]},
        ],
    });
    let request = json!({"command":{"kind":"ARGV","argv":BUILD},"context":context});
    let legacy = ok(
        &f,
        arc_ipc::OPERATION_JOB_CLASSIFY,
        json!({"command":"docker build .","shell_kind":"sh"}),
    );
    let legacy: Classification = serde_json::from_value(legacy).unwrap();
    assert_eq!(legacy, legacy.legacy());
    assert_eq!(legacy.classification, arc_ipc::JobClass::Unknown);
    assert_eq!(
        ok(&f, arc_ipc::locality::OPERATION_CLASSIFY, request.clone()),
        projected
    );
    assert_eq!(ok(&f, NEGOTIATED_CLASSIFY, request), container);

    // An earlier arcctl sends BUILD/HOST_LOCAL/LOCAL_ENDPOINT_BUILD. It is still
    // accepted and admitted, stored canonically and answered in its vocabulary.
    let earlier = JobId::try_from(random_id()).unwrap();
    let observer = protocol::observer();
    let enqueue = GovernorJobRequest::Traffic {
        request: JobRequest::Enqueue {
            job_id: earlier.clone(),
            observer: observer.clone(),
            classification: serde_json::from_value(projected.clone()).unwrap(),
            lineage_context: Some(
                LineageContext::try_from(format!("{}{}", random_id(), random_id())).unwrap(),
            ),
        },
    };
    let granted = ok(
        &f,
        arc_ipc::traffic::endpoint::OPERATION_JOB,
        json!(enqueue),
    );
    assert!(!carries_container(&granted), "{granted}");
    assert_eq!(granted["result"]["state"], "GRANTED", "{granted}");
    assert!(granted["result"]["reservation"]["cpu_millis"].as_u64() > Some(0));
    assert_eq!(admission(&f, Some(&earlier)), (1, 0, 1, 1));
    assert_eq!(stored(&f, &earlier).1, "LOCAL_CONTAINER");
    assert_eq!(
        recorded(&f, OPERATION_JOB_STATUS_LOCALITY, &earlier),
        projected
    );
    assert_eq!(recorded(&f, NEGOTIATED_STATUS, &earlier), container);
    let cancel = GovernorJobRequest::Traffic {
        request: JobRequest::Cancel {
            job_id: earlier.clone(),
            observer,
            permit_generation: granted["result"]["permit_generation"].as_u64(),
        },
    };
    let cancelled = ok(&f, arc_ipc::traffic::endpoint::OPERATION_JOB, json!(cancel));
    assert!(!carries_container(&cancelled), "{cancelled}");
    assert_eq!(admission(&f, Some(&earlier)).3, 0, "{cancelled}");

    assert_eq!(s.calls(), 1);
    assert_eq!(engine.connections(), 0);
    daemon.stop(libc::SIGTERM);
    f.report();
}

pub(super) fn local_container_classify_contacts_no_socket() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    let engine = LocalEngine::proven(&f, &s);
    let env = engine.env();
    let mut daemon = f.spawn_isolated("test");
    for argv in [&BUILDX[..], &BUILD[..]] {
        // `Sandbox::classify` also asserts that no offline stub was started.
        let inspection = s.classify(&f, argv, &env);
        assert_eq!(inspection, container_record("LOCAL_CONTAINER"), "{argv:?}");
        let mut text = s.command(&f, &env);
        text.args(["classify", "--", &argv.join(" ")]);
        let output = child_watchdog::output(&mut text, "classify text").unwrap();
        assert!(output.status.success(), "{argv:?}: {output:?}");
        assert!(output.stderr.is_empty(), "{argv:?}: {output:?}");
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            "Classification: BUILD\nLocality: LOCAL_CONTAINER\nReason: LOCAL_ENDPOINT_BUILD\n\
             Uncertainty: NONE\nCommand not executed.\n",
            "{argv:?}"
        );
    }
    // Nothing ran, connected, registered, queued or reserved.
    assert_eq!(s.calls(), 0);
    assert_eq!(engine.connections(), 0);
    assert_eq!(admission(&f, None), (0, 0, 0, 0));
    let db = rusqlite::Connection::open(&f.database).unwrap();
    let jobs: i64 = db
        .query_row("SELECT count(*) FROM jobs", [], |row| row.get(0))
        .unwrap();
    assert_eq!(jobs, 0, "classification registered a job");
    drop(db);
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn registered_reads_have_exact_types_and_privacy_projection() {
    let mut cases = serde_json::Map::new();
    let peer = Peer::new(1, |stream, request| {
        assert_eq!(request.protocol_version, 1);
        assert_eq!(request.operation, arc_ipc::OPERATION_HEALTH);
        assert_eq!(request.payload, Value::Null);
        arc_ipc::write_response(stream, &Response::ok(request.request_id, health())).unwrap();
    });
    let app = App::new(peer.bridge());
    let reply = app.invoke("daemon_health", json!({})).unwrap();
    assert_eq!(reply, json!({"data":health(),"received_at_ms":RECEIVED}));
    cases.insert("health".into(), reply);

    let peer = Peer::new(1, |stream, request| {
        assert_eq!(request.operation, OPERATION_JOB_STATUS);
        assert_eq!(request.payload, json!({"query":"RECENT"}));
        arc_ipc::write_response(stream, &Response::ok(request.request_id, recent())).unwrap();
    });
    let app = App::new(peer.bridge());
    let reply = app.invoke("recent_jobs", json!({})).unwrap();
    assert_eq!(
        reply,
        json!({"data":{"jobs":[summary()],"truncated":false,"observed_at_ms":160,"liveness_fresh":false,"limit":50},"received_at_ms":RECEIVED})
    );
    cases.insert("recent".into(), reply);

    for code in [0, 17] {
        let peer = Peer::new(1, move |stream, request| {
            assert_eq!(request.operation, OPERATION_JOB_STATUS);
            assert_eq!(request.payload, json!({"query":"BY_ID","job_id":JOB}));
            arc_ipc::write_response(
                stream,
                &Response::ok(request.request_id, found(snapshot(code))),
            )
            .unwrap();
        });
        let reply = App::new(peer.bridge())
            .invoke("job_by_id", json!({"job_id":JOB}))
            .unwrap();
        assert_eq!(
            reply,
            json!({"data":projected(code),"received_at_ms":RECEIVED})
        );
        let encoded = reply.to_string();
        for private in [
            "observer",
            "child_verified_at_ms",
            "classification",
            "birth",
            "argv",
            "command",
            "cwd",
        ] {
            assert!(!encoded.contains(private));
        }
        cases.insert(format!("exit{code}"), reply);
    }
    let mut unknown = snapshot(0);
    unknown.phase = Phase::OutcomeUnknown;
    unknown.reason = LifecycleReason::IdentityUnavailable;
    unknown.observation.reason = unknown.reason;
    unknown.observation.status = ObservationStatus::Indeterminate;
    unknown.completed_at_ms = None;
    unknown.outcome = None;
    let peer = Peer::result(found(unknown));
    let reply = App::new(peer.bridge())
        .invoke("job_by_id", json!({"job_id":JOB}))
        .unwrap();
    assert_eq!(reply["data"]["job"]["outcome"], Value::Null);
    assert_eq!(reply["data"]["job"]["phase"], "OUTCOME_UNKNOWN");
    cases.insert("unknown".into(), reply);
    let peer = Peer::result(json!({"kind":"NOT_FOUND"}));
    let reply = App::new(peer.bridge())
        .invoke("job_by_id", json!({"job_id":JOB}))
        .unwrap();
    assert_eq!(
        reply,
        json!({"data":{"kind":"not_found"},"received_at_ms":RECEIVED})
    );
    cases.insert("notFound".into(), reply);
    let app = App::new(DesktopBridge::from_socket_path(
        "/missing/arc-desktop-fixture.sock".into(),
    ));
    let metadata = app.invoke("desktop_metadata", json!({})).unwrap();
    assert_eq!(metadata["product_name"], "Agent Resource Control");
    assert_eq!(metadata["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(metadata["os"], std::env::consts::OS);
    assert_eq!(metadata["architecture"], std::env::consts::ARCH);
    assert_eq!(metadata["license"], Value::Null);
    assert!(metadata["open_source_notices"]
        .as_str()
        .unwrap()
        .contains("Rust tauri@2.12.5"));
    if let Some(commit) = metadata["build_commit"].as_str() {
        assert!(matches!(commit.len(), 40 | 64));
    }
    cases.insert("metadata".into(), metadata);
    cases.insert(
        "error".into(),
        app.invoke("daemon_health", json!({})).unwrap_err(),
    );
    // Optional receipt output never replaces assertions. The interface owner can
    // bind these handler-produced bytes as its shared golden fixture input.
    if let Some(path) = std::env::var_os("ARC_DESKTOP_TEST_ARTIFACT_DIR") {
        let path = PathBuf::from(path);
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(
            path.join("registered-wire-cases.json"),
            serde_json::to_vec_pretty(&cases).unwrap(),
        )
        .unwrap();
    }
}

#[test]
fn registered_meter_preserves_partial_failures_and_working_legacy_reads() {
    // Synthetic per-field failure projection; not native calibration evidence.
    let peer = Peer::new(3, |stream, request| {
        let value = match request.operation.as_str() {
            arc_ipc::meter::OPERATION_METER_SNAPSHOT => {
                assert!(request.payload.is_null());
                let mut meter: Value = serde_json::from_str(include_str!(
                    "../../../../../crates/arc-core/tests/fixtures/meter-snapshot.json"
                ))
                .unwrap();
                meter["host"]["memory_available_bytes"]["reading"] =
                    json!({"availability":"denied"});
                meter["host"]["swap_used_bytes"]["reading"] =
                    json!({"availability":"failed","diagnostic":"injected_native_failure"});
                meter["host"]["filesystem"]["identity"] = json!({"path":"/","mount_point":"/","filesystem_type":"apfs","volume_id":"synthetic-volume","is_local":true});
                for (name, value) in [
                    ("capacity_bytes", 512_000),
                    ("free_bytes", 256_000),
                    ("available_bytes", 128_000),
                ] {
                    meter["host"]["filesystem"][name]["reading"] =
                        json!({"availability":"available","value":value});
                }
                meter["host"]["filesystem"]["pressure"]["reading"] =
                    json!({"availability":"available","value":"normal"});
                meter
            }
            arc_ipc::OPERATION_HEALTH => health(),
            OPERATION_JOB_STATUS => recent(),
            _ => panic!("unexpected registered operation"),
        };
        arc_ipc::write_response(stream, &Response::ok(request.request_id, value)).unwrap();
    });
    let app = App::new(peer.bridge());
    let reply = app.invoke("meter_snapshot", json!({})).unwrap();
    assert_eq!(
        reply["data"]["host"]["memory_available_bytes"]["reading"],
        json!({"availability":"denied"})
    );
    assert_eq!(
        reply["data"]["host"]["swap_used_bytes"]["reading"]["availability"],
        "failed"
    );
    assert!(
        reply["data"]["host"]["filesystem"]["free_bytes"]["reading"]["value"]
            .as_u64()
            .is_some()
    );
    assert!(reply["data"]["daemon"]["rss_bytes"]["reading"]["value"]
        .as_u64()
        .is_some());
    assert_eq!(
        app.invoke("daemon_health", json!({})).unwrap()["data"]["protocol_version"],
        1
    );
    assert_eq!(
        app.invoke("recent_jobs", json!({})).unwrap()["data"]["jobs"][0]["job_id"],
        JOB
    );
}

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

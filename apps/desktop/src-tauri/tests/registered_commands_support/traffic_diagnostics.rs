// The production router is exercised with authenticated socket fixtures here.
// Production-daemon/SQLite/native-tool reconciliation remains task-9 evidence.
fn traffic_fixtures() -> Value {
    serde_json::from_str(include_str!(
        "../../../tests/fixtures/traffic-scripts-wire.json"
    ))
    .unwrap()
}
fn traffic_peer(result: Value) -> Peer {
    use arc_ipc::traffic::diagnostics as d;
    Peer::new(2, move |stream, request| {
        assert_eq!(request.protocol_version, 1);
        let result = match request.operation.as_str() {
            d::OPERATION_PYTHON_CAPABILITIES => {
                assert!(request.payload.is_null());
                json!({"capability": d::PYTHON_CAPABILITY})
            }
            d::OPERATION_PYTHON_JOB => {
                assert_eq!(
                    d::decode_request(&request.payload).unwrap().job_id.as_str(),
                    JOB
                );
                result.clone()
            }
            _ => panic!("unexpected traffic read operation"),
        };
        arc_ipc::write_response(stream, &Response::ok(request.request_id, result)).unwrap();
    })
}

#[test]
fn registered_traffic_diagnostics_preserve_rust_facts_without_changing_legacy_dtos() {
    let fixtures = traffic_fixtures();
    for result in fixtures["results"].as_array().unwrap() {
        let peer = traffic_peer(result.clone());
        let actual = App::new(peer.bridge())
            .invoke("traffic_job_diagnostics", json!({"job_id":JOB}))
            .unwrap();
        assert_eq!(actual, json!({"data":result,"received_at_ms":RECEIVED}));
        assert!(arc_ipc::traffic::diagnostics::decode_result(
            &actual["data"],
            &JobId::try_from(JOB.to_owned()).unwrap()
        )
        .is_ok());
        for private in ["argv", "command", "cwd", "observer", "lineage", SENTINEL] {
            assert!(!actual.to_string().contains(private));
        }
        assert_eq!(peer.requests.load(Ordering::Relaxed), 2);
    }
    // New-class facts never get fabricated from the legacy job projection.
    assert!(projected(17)["job"].get("classification").is_none());
}

#[test]
fn registered_python_diagnostics_fall_back_only_to_negotiated_legacy_facts() {
    use arc_ipc::traffic::diagnostics as d;
    let fixtures = traffic_fixtures();
    let mut python = fixtures["results"][0].clone();
    python["classification"] = json!({
        "classification":"TEST", "locality":"HOST_LOCAL",
        "reason":"PYTHON_PYTEST", "uncertainty":"NONE"
    });
    let id = JobId::try_from(JOB.to_owned()).unwrap();
    let legacy = serde_json::to_value(d::decode_result(&python, &id).unwrap().legacy()).unwrap();
    for result in [fixtures["results"][0].clone(), legacy.clone(), python.clone()] {
        let reply = result.clone();
        let peer = Peer::new(3, move |stream, request| {
            assert_eq!(request.protocol_version, 1);
            let response = match request.operation.as_str() {
                d::OPERATION_PYTHON_CAPABILITIES => {
                    assert!(request.payload.is_null());
                    Response::error(request.request_id, arc_ipc::ERROR_UNSUPPORTED_CAPABILITY)
                }
                d::OPERATION_CAPABILITIES => {
                    assert!(request.payload.is_null());
                    Response::ok(request.request_id, json!({"capability":d::CAPABILITY}))
                }
                d::OPERATION_JOB => {
                    assert_eq!(request.payload, json!({"job_id":JOB}));
                    Response::ok(request.request_id, reply.clone())
                }
                _ => panic!("unsupported Python capability must not query Python or write"),
            };
            arc_ipc::write_response(stream, &response).unwrap();
        });
        let actual = App::new(peer.bridge()).invoke("traffic_job_diagnostics", json!({"job_id":JOB}));
        if result == python {
            assert_eq!(actual, Err(error("invalid_reply", false)));
        } else {
            assert_eq!(actual.unwrap()["data"], result);
        }
        assert_eq!(peer.requests.load(Ordering::Relaxed), 3);
    }
    assert_eq!(legacy["classification"]["classification"], "UNKNOWN");
    assert_eq!(legacy["classification"]["reason"], "UNRESOLVED_COMMAND");
    assert!(legacy["admission"].is_null());
}

#[test]
fn registered_traffic_diagnostics_validate_ids_before_io_and_never_proxy_writes() {
    let peer = Peer::new(0, |_, _| panic!("invalid traffic input performed I/O"));
    let app = App::new(peer.bridge());
    for id in [
        String::new(),
        "a".repeat(31),
        "a".repeat(33),
        "A".repeat(32),
        "x".repeat(1_048_577),
    ] {
        assert_eq!(
            app.invoke("traffic_job_diagnostics", json!({"job_id":id})),
            Err(error("invalid_request", false))
        );
    }
    for body in [
        json!({}),
        json!({"jobId":JOB}),
        json!({"job_id":[SENTINEL]}),
    ] {
        assert!(app.invoke("traffic_job_diagnostics", body).is_err());
    }
    assert!(app
        .invoke("traffic_job", json!({"action":"ENQUEUE"}))
        .is_err());
    assert_eq!(peer.requests.load(Ordering::Relaxed), 0);
}

#[test]
fn registered_traffic_diagnostics_require_the_exact_independent_capability() {
    use arc_ipc::traffic::diagnostics as d;
    for caps in [
        Value::Null,
        json!([]),
        json!([d::PYTHON_CAPABILITY]),
        json!({"capability":"traffic.v1"}),
        json!({"capability":"traffic.bun-scripts.v1"}),
        json!({"capability":d::CAPABILITY}),
        json!({"capability":d::PYTHON_CAPABILITY,"extra":SENTINEL}),
        json!({"capability":"x".repeat(513)}),
    ] {
        let peer = Peer::new(1, move |stream, request| {
            assert_eq!(request.operation, d::OPERATION_PYTHON_CAPABILITIES);
            arc_ipc::write_response(stream, &Response::ok(request.request_id, caps.clone()))
                .unwrap();
        });
        assert_eq!(
            App::new(peer.bridge()).invoke("traffic_job_diagnostics", json!({"job_id":JOB})),
            Err(error("invalid_reply", false))
        );
        assert_eq!(peer.requests.load(Ordering::Relaxed), 1);
    }
}

#[test]
fn registered_traffic_diagnostics_keep_old_and_replaced_peers_explicitly_unavailable() {
    use arc_ipc::traffic::diagnostics as d;
    for replacement in [false, true] {
        let peer = Peer::new(3, move |stream, request| {
            let response = match request.operation.as_str() {
                d::OPERATION_PYTHON_CAPABILITIES if replacement => {
                    Response::ok(request.request_id, json!({"capability":d::PYTHON_CAPABILITY}))
                }
                d::OPERATION_PYTHON_CAPABILITIES | d::OPERATION_CAPABILITIES | d::OPERATION_PYTHON_JOB => {
                    Response::error(request.request_id, arc_ipc::ERROR_UNSUPPORTED_CAPABILITY)
                }
                arc_ipc::OPERATION_HEALTH => Response::ok(request.request_id, health()),
                _ => panic!("unexpected fallback or write"),
            };
            arc_ipc::write_response(stream, &response).unwrap();
        });
        let app = App::new(peer.bridge());
        assert_eq!(
            app.invoke("traffic_job_diagnostics", json!({"job_id":JOB})),
            Err(error("unsupported", false))
        );
        assert_eq!(
            app.invoke("daemon_health", json!({})).unwrap()["data"],
            health()
        );
        assert_eq!(
            peer.requests.load(Ordering::Relaxed),
            3
        );
    }
}

#[test]
fn traffic_diagnostics_reprobe_each_read_and_do_not_replay_lost_replies() {
    use arc_ipc::traffic::diagnostics as d;
    let mut probes = 0;
    let peer = Peer::new(4, move |stream, request| {
        let response = if request.operation == d::OPERATION_CAPABILITIES {
            Response::error(request.request_id, arc_ipc::ERROR_UNSUPPORTED_CAPABILITY)
        } else if request.operation == d::OPERATION_PYTHON_CAPABILITIES {
            probes += 1;
            assert!(request.payload.is_null());
            if probes == 1 {
                Response::ok(request.request_id, json!({"capability":d::PYTHON_CAPABILITY}))
            } else {
                Response::error(request.request_id, arc_ipc::ERROR_UNSUPPORTED_CAPABILITY)
            }
        } else {
            assert_eq!(request.operation, d::OPERATION_PYTHON_JOB);
            Response::ok(request.request_id, traffic_fixtures()["results"][0].clone())
        };
        arc_ipc::write_response(stream, &response).unwrap();
    });
    let bridge = peer.bridge();
    assert!(bridge.traffic_job_diagnostics(JOB).is_ok());
    assert_eq!(
        bridge.traffic_job_diagnostics(JOB).unwrap_err().code,
        ErrorCode::Unsupported
    );
    assert_eq!(peer.requests.load(Ordering::Relaxed), 4);

    let peer = Peer::new(2, |stream, request| {
        if request.operation == d::OPERATION_PYTHON_CAPABILITIES {
            arc_ipc::write_response(
                stream,
                &Response::ok(request.request_id, json!({"capability":d::PYTHON_CAPABILITY})),
            )
            .unwrap();
        } else {
            assert_eq!(request.operation, d::OPERATION_PYTHON_JOB);
            // Close after receiving the read, without an acknowledgment.
        }
    });
    assert_eq!(
        App::new(peer.bridge()).invoke("traffic_job_diagnostics", json!({"job_id":JOB})),
        // Session's strict frame decoder rejects an absent frame as invalid.
        // This is an unavailable read, never evidence of an absent permit or
        // execution. Retained/unconfirmed UI behavior is tested separately.
        Err(error("invalid_reply", false))
    );
    assert_eq!(peer.requests.load(Ordering::Relaxed), 2);
}

#[test]
fn registered_traffic_diagnostics_reject_malformed_and_inconsistent_results() {
    let valid = traffic_fixtures()["results"][0].clone();
    let mut cases = vec![Value::Null, json!([])];
    for (path, value) in [
        (vec!["job_id"], json!(OTHER_JOB)),
        (vec!["recorded_at_ms"], json!(9)),
        (vec!["recorded_at_ms"], json!(9_007_199_254_740_992_u64)),
        (vec!["liveness_fresh"], json!(true)),
        (
            vec!["classification"],
            json!(["TYPECHECK", "HOST_LOCAL", "BUN_SCRIPT_TYPECHECK", "NONE"]),
        ),
        (vec!["classification", "classification"], json!("TEST")),
        (vec!["classification", "private"], json!(SENTINEL)),
        (vec!["admission", "state"], json!("BYPASSED")),
        (vec!["admission", "permit_active"], json!(true)),
        (vec!["admission", "wait_reason"], json!("class_window")),
        (vec!["admission", "sequence"], json!(0)),
        (
            vec!["admission", "outcome"],
            json!({"kind":"EXITED","code":17}),
        ),
        (vec!["admission", "root_job_id"], json!(JOB)),
        (vec!["limits", "typecheck", "window"], json!(0)),
        (
            vec!["limits", "typecheck", "reservation", "cpu_millis"],
            json!(2001),
        ),
        (
            vec!["limits", "typecheck"],
            json!([1,{"cpu_millis":1000,"memory_bytes":1073741824}]),
        ),
        (vec!["private"], json!("x".repeat(8193))),
    ] {
        let mut case = valid.clone();
        let mut target = &mut case;
        for key in path {
            target = &mut target[key];
        }
        *target = value;
        cases.push(case);
    }
    for field in ["classification", "admission"] {
        let mut case = valid.clone();
        case.as_object_mut().unwrap().remove(field);
        cases.push(case);
    }
    let mut missing = valid.clone();
    missing["admission"]
        .as_object_mut()
        .unwrap()
        .remove("granted_at_ms");
    cases.push(missing);
    for case in cases {
        let peer = traffic_peer(case);
        assert_eq!(
            App::new(peer.bridge()).invoke("traffic_job_diagnostics", json!({"job_id":JOB})),
            Err(error("invalid_reply", false))
        );
    }
}

#[test]
fn traffic_diagnostics_wire_errors_release_reads_and_shutdown_stops_dispatch() {
    use arc_ipc::traffic::diagnostics as d;
    for operation in [d::OPERATION_PYTHON_CAPABILITIES, d::OPERATION_PYTHON_JOB] {
        let peer = Peer::new(
            if operation == d::OPERATION_PYTHON_JOB { 3 } else { 2 },
            move |stream, request| {
                let response = if request.operation == operation {
                    Response::error(request.request_id, arc_ipc::ERROR_DB_BUSY)
                } else if request.operation == d::OPERATION_PYTHON_CAPABILITIES {
                    Response::ok(request.request_id, json!({"capability":d::PYTHON_CAPABILITY}))
                } else {
                    assert_eq!(request.operation, arc_ipc::OPERATION_HEALTH);
                    Response::ok(request.request_id, health())
                };
                arc_ipc::write_response(stream, &response).unwrap();
            },
        );
        let bridge = peer.bridge();
        assert_eq!(
            bridge.traffic_job_diagnostics(JOB).unwrap_err().code,
            ErrorCode::Busy
        );
        assert!(bridge.daemon_health().is_ok());
    }
    // Shared exclusion and dropped-waiter ownership use held blocking closures
    // in bridge::permit_tests, independent of native socket expiry.
    let peer = traffic_peer(traffic_fixtures()["results"][0].clone());
    let bridge = peer.bridge();
    tauri::async_runtime::block_on(bridge.traffic_job_diagnostics_async(JOB.to_owned())).unwrap();
    assert_eq!(peer.requests.load(Ordering::Relaxed), 2);
    bridge.shutdown();
    assert_eq!(
        bridge.traffic_job_diagnostics(JOB).unwrap_err().code,
        ErrorCode::Cancelled
    );
}

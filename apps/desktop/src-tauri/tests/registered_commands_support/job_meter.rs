fn job_meter_summary_wire() -> Value {
    let mut report: Value = serde_json::from_str(include_str!(
        "../../../../../crates/arc-core/tests/fixtures/job-meter-report.json"
    ))
    .unwrap();
    report["job_id"] = json!(JOB);
    report["accounting_relation"]["root_job_id"] = json!(JOB);
    json!({"schema_version":1,"job_id":JOB,"accounting":{"state":"recorded","report":report},
        "queue_wait":{"seconds":3.0,"clock":"daemon_wall_clock","basis":"queued_to_reserved"}})
}

#[test]
fn registered_job_meter_uses_fixed_authenticated_operation_and_exact_correlated_schema() {
    let peer = Peer::new(1, |stream, request| {
        assert_eq!(request.protocol_version, 1);
        assert_eq!(request.operation, arc_ipc::meter::OPERATION_METER_SUMMARY);
        assert_eq!(request.payload, json!({"job_id":JOB}));
        arc_ipc::write_response(
            stream,
            &Response::ok(request.request_id, job_meter_summary_wire()),
        )
        .unwrap();
    });
    let actual = App::new(peer.bridge())
        .invoke("meter_summary", json!({"job_id":JOB}))
        .unwrap();
    assert_eq!(
        actual,
        json!({"data":job_meter_summary_wire(),"received_at_ms":RECEIVED})
    );
    let decoded = arc_ipc::meter::decode_summary(&actual["data"], JOB).unwrap();
    assert!(decoded.is_valid());
    for private in ["argv", "cwd", "command", "classification", SENTINEL] {
        assert!(!actual.to_string().contains(private));
    }
}

#[test]
fn registered_job_meter_validates_input_before_socket_work() {
    let peer = Peer::new(0, |_, _| panic!("invalid meter ID performed I/O"));
    let app = App::new(peer.bridge());
    for id in [
        String::new(),
        "a".repeat(31),
        "a".repeat(33),
        "A".repeat(32),
        "x".repeat(1_048_577),
    ] {
        assert_eq!(
            app.invoke("meter_summary", json!({"job_id":id})),
            Err(error("invalid_request", false))
        );
    }
    for body in [
        json!({}),
        json!({"jobId":JOB}),
        json!({"job_id":[SENTINEL]}),
    ] {
        let rejected = app.invoke("meter_summary", body).unwrap_err();
        assert!(!rejected.to_string().contains(SENTINEL));
    }
    assert_eq!(peer.requests.load(Ordering::Relaxed), 0);
    assert!(
        app.invoke("meter_report", json!({})).is_err(),
        "no desktop reporting proxy"
    );
}

#[test]
fn registered_job_meter_preserves_partial_nested_and_nullable_values() {
    let mut partial = job_meter_summary_wire();
    partial["accounting"]["report"]["completeness"] = json!("partial");
    partial["accounting"]["report"]["missing_coverage"] = json!(["unobserved_descendants"]);
    partial["accounting"]["report"]["accounting_relation"] =
        json!({"kind":"included_in_root","root_job_id":OTHER_JOB});
    let mut unknown = job_meter_summary_wire();
    unknown["accounting"]["report"]["completeness"] = json!("unknown");
    unknown["accounting"]["report"]["missing_coverage"] = json!(["native_usage_unavailable"]);
    unknown["accounting"]["report"]["cpu_core_seconds"] =
        json!({"user":null,"system":null,"total":null});
    for expected in [partial, unknown] {
        let peer = Peer::result(expected.clone());
        let actual = App::new(peer.bridge())
            .invoke("meter_summary", json!({"job_id":JOB}))
            .unwrap();
        assert_eq!(actual["data"], expected);
    }
    for state in [
        "missing",
        "expired",
        "legacy",
        "interrupted",
        "unavailable",
        "not_terminated",
        "unsupported",
    ] {
        let mut expected = job_meter_summary_wire();
        expected["accounting"] = if matches!(state, "missing" | "expired") {
            json!({"state":state})
        } else {
            json!({"state":"unknown","reason":state})
        };
        expected["queue_wait"] = json!({"seconds":null,"clock":"unknown","basis":"unavailable"});
        let peer = Peer::result(expected.clone());
        assert_eq!(
            App::new(peer.bridge())
                .invoke("meter_summary", json!({"job_id":JOB}))
                .unwrap()["data"],
            expected
        );
    }
}

#[test]
fn registered_job_meter_rejects_malformed_private_and_mismatched_replies() {
    let mut cases = vec![Value::Null, json!([])];
    for (path, value) in [
        (vec!["job_id"], json!(OTHER_JOB)),
        (vec!["secret"], json!(SENTINEL)),
        (vec!["accounting", "report", "job_id"], json!(OTHER_JOB)),
        (
            vec!["accounting", "report", "cpu_core_seconds", "total"],
            json!(3),
        ),
        (
            vec!["accounting", "report", "scope"],
            json!("complete_tree"),
        ),
        (
            vec!["accounting", "report", "observer"],
            json!([100, "1001", "00000000-0000-0000-0000-000000000001"]),
        ),
        (
            vec!["accounting", "report", "missing_coverage"],
            json!([SENTINEL]),
        ),
        (
            vec!["queue_wait"],
            json!([3, "daemon_wall_clock", "queued_to_reserved"]),
        ),
    ] {
        let mut v = job_meter_summary_wire();
        let mut target = &mut v;
        for key in path {
            target = &mut target[key];
        }
        *target = value;
        cases.push(v);
    }
    for v in cases {
        let peer = Peer::result(v);
        assert_eq!(
            App::new(peer.bridge()).invoke("meter_summary", json!({"job_id":JOB})),
            Err(error("invalid_reply", false))
        );
    }
}

#[test]
fn registered_job_meter_errors_are_local_static_and_do_not_retry_busy() {
    for (wire, expected, retryable) in [
        (arc_ipc::ERROR_UNSUPPORTED_CAPABILITY, "unsupported", false),
        (arc_ipc::meter::ERROR_METER_UNAVAILABLE, "unavailable", true),
        (arc_ipc::ERROR_DB_BUSY, "busy", true),
    ] {
        let peer = Peer::new(2, move |stream, request| {
            let response = if request.operation == arc_ipc::meter::OPERATION_METER_SUMMARY {
                Response::error(request.request_id, wire)
            } else {
                assert_eq!(request.operation, arc_ipc::OPERATION_HEALTH);
                Response::ok(request.request_id, health())
            };
            arc_ipc::write_response(stream, &response).unwrap();
        });
        let app = App::new(peer.bridge());
        assert_eq!(
            app.invoke("meter_summary", json!({"job_id":JOB})),
            Err(error(expected, retryable))
        );
        assert_eq!(
            app.invoke("daemon_health", json!({})).unwrap()["data"],
            health()
        );
        assert_eq!(peer.requests.load(Ordering::Relaxed), 2);
    }
}

#[test]
fn job_meter_sync_bridge_uses_the_same_validator_and_shutdown_lane() {
    let peer = Peer::result(job_meter_summary_wire());
    let bridge = peer.bridge();
    let actual = bridge.meter_summary(JOB).unwrap();
    assert_eq!(
        serde_json::to_value(actual).unwrap()["data"],
        job_meter_summary_wire()
    );
    bridge.shutdown();
    assert_eq!(
        bridge.meter_summary(JOB).unwrap_err().code,
        ErrorCode::Cancelled
    );
}

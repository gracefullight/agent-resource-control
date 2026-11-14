use arc_ipc::{
    dispatch, governor,
    traffic::{self, diagnostics as d, ProbeResult},
    Request, Response, ERROR_INVALID_REQUEST, ERROR_UNSUPPORTED_CAPABILITY,
};
use serde_json::{json, Value};
const ID: &str = "0123456789abcdef0123456789abcdef";
fn enqueue(class: &str, reason: &str) -> Value {
    json!({"action":"ENQUEUE","job_id":ID,"observer":{"pid":1,"start_token":"1",
        "boot_id":"01234567-89ab-cdef-0123-456789abcdef"},"classification":{
        "classification":class,"locality":"HOST_LOCAL","reason":reason,"uncertainty":"NONE"}})
}
#[test]
fn four_script_classes_are_gated_without_widening_legacy_or_direct_bun_contracts() {
    assert_eq!(arc_ipc::PROTOCOL_VERSION, 1);
    for (class, reason) in [
        ("BUILD", "BUN_SCRIPT_BUILD"),
        ("TEST", "BUN_SCRIPT_TEST"),
        ("LINT", "BUN_SCRIPT_LINT"),
        ("TYPECHECK", "BUN_SCRIPT_TYPECHECK"),
    ] {
        let raw = enqueue(class, reason);
        let wrapped = json!({"action":"TRAFFIC","request":raw});
        assert!(traffic::scripts::decode_job(traffic::OPERATION_BUN_SCRIPTS_JOB, &wrapped).is_ok());
        assert_eq!(governor::decode_job(&wrapped), Err(ERROR_INVALID_REQUEST));
        for old in [traffic::OPERATION_TRAFFIC_JOB, traffic::OPERATION_BUN_JOB] {
            assert_eq!(
                traffic::decode_job_for(old, &raw),
                Err(ERROR_INVALID_REQUEST)
            );
        }
        let register = json!({"action":"REGISTER","job_id":ID,"observer":raw["observer"],"classification":raw["classification"]});
        assert!(arc_ipc::jobs::decode_start(&register).is_err());
        // A replacement old dispatcher must reject before inspecting new enums.
        let mut request = Request::new("same-request", traffic::OPERATION_BUN_SCRIPTS_JOB);
        request.payload = json!({"classification":"THIS_ENUM_IS_INVALID"});
        assert_eq!(
            dispatch(&request, || panic!("unknown operation reached database")),
            Response::error("same-request", ERROR_UNSUPPORTED_CAPABILITY)
        );
    }
    for (class, reason) in [("BUILD", "CARGO_BUILD"), ("TEST", "BUN_TEST")] {
        let raw = enqueue(class, reason);
        assert!(governor::decode_job(&json!({"action":"TRAFFIC","request":raw})).is_ok());
        assert_eq!(
            traffic::scripts::decode_job(
                traffic::OPERATION_BUN_SCRIPTS_JOB,
                &json!({"action":"TRAFFIC","request":raw})
            ),
            Err(ERROR_INVALID_REQUEST)
        );
    }
    assert_eq!(
        traffic::scripts::decode_job("unknown.operation", &json!([])),
        Err(ERROR_UNSUPPORTED_CAPABILITY)
    );
    assert_eq!(
        traffic::decode_job_for("unknown.operation", &json!([])),
        Err(ERROR_UNSUPPORTED_CAPABILITY)
    );
    assert_eq!(
        serde_json::to_value(traffic::Capabilities::supported()).unwrap(),
        json!({"capability":"traffic.v1"})
    );
    assert_eq!(
        serde_json::to_value(traffic::Capabilities::bun_supported()).unwrap(),
        json!({"capability":"traffic.bun.v1"})
    );
    assert_eq!(
        serde_json::to_value(governor::GovernorCapabilities::supported()).unwrap(),
        json!({"capability":"governor.v1","execution_capability":"governor.execution.v1"})
    );
}
#[test]
fn script_and_diagnostic_probes_require_exact_identity_protocol_and_capability_pairs() {
    for (op, cap) in [
        (
            traffic::OPERATION_BUN_SCRIPTS_CAPABILITIES,
            traffic::BUN_SCRIPTS_CAPABILITY,
        ),
        (d::OPERATION_CAPABILITIES, d::CAPABILITY),
    ] {
        let request = Request::new("probe-id", op);
        let supported = Response::ok("probe-id", json!({"capability":cap}));
        assert_eq!(
            traffic::interpret_probe_for(&request, &supported, op, cap),
            ProbeResult::Supported
        );
        let old = dispatch(&request, || panic!());
        assert_eq!(
            traffic::interpret_probe_for(&request, &old, op, cap),
            ProbeResult::Unsupported
        );
        for invalid in [
            Response::ok("wrong-id", json!({"capability":cap})),
            Response::ok("probe-id", json!({"capability":cap,"extra":true})),
            Response::ok("probe-id", json!({"capability":traffic::CAPABILITY})),
            Response::error("probe-id", arc_ipc::ERROR_DB_BUSY),
        ] {
            assert_eq!(
                traffic::interpret_probe_for(&request, &invalid, op, cap),
                ProbeResult::Invalid
            );
        }
        let mut bad = supported.clone();
        bad.protocol_version = 2;
        assert_eq!(
            traffic::interpret_probe_for(&request, &bad, op, cap),
            ProbeResult::Invalid
        );
        assert_eq!(
            traffic::interpret_probe_for(&request, &supported, "invented", cap),
            ProbeResult::Invalid
        );
    }
}
fn diagnostic() -> Value {
    json!({"schema_version":1,"job_id":ID,"recorded_at_ms":20,"liveness_fresh":false,
        "classification":enqueue("TYPECHECK","BUN_SCRIPT_TYPECHECK")["classification"],
        "admission":{"state":"RESERVED","sequence":1,"queued_at_ms":10,"granted_at_ms":12,
            "queue_wait_ms":null,"wait_reason":null,"last_decision":null,"root_job_id":ID,
            "permit_generation":1,"reservation":{"cpu_millis":1000,"memory_bytes":1073741824},
            "permit_active":true,"outcome":null},"limits":arc_ipc::policy::ScriptPolicyLimits::default()})
}
#[test]
fn diagnostic_read_is_bounded_strict_safe_integer_and_identity_bound() {
    let id = arc_ipc::jobs::JobId::try_from(ID.to_owned()).unwrap();
    let valid = diagnostic();
    assert!(d::decode_result(&valid, &id).is_ok());
    let mut request = Request::new("query-id", d::OPERATION_JOB);
    request.payload = json!({"job_id":ID});
    assert!(d::interpret_result(&request, &Response::ok("query-id", valid.clone())).is_ok());
    assert!(d::interpret_result(&request, &Response::ok("another", valid.clone())).is_err());
    assert_eq!(
        d::interpret_result(
            &request,
            &Response::error("query-id", ERROR_UNSUPPORTED_CAPABILITY)
        ),
        Err(ERROR_UNSUPPORTED_CAPABILITY)
    );
    for pointer in [
        "/classification",
        "/admission",
        "/admission/reservation",
        "/admission/outcome",
    ] {
        let mut bad = valid.clone();
        *bad.pointer_mut(pointer).unwrap() = json!([]);
        assert!(d::decode_result(&bad, &id).is_err(), "{pointer}");
    }
    for (pointer, value) in [
        ("/liveness_fresh", json!(true)),
        ("/schema_version", json!(2)),
        ("/recorded_at_ms", json!(9_007_199_254_740_992u64)),
        ("/recorded_at_ms", json!(9)),
        ("/job_id", json!("a".repeat(32))),
        ("/admission/sequence", json!(0)),
        ("/admission/permit_generation", json!(0)),
        ("/admission/granted_at_ms", json!(9)),
        ("/admission/permit_active", json!(false)),
        ("/admission/wait_reason", json!("class_window")),
    ] {
        let mut bad = valid.clone();
        *bad.pointer_mut(pointer).unwrap() = value;
        assert!(d::decode_result(&bad, &id).is_err(), "{pointer}");
    }
    let mut bad = valid.clone();
    bad["admission"]["lineage_context"] = json!("a".repeat(64));
    assert!(d::decode_result(&bad, &id).is_err());
    for field in ["classification", "admission", "limits"] {
        let mut bad = valid.clone();
        bad.as_object_mut().unwrap().remove(field);
        assert!(d::decode_result(&bad, &id).is_err());
    }
    assert!(d::decode_request(&json!({"job_id":ID,"path":"private"})).is_err());
    assert!(d::decode_request(&json!({"job_id":"a".repeat(1024)})).is_err());
}

#[test]
fn completed_root_may_retain_its_permit_for_nested_descendants() {
    let id = arc_ipc::jobs::JobId::try_from(ID.to_owned()).unwrap();
    let mut value = diagnostic();
    value["admission"]["state"] = json!("COMPLETED");
    value["admission"]["queue_wait_ms"] = json!(2);
    value["admission"]["outcome"] = json!({"kind":"EXITED","code":17});
    // The parent's outcome is final, but the existing shared root reservation
    // remains active until its last verified descendant finishes.
    assert!(d::decode_result(&value, &id).is_ok());
    value["admission"]["permit_active"] = json!(false);
    assert!(d::decode_result(&value, &id).is_ok());
    value["admission"]["state"] = json!("UNRESERVED");
    value["admission"]["outcome"] = Value::Null;
    assert!(d::decode_result(&value, &id).is_ok());
    value["admission"]["permit_active"] = json!(true);
    assert!(d::decode_result(&value, &id).is_err());
}

fn assert_terminal_shared_permit_decodes(state: &str) {
    let id = arc_ipc::jobs::JobId::try_from(ID.to_owned()).unwrap();
    for permit_active in [true, false] {
        for queue_wait in [Value::Null, json!(2)] {
            let mut value = diagnostic();
            value["admission"]["state"] = json!(state);
            value["admission"]["permit_active"] = json!(permit_active);
            value["admission"]["queue_wait_ms"] = queue_wait;
            // Terminal roots retain the shared permit until every active
            // descendant finishes, independently of the root's own state.
            let result = d::decode_result(&value, &id)
                .unwrap_or_else(|error| panic!("{state} permit_active={permit_active}: {error}"));
            assert_eq!(serde_json::to_value(result).unwrap(), value);
        }
    }
    // A terminal entry can also have no recorded reservation at all.
    let mut value = diagnostic();
    value["admission"]["state"] = json!(state);
    value["admission"]["permit_active"] = json!(false);
    for field in ["root_job_id", "permit_generation", "reservation"] {
        value["admission"][field] = Value::Null;
    }
    assert!(d::decode_result(&value, &id).is_ok());
    value["admission"]["permit_active"] = json!(true);
    assert_eq!(d::decode_result(&value, &id), Err(ERROR_INVALID_REQUEST));
}

#[test]
fn cancelled_root_diagnostic_preserves_retained_and_released_shared_permits() {
    assert_terminal_shared_permit_decodes("CANCELLED");
}

#[test]
fn abandoned_root_diagnostic_preserves_retained_and_released_shared_permits() {
    assert_terminal_shared_permit_decodes("ABANDONED");
}

#[test]
fn terminal_shared_permits_keep_outcome_timing_resource_and_completeness_validation() {
    let id = arc_ipc::jobs::JobId::try_from(ID.to_owned()).unwrap();
    for state in ["CANCELLED", "ABANDONED"] {
        for permit_active in [true, false] {
            let mut valid = diagnostic();
            valid["admission"]["state"] = json!(state);
            valid["admission"]["permit_active"] = json!(permit_active);
            for (pointer, invalid) in [
                ("/admission/outcome", json!({"kind":"EXITED","code":17})),
                ("/admission/granted_at_ms", json!(9)),
                ("/admission/granted_at_ms", json!(21)),
                ("/admission/queue_wait_ms", json!(3)),
                ("/admission/root_job_id", Value::Null),
                ("/admission/permit_generation", Value::Null),
                ("/admission/reservation", Value::Null),
                ("/admission/permit_generation", json!(0)),
                ("/admission/reservation/cpu_millis", json!(0)),
                ("/admission/reservation/memory_bytes", json!(0)),
                (
                    "/admission/reservation/memory_bytes",
                    json!(arc_ipc::jobs::MAX_TIMESTAMP + 1),
                ),
            ] {
                let mut bad = valid.clone();
                *bad.pointer_mut(pointer).unwrap() = invalid;
                assert_eq!(
                    d::decode_result(&bad, &id),
                    Err(ERROR_INVALID_REQUEST),
                    "{state} permit_active={permit_active} {pointer}"
                );
            }
            // Timing validation also applies without a known grant time.
            valid["admission"]["granted_at_ms"] = Value::Null;
            valid["admission"]["queue_wait_ms"] = json!(11);
            assert_eq!(d::decode_result(&valid, &id), Err(ERROR_INVALID_REQUEST));
        }
    }
    let mut unreserved = diagnostic();
    unreserved["admission"]["state"] = json!("UNRESERVED");
    assert_eq!(
        d::decode_result(&unreserved, &id),
        Err(ERROR_INVALID_REQUEST)
    );
}

#[test]
fn null_grant_queue_wait_cannot_place_execution_after_the_snapshot() {
    let id = arc_ipc::jobs::JobId::try_from(ID.to_owned()).unwrap();
    let fixtures: Value = serde_json::from_str(include_str!(
        "../../../apps/desktop/tests/fixtures/traffic-scripts-wire.json"
    ))
    .unwrap();
    let completed = fixtures["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|value| value["admission"]["state"] == "COMPLETED")
        .unwrap();
    for state in ["RUNNING", "COMPLETED", "UNRESERVED"] {
        let mut value = completed.clone();
        value["admission"]["state"] = json!(state);
        if state != "COMPLETED" {
            value["admission"]["outcome"] = Value::Null;
        }
        assert_eq!(value["admission"]["queued_at_ms"], json!(10));
        assert_eq!(value["recorded_at_ms"], json!(20));
        assert!(value["admission"]["granted_at_ms"].is_null());
        assert!(d::decode_result(&value, &id).is_ok(), "{state}");
        value["admission"]["queue_wait_ms"] = json!(11);
        let result: d::DiagnosticResult = serde_json::from_value(value.clone()).unwrap();
        assert!(!result.is_valid(), "{state} starts after its snapshot");
        assert_eq!(d::decode_result(&value, &id), Err(ERROR_INVALID_REQUEST));
    }
}

#[test]
fn queue_wait_accepts_snapshot_and_safe_integer_boundaries_without_a_grant() {
    let id = arc_ipc::jobs::JobId::try_from(ID.to_owned()).unwrap();
    let max = arc_ipc::jobs::MAX_TIMESTAMP;
    for state in ["RUNNING", "COMPLETED", "UNRESERVED"] {
        for (queued, recorded, wait) in [
            (10, 20, 0),
            (10, 20, 10),
            (max, max, 0),
            (max - 10, max, 10),
            (0, max, max),
        ] {
            let mut value = diagnostic();
            value["recorded_at_ms"] = json!(recorded);
            let admission = &mut value["admission"];
            admission["state"] = json!(state);
            admission["queued_at_ms"] = json!(queued);
            admission["queue_wait_ms"] = json!(wait);
            admission["granted_at_ms"] = Value::Null;
            admission["root_job_id"] = Value::Null;
            admission["permit_generation"] = Value::Null;
            admission["reservation"] = Value::Null;
            admission["permit_active"] = json!(false);
            if state == "COMPLETED" {
                admission["outcome"] = json!({"kind":"EXITED","code":17});
            }
            let result = d::decode_result(&value, &id).unwrap_or_else(|error| {
                panic!("{state} queued={queued} recorded={recorded} wait={wait}: {error}")
            });
            assert_eq!(serde_json::to_value(result).unwrap(), value);
        }
    }
}

#[test]
fn queue_wait_sum_must_fit_the_recorded_safe_integer_timestamp() {
    let id = arc_ipc::jobs::JobId::try_from(ID.to_owned()).unwrap();
    let max = arc_ipc::jobs::MAX_TIMESTAMP;
    for (queued, wait) in [(max, 1), (1, max), (max, max)] {
        let mut value = diagnostic();
        value["recorded_at_ms"] = json!(max);
        value["admission"]["state"] = json!("RUNNING");
        value["admission"]["queued_at_ms"] = json!(queued);
        value["admission"]["queue_wait_ms"] = json!(wait);
        value["admission"]["granted_at_ms"] = Value::Null;
        assert_eq!(d::decode_result(&value, &id), Err(ERROR_INVALID_REQUEST));
    }
}

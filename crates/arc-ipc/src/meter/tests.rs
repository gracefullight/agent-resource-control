use super::*;
use serde_json::json;

fn fixture() -> MeterSnapshot {
    serde_json::from_str(include_str!(
        "../../../arc-core/tests/fixtures/meter-snapshot.json"
    ))
    .unwrap()
}

#[test]
fn additive_contract_keeps_protocol_one_and_legacy_unsupported() {
    let request = snapshot_request("meter-contract-1");
    assert_eq!(request.protocol_version, 1);
    assert!(request.payload.is_null());
    // The legacy dispatcher has no new handler until the runtime task installs it.
    let legacy = crate::dispatch(&request, || panic!("meter never queries SQLite"));
    assert_eq!(
        interpret_snapshot(&request, &legacy),
        Err(SnapshotError::Unsupported)
    );
    let response = snapshot_response(&request, &fixture());
    assert_eq!(interpret_snapshot(&request, &response).unwrap(), fixture());
    let mut bytes = Vec::new();
    crate::write_response(&mut bytes, &response).unwrap();
    assert_eq!(
        crate::read_response(&mut bytes.as_slice()).unwrap(),
        response
    );
    let health = crate::dispatch(
        &Request::new("health", crate::OPERATION_HEALTH),
        || panic!(),
    );
    assert_eq!(health.status, STATUS_OK);
    assert!(health.result.unwrap().get("host").is_none());
}

#[test]
fn request_remains_fixed_and_envelope_identity_is_checked() {
    let request = snapshot_request("meter-contract-2");
    let snapshot = fixture();
    for payload in [json!({}), json!({"path":"/"}), json!([]), json!(0)] {
        let mut invalid = request.clone();
        invalid.payload = payload;
        assert_eq!(
            snapshot_response(&invalid, &snapshot).error.unwrap().code,
            ERROR_INVALID_REQUEST
        );
    }
    let mut response = snapshot_response(&request, &snapshot);
    response.request_id = "other".to_owned();
    assert_eq!(
        interpret_snapshot(&request, &response),
        Err(SnapshotError::InvalidReply)
    );
    response.request_id = request.request_id.clone();
    response.protocol_version = 2;
    assert_eq!(
        interpret_snapshot(&request, &response),
        Err(SnapshotError::Protocol)
    );
    let mut invalid = request.clone();
    invalid.protocol_version = 2;
    assert_eq!(
        snapshot_response(&invalid, &snapshot).error.unwrap().code,
        crate::ERROR_PROTOCOL_INCOMPATIBLE
    );
    invalid = Request::new("wrong-op", crate::OPERATION_HEALTH);
    assert_eq!(
        interpret_snapshot(&invalid, &snapshot_response(&request, &snapshot)),
        Err(SnapshotError::InvalidReply)
    );
    assert_eq!(
        snapshot_response(&invalid, &snapshot).error.unwrap().code,
        ERROR_INVALID_REQUEST
    );
}

#[test]
fn missing_malformed_and_unsupported_responses_stay_distinct() {
    let request = snapshot_request("meter-contract-3");
    for (code, expected) in [
        (
            crate::ERROR_UNSUPPORTED_CAPABILITY,
            SnapshotError::Unsupported,
        ),
        (ERROR_METER_UNAVAILABLE, SnapshotError::Unavailable),
        ("UNRECOGNIZED", SnapshotError::InvalidReply),
    ] {
        assert_eq!(
            interpret_snapshot(&request, &Response::error(&request.request_id, code)),
            Err(expected)
        );
    }
    let mut invalid = fixture();
    invalid.sequence = 0;
    assert_eq!(
        snapshot_response(&request, &invalid).error.unwrap().code,
        ERROR_METER_UNAVAILABLE
    );
    let mut response = snapshot_response(&request, &fixture());
    response.error = Response::error("x", "extra").error;
    assert_eq!(
        interpret_snapshot(&request, &response),
        Err(SnapshotError::InvalidReply)
    );
}

#[test]
fn wire_validation_rejects_arrays_unknown_keys_and_oversize_content() {
    let value = serde_json::to_value(fixture()).unwrap();
    for invalid in [json!([]), json!(0), Value::Null] {
        assert!(decode_snapshot(&invalid).is_err());
    }
    let mut invalid = value.clone();
    invalid["host"]["swap_used_bytes"] =
        json!(["bytes", "native", 1700000000000_u64, 0, {"availability":"denied"}]);
    assert!(decode_snapshot(&invalid).is_err());
    invalid = value.clone();
    invalid["host"]["invented_job_cpu"] = json!(0);
    assert!(decode_snapshot(&invalid).is_err());
    invalid = value.clone();
    invalid["host"]["filesystem"]["free_bytes"]["reading"]["diagnostic"] =
        json!("x".repeat(MAX_SNAPSHOT_BYTES));
    assert!(decode_snapshot(&invalid).is_err());
    invalid = value.clone();
    invalid["host"]["filesystem"]["free_bytes"]["reading"]["diagnostic"] = json!("x".repeat(129));
    assert!(decode_snapshot(&invalid).is_err());
    invalid = value;
    invalid["daemon"]["daemon_instance"] = json!({"pid":0});
    assert!(decode_snapshot(&invalid).is_err());
}

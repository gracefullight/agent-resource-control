use arc_ipc::jobs::{
    CompleteRequest, JobId, Outcome, ProcessIdentity, StartRequest, StatusRequest,
};
use serde_json::{json, Value};
fn identity() -> Value {
    json!({"pid":1,"start_token":"18446744073709551615","boot_id":"01234567-89ab-cdef-0123-456789abcdef"})
}
#[test]
fn lifecycle_wire_shapes_are_protocol_one_and_deny_secret_fields() {
    let job_id = "0123456789abcdef0123456789abcdef";
    for value in [
        json!({"action":"CLAIM","job_id":job_id,"observer":identity()}),
        json!({"action":"ATTACH","job_id":job_id,"observer":identity(),"child":identity()}),
        json!({"action":"REGISTER","job_id":job_id,"observer":identity(),"classification":{
            "classification":"UNKNOWN","locality":"UNKNOWN","reason":"UNSUPPORTED_SYNTAX","uncertainty":"OBSERVE"}}),
    ] {
        let typed: StartRequest = serde_json::from_value(value.clone()).unwrap();
        assert!(typed.is_valid());
        assert_eq!(serde_json::to_value(typed).unwrap(), value);
        for field in ["argv", "environment", "cwd", "secret"] {
            let mut bad = value.clone();
            bad[field] = json!("secret-sentinel");
            assert!(serde_json::from_value::<StartRequest>(bad).is_err());
        }
    }
    for value in [
        json!({"query":"RECENT"}),
        json!({"query":"BY_ID","job_id":job_id}),
    ] {
        let typed: StatusRequest = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(typed).unwrap(), value);
    }
    for outcome in [
        json!({"kind":"EXITED","code":17}),
        json!({"kind":"SIGNALED","signal":15,"core_dumped":false}),
        json!({"kind":"SPAWN_FAILED","reason":"NOT_FOUND","os_code":2}),
    ] {
        let value = json!({"job_id":job_id,"observer":identity(),"outcome":outcome});
        let typed: CompleteRequest = serde_json::from_value(value.clone()).unwrap();
        assert!(typed.is_valid());
        assert_eq!(serde_json::to_value(typed).unwrap(), value);
    }
    assert_eq!(arc_ipc::PROTOCOL_VERSION, 1);
}
#[test]
fn identity_keys_and_outcomes_reject_invalid_native_ranges() {
    for bad in [
        "",
        "SECRET",
        "0123456789ABCDEF0123456789ABCDEF",
        "0123456789abcdef0123456789abcdeff",
    ] {
        assert!(JobId::try_from(bad.to_owned()).is_err());
    }
    for (field, bad) in [
        ("pid", json!(0)),
        ("pid", json!(2147483648u64)),
        ("start_token", json!("0")),
        ("start_token", json!("01")),
        ("start_token", json!("18446744073709551616")),
        ("boot_id", json!("SECRET")),
    ] {
        let mut value = identity();
        value[field] = bad;
        assert!(serde_json::from_value::<ProcessIdentity>(value).is_err());
    }
    assert!(!Outcome::Signaled {
        signal: 0,
        core_dumped: false
    }
    .is_valid());
    assert!(!Outcome::Signaled {
        signal: 128,
        core_dumped: false
    }
    .is_valid());
    assert!(serde_json::from_value::<Outcome>(json!({"kind":"EXITED","code":256})).is_err());
    assert!(serde_json::from_value::<Outcome>(
        json!({"kind":"EXITED","code":17,"secret":"sentinel"})
    )
    .is_err());
}

#[test]
fn lifecycle_decoders_enforce_object_shape_size_semantics_and_required_nulls() {
    use arc_ipc::jobs::{decode_complete, decode_start, decode_status};
    let key = "0123456789abcdef0123456789abcdef";
    let claim = json!({"action":"CLAIM","job_id":key,"observer":identity()});
    assert!(decode_start(&claim).is_ok());
    let mut bad = claim.clone();
    bad["observer"] = json!([1, "1", "01234567-89ab-cdef-0123-456789abcdef"]);
    assert!(decode_start(&bad).is_err());
    bad = claim.clone();
    bad["secret"] = json!("x".repeat(8192));
    assert_eq!(decode_start(&bad), Err(arc_ipc::ERROR_INVALID_REQUEST));
    assert!(decode_start(&json!([])).is_err());
    assert!(decode_status(&json!({"query":"RECENT","unexpected":1})).is_err());
    let mut request = json!({"job_id":key,"observer":identity(),"outcome":{"kind":"SPAWN_FAILED","reason":"OTHER","os_code":null}});
    assert!(decode_complete(&request).is_ok());
    request["outcome"]
        .as_object_mut()
        .unwrap()
        .remove("os_code");
    assert!(decode_complete(&request).is_err());
    request["outcome"] = json!({"kind":"SIGNALED","signal":0,"core_dumped":false});
    assert!(decode_complete(&request).is_err());
    request["outcome"] = json!({"kind":"SIGNALED","signal":128,"core_dumped":false});
    assert!(decode_complete(&request).is_err());
    let register = json!({"action":"REGISTER","job_id":key,"observer":identity(),
        "classification":{"classification":"BUILD","locality":"REMOTE","reason":"CARGO_BUILD","uncertainty":"NONE"}});
    assert!(decode_start(&register).is_err());
}

#[test]
fn snapshot_validation_rejects_phase_identity_outcome_and_time_contradictions() {
    use arc_ipc::jobs::{JobSnapshot, Phase};
    let snapshot = json!({
        "job_id":"0123456789abcdef0123456789abcdef", "phase":"REGISTERED",
        "classification":{"classification":"UNKNOWN","locality":"UNKNOWN","reason":"UNSUPPORTED_SYNTAX","uncertainty":"OBSERVE"},
        "observer":identity(), "observer_verified_at_ms":100, "child":null,"child_verified_at_ms":null,
        "observation":{"target":"OBSERVER","status":"VERIFIED_MATCH","observed_at_ms":100,"reason":"REGISTERED_NOT_CLAIMED"},
        "reason":"REGISTERED_NOT_CLAIMED","outcome":null,"registered_at_ms":100,
        "launch_claimed_at_ms":null,"completed_at_ms":null,"updated_at_ms":100,"revision":1
    });
    let base: JobSnapshot = serde_json::from_value(snapshot.clone()).unwrap();
    assert!(base.is_valid());
    for field in [
        "child",
        "child_verified_at_ms",
        "outcome",
        "launch_claimed_at_ms",
        "completed_at_ms",
    ] {
        let mut bad = snapshot.clone();
        bad.as_object_mut().unwrap().remove(field);
        assert!(
            serde_json::from_value::<JobSnapshot>(bad).is_err(),
            "{field}"
        );
    }
    let mut running = base.clone();
    running.phase = Phase::Running;
    assert!(!running.is_valid());
    for (field, value) in [
        ("revision", json!(0)),
        ("observer_verified_at_ms", json!(101)),
        ("launch_claimed_at_ms", json!(100)),
        ("registered_at_ms", json!(101)),
        ("phase", json!("COMPLETED")),
        ("outcome", json!({"kind":"EXITED","code":17})),
        ("child_verified_at_ms", json!(100)),
        ("completed_at_ms", json!(100)),
    ] {
        let mut bad = snapshot.clone();
        bad[field] = value;
        assert!(
            !serde_json::from_value::<JobSnapshot>(bad)
                .unwrap()
                .is_valid(),
            "{field}"
        );
    }
    let register = json!({"action":"REGISTER","job_id":base.job_id,"observer":identity(),
        "classification":["UNKNOWN","UNKNOWN","UNSUPPORTED_SYNTAX","OBSERVE"]});
    assert!(arc_ipc::jobs::decode_start(&register).is_err());
}

#[test]
fn native_signal_limit_rejects_impossible_completion_without_exposing_input() {
    #[cfg(target_os = "macos")]
    let invalid = 32;
    #[cfg(target_os = "linux")]
    let invalid = 65;
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    let invalid = 128;
    let payload = json!({"job_id":"0123456789abcdef0123456789abcdef","observer":identity(),"outcome":{"kind":"SIGNALED","signal":invalid,"core_dumped":false}});
    assert!(arc_ipc::jobs::decode_complete(&payload).is_err());
}

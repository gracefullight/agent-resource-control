use arc_ipc::{
    dispatch, Request, ERROR_INVALID_REQUEST, ERROR_PROTOCOL_INCOMPATIBLE, OPERATION_JOB_CLASSIFY,
};
use serde_json::{json, Value};

#[path = "../../arc-detector/tests/support/contract.rs"]
mod contract_fixtures;

fn contract() -> Value {
    serde_json::from_str(include_str!(
        "../../../.arc-verify/command-classification/contract.json"
    ))
    .unwrap()
}

#[test]
fn classification_contract_dispatch_never_calls_doctor() {
    for case in contract_fixtures::fixtures() {
        let mut request = Request::new(case["id"].as_str().unwrap(), OPERATION_JOB_CLASSIFY);
        request.payload = case["payload"].clone();
        let response = dispatch(&request, || {
            panic!("classification reached database callback")
        });
        assert_eq!(response.status, "ok", "{}", case["id"]);
        assert_eq!(
            response.result,
            Some(case["result"].clone()),
            "{}",
            case["id"]
        );
    }
}

#[test]
fn ssh_post_destination_typed_dispatch() {
    for case in contract_fixtures::ssh_fixtures() {
        let mut request = Request::new(case["id"].as_str().unwrap(), OPERATION_JOB_CLASSIFY);
        request.payload = case["payload"].clone();
        let response = dispatch(&request, || panic!("classification reached doctor"));
        assert_eq!(response.protocol_version, 1);
        assert_eq!(response.request_id, request.request_id);
        assert_eq!(response.status, "ok", "{}", case["id"]);
        assert!(response.error.is_none());
        let actual: arc_ipc::Classification =
            serde_json::from_value(response.result.unwrap()).unwrap();
        let expected: arc_ipc::Classification =
            serde_json::from_value(case["result"].clone()).unwrap();
        assert_eq!(actual, expected, "{}", case["id"]);
        assert!(actual.is_consistent());
    }
}

#[test]
fn malformed_payload_version_priority_and_legacy_envelopes() {
    for case in contract()["malformedPayloadFixtures"].as_array().unwrap() {
        let mut request = Request::new("malformed", OPERATION_JOB_CLASSIFY);
        request.payload = case["payload"].clone();
        let response = dispatch(&request, || panic!("invalid payload reached doctor"));
        assert_eq!(
            response.error.unwrap().code,
            ERROR_INVALID_REQUEST,
            "{}",
            case["id"]
        );
        request.protocol_version = 99;
        assert_eq!(
            dispatch(&request, || panic!()).error.unwrap().code,
            ERROR_PROTOCOL_INCOMPATIBLE
        );
    }
    let request = Request::decode(&serde_json::to_vec(&json!({"protocol_version":1,"request_id":"legacy","operation":"daemon.health","ignored":"value"})).unwrap()).unwrap();
    assert_eq!(dispatch(&request, || panic!()).status, "ok");
}

#[test]
fn ssh_option_values_are_not_forwarding_flags() {
    for (index, (command, reason)) in [
        ("ssh -lDora build01 make", "SSH_AMBIGUOUS"),
        ("ssh -oLogLevel=INFO build01 make", "SSH_AMBIGUOUS"),
        ("ssh -l N build01 make", "SSH_AMBIGUOUS"),
        ("ssh -o '--N' build01 make", "SSH_AMBIGUOUS"),
        ("ssh -zN build01 make", "SSH_AMBIGUOUS"),
        ("ssh -nNT build01", "SSH_FORWARDING"),
        ("ssh -vNL8080:host:80 build01 make", "SSH_FORWARDING"),
        ("ssh -D8080 build01 make", "SSH_FORWARDING"),
        ("ssh -lDora -N build01", "SSH_FORWARDING"),
    ]
    .iter()
    .enumerate()
    {
        let mut request = Request::new(format!("ssh-option-{index}"), OPERATION_JOB_CLASSIFY);
        request.payload = json!({"command": command});
        let response = dispatch(&request, || panic!("classification reached doctor"));
        assert_eq!(response.status, "ok");
        assert_eq!(
            response.result,
            Some(json!({"classification":"UNKNOWN",
            "locality":"UNKNOWN", "reason":reason,"uncertainty":"OBSERVE"})),
            "ssh-option-{index}"
        );
    }
}

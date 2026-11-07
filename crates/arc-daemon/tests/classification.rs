//! Actual daemon/client contract cases; no analyzed command is ever executed.
#[path = "classification/boundaries.rs"]
mod boundaries;
#[path = "classification/privacy.rs"]
mod privacy;
#[path = "support/runtime.rs"]
#[allow(dead_code)] // Shared owned-child fixture also exposes unrelated workload helpers.
mod runtime;
#[path = "classification/saturation.rs"]
mod saturation;
mod support;

#[path = "../../arc-detector/tests/support/contract.rs"]
mod contract_fixtures;

use arc_ipc::{Request, Response, OPERATION_JOB_CLASSIFY};
use runtime::{Fixture, OwnedChild};
use serde_json::{json, Value};
use std::{
    os::unix::net::UnixStream,
    path::Path,
    process::{Command, Output},
};

fn contract() -> Value {
    serde_json::from_str(include_str!(
        "../../../.arc-verify/command-classification/contract.json"
    ))
    .unwrap()
}
fn request(payload: Value) -> Request {
    let mut request = Request::new("classification-fixture", OPERATION_JOB_CLASSIFY);
    request.payload = payload;
    request
}
fn exchange(stream: &mut UnixStream, payload: Value) -> Response {
    arc_ipc::exchange(stream, &request(payload)).unwrap()
}
fn cli(fixture: &Fixture, args: &[&str]) -> Output {
    let path = Path::new(env!("CARGO_BIN_EXE_arc-daemon")).with_file_name("arcctl");
    Command::new(path)
        .args(args)
        .env(arc_core::ARC_SOCKET_PATH, &fixture.socket)
        .output()
        .unwrap()
}
fn human(result: &Value) -> String {
    format!(
        "Classification: {}\nLocality: {}\nReason: {}\nUncertainty: {}\nCommand not executed.\n",
        result["classification"].as_str().unwrap(),
        result["locality"].as_str().unwrap(),
        result["reason"].as_str().unwrap(),
        result["uncertainty"].as_str().unwrap()
    )
}

#[test]
fn actual_daemon_and_cli_delivered_contract_matrix() {
    daemon_and_cli_matrix(contract_fixtures::fixtures());
}

#[test]
fn ssh_post_destination_daemon_human_and_json_cli_matrix() {
    daemon_and_cli_matrix(contract_fixtures::ssh_fixtures());
}

fn daemon_and_cli_matrix(cases: Vec<Value>) {
    let _serial = support::serial();
    let fixture = Fixture::new();
    let mut child = fixture.spawn("");
    let mut stream = fixture.connect();
    for case in cases {
        let response = exchange(&mut stream, case["payload"].clone());
        assert_eq!(response.status, "ok", "{}", case["id"]);
        assert_eq!(
            response.result.as_ref(),
            Some(&case["result"]),
            "{}",
            case["id"]
        );
        let command = case["payload"]["command"].as_str().unwrap();
        let shell = case["payload"]["shell_kind"].as_str().unwrap_or("sh");
        for json in [false, true] {
            let mut args = vec!["classify", "--shell", shell];
            if json {
                args.push("--json");
            }
            args.extend(["--", command]);
            let output = cli(&fixture, &args);
            assert!(output.status.success(), "{}", case["id"]);
            assert!(output.stderr.is_empty(), "{}", case["id"]);
            if json {
                assert_eq!(output.stdout.last(), Some(&b'\n'), "{}", case["id"]);
                assert_eq!(output.stdout.iter().filter(|b| **b == b'\n').count(), 1);
                assert_eq!(
                    serde_json::from_slice::<Value>(&output.stdout).unwrap(),
                    case["result"],
                    "{}",
                    case["id"]
                );
            } else {
                assert_eq!(
                    output.stdout,
                    human(&case["result"]).as_bytes(),
                    "{}",
                    case["id"]
                );
            }
            assert!(!output.stdout.contains(&27));
        }
    }
    fixture.assert_cli();
    drop(stream);
    child.stop(libc::SIGTERM);
    let report = fixture.report();
    assert_eq!(report["metrics"]["db_outstanding"]["high_water"], 1); // only the explicit doctor
}

#[test]
fn invalid_payloads_version_priority_and_same_connection_recovery() {
    let _serial = support::serial();
    let fixture = Fixture::new();
    let mut child = fixture.spawn("");
    let mut stream = fixture.connect();
    for case in contract()["malformedPayloadFixtures"].as_array().unwrap() {
        let response = exchange(&mut stream, case["payload"].clone());
        assert_eq!(
            response.error.unwrap().code,
            arc_ipc::ERROR_INVALID_REQUEST,
            "{}",
            case["id"]
        );
        let mut wrong = request(case["payload"].clone());
        wrong.protocol_version = 99;
        assert_eq!(
            arc_ipc::exchange(&mut stream, &wrong)
                .unwrap()
                .error
                .unwrap()
                .code,
            arc_ipc::ERROR_PROTOCOL_INCOMPATIBLE
        );
        assert_eq!(
            exchange(&mut stream, json!({"command":"cargo test"}))
                .result
                .unwrap()["classification"],
            "TEST"
        );
    }
    arc_ipc::write_frame(&mut stream, b"{").unwrap();
    assert_eq!(
        arc_ipc::read_response(&mut stream)
            .unwrap()
            .error
            .unwrap()
            .code,
        arc_ipc::ERROR_INVALID_REQUEST
    );
    assert_eq!(
        exchange(&mut stream, json!({"command":"gh run watch"}))
            .result
            .unwrap()["classification"],
        "REMOTE_CI"
    );
    let unknown = Request::new("unsupported", "job.admit");
    assert_eq!(
        arc_ipc::exchange(&mut stream, &unknown)
            .unwrap()
            .error
            .unwrap()
            .code,
        arc_ipc::ERROR_UNSUPPORTED_CAPABILITY
    );
    drop(stream);
    child.stop(libc::SIGTERM);
    let report = fixture.report();
    assert_eq!(report["metrics"]["db_outstanding"]["high_water"], 0);
}

#[test]
fn ssh_option_values_preserve_typed_daemon_and_cli_observations() {
    let _serial = support::serial();
    let fixture = Fixture::new();
    let mut child = fixture.spawn("");
    let mut stream = fixture.connect();
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
        let expected = json!({"classification":"UNKNOWN","locality":"UNKNOWN",
            "reason":reason,"uncertainty":"OBSERVE"});
        let response = exchange(&mut stream, json!({"command":command}));
        assert_eq!(response.status, "ok", "ssh-option-{index}");
        assert_eq!(
            response.result,
            Some(expected.clone()),
            "ssh-option-{index}"
        );
        for json in [false, true] {
            let mut args = vec!["classify"];
            if json {
                args.push("--json");
            }
            args.extend(["--", command]);
            let output = cli(&fixture, &args);
            assert!(
                output.status.success() && output.stderr.is_empty(),
                "ssh-option-{index}"
            );
            if json {
                assert_eq!(
                    serde_json::from_slice::<Value>(&output.stdout).unwrap(),
                    expected,
                    "ssh-option-{index}"
                );
            } else {
                assert_eq!(
                    output.stdout,
                    human(&expected).as_bytes(),
                    "ssh-option-{index}"
                );
            }
        }
    }
    drop(stream);
    child.stop(libc::SIGTERM);
    assert_eq!(
        fixture.report()["metrics"]["db_outstanding"]["high_water"],
        0
    );
}

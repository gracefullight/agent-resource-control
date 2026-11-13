//! Deterministic authenticated transport plus actual CLI/native-child execution.
//! The downstream integration owner proves durable production-daemon semantics.
#![cfg(unix)]
#[path = "governor/support.rs"]
mod support;
use arc_ipc::governor::*;
use support::*;
#[allow(dead_code)]
#[path = "../../arc-ipc/tests/fixtures/memory_pressure.rs"]
mod fixture;
use serde_json::{json, Value};
use std::thread;

#[test]
fn desired_state_controls_and_json_readback_use_the_frozen_authenticated_contract() {
    let socket = Socket::new();
    let listener = socket.listener.try_clone().unwrap();
    let server = thread::spawn(move || {
        for enabled in [false, true] {
            let (mut stream, request) = next(&listener);
            assert_eq!(request.operation, OPERATION_GET_SWITCH);
            reply(
                &mut stream,
                &request,
                json!({"switch":state(!enabled, if enabled {5} else {4})}),
            );
            let (mut stream, request) = next(&listener);
            assert_eq!(
                request.operation,
                if enabled {
                    OPERATION_RESUME
                } else {
                    OPERATION_PAUSE
                }
            );
            let input = decode_set_switch(&request.operation, &request.payload).unwrap();
            assert_eq!(input.enabled, enabled);
            reply(
                &mut stream,
                &request,
                json!({"status":"COMMITTED","mutation_id":input.mutation_id,"switch":state(enabled, input.expected_revision+1),"changed":true,"replayed":false}),
            );
        }
        let (mut stream, request) = next(&listener);
        assert_eq!(request.operation, OPERATION_GET_SWITCH);
        reply(&mut stream, &request, json!({"switch":state(true, 6)}));
    });
    for action in ["pause", "resume"] {
        let mut command = socket.cli();
        command.args(["daemon", action, "--json"]);
        let result = output(command);
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let result: Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(result["status"], "COMMITTED");
        assert_eq!(result["switch"]["enabled"], action == "resume");
    }
    let mut command = socket.cli();
    command.args(["daemon", "governor", "--json"]);
    let result = output(command);
    assert!(result.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&result.stdout).unwrap(),
        json!({"switch":state(true,6)})
    );
    server.join().unwrap();
}

#[test]
fn lost_control_ack_reads_the_same_identity_once_without_replaying_a_write() {
    for receipt_exists in [false, true] {
        let socket = Socket::new();
        let listener = socket.listener.try_clone().unwrap();
        let id = format!("4-{}", "a".repeat(32));
        let expected_id = id.clone();
        let server = thread::spawn(move || {
            let (stream, request) = next(&listener);
            let input = decode_set_switch(OPERATION_PAUSE, &request.payload).unwrap();
            assert_eq!(input.mutation_id, expected_id);
            drop(stream);
            let (mut stream, request) = next(&listener);
            assert_eq!(request.operation, OPERATION_GET_SWITCH);
            assert_eq!(request.payload["mutation_id"], expected_id);
            let mut result = json!({"switch":state(false,5)});
            if receipt_exists {
                result["mutation_result"] = json!({"status":"COMMITTED","mutation_id":expected_id,"switch":state(false,5),"changed":true,"replayed":true});
            }
            reply(&mut stream, &request, result);
        });
        let mut command = socket.cli();
        command.args([
            "daemon",
            "pause",
            "--json",
            "--expected-revision",
            "4",
            "--mutation-id",
            &id,
        ]);
        let result = output(command);
        assert_eq!(result.status.success(), receipt_exists);
        assert!(String::from_utf8_lossy(&result.stderr).contains(&id));
        assert!(String::from_utf8_lossy(&result.stderr).contains("Change not confirmed"));
        server.join().unwrap();
    }
}

#[test]
fn replayed_pause_receipt_displays_current_state_after_another_client_resumes() {
    for json_output in [false, true] {
        let socket = Socket::new();
        let listener = socket.listener.try_clone().unwrap();
        let id = format!("4-{}", "d".repeat(32));
        let expected_id = id.clone();
        let server = thread::spawn(move || {
            let (mut stream, request) = next(&listener);
            assert_eq!(request.operation, OPERATION_PAUSE);
            let input = decode_set_switch(OPERATION_PAUSE, &request.payload).unwrap();
            assert_eq!(input.mutation_id, expected_id);
            let receipt = json!({"status":"COMMITTED","mutation_id":expected_id,"switch":state(false,5),"changed":true,"replayed":true});
            reply(&mut stream, &request, receipt.clone());
            let (mut stream, request) = next(&listener);
            assert_eq!(request.operation, OPERATION_GET_SWITCH);
            assert_eq!(request.payload["mutation_id"], expected_id);
            reply(
                &mut stream,
                &request,
                json!({"switch":state(true,6),"mutation_result":receipt}),
            );
        });
        let mut command = socket.cli();
        command.args([
            "daemon",
            "pause",
            "--expected-revision",
            "4",
            "--mutation-id",
            &id,
        ]);
        if json_output {
            command.arg("--json");
        }
        let result = output(command);
        assert!(result.status.success(), "{result:?}");
        if json_output {
            let value: Value = serde_json::from_slice(&result.stdout).unwrap();
            assert_eq!(value["switch"], state(true, 6));
            assert_eq!(value["mutation_result"]["switch"], state(false, 5));
            assert_eq!(value["mutation_result"]["replayed"], true);
        } else {
            let text = String::from_utf8(result.stdout).unwrap();
            assert!(text.contains("Active · revision 6"), "{text}");
            assert!(!text.contains("Paused · revision 5"), "{text}");
        }
        server.join().unwrap();
        socket.listener.set_nonblocking(true).unwrap();
        assert_eq!(
            socket.listener.accept().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock,
            "a replay must only read, never resend or rebase the mutation"
        );
    }
}

#[test]
fn conflict_is_a_failed_control_and_invalid_revision_identity_never_dispatches() {
    let socket = Socket::new();
    let listener = socket.listener.try_clone().unwrap();
    let id = format!("4-{}", "b".repeat(32));
    let server = thread::spawn(move || {
        let (mut stream, request) = next(&listener);
        let input = decode_set_switch(OPERATION_PAUSE, &request.payload).unwrap();
        reply(
            &mut stream,
            &request,
            json!({"status":"CONFLICT","mutation_id":input.mutation_id,"switch":state(false,5),"reason":"REVISION_CONFLICT"}),
        );
    });
    let mut command = socket.cli();
    command.args([
        "daemon",
        "pause",
        "--json",
        "--expected-revision",
        "4",
        "--mutation-id",
        &id,
    ]);
    let result = output(command);
    assert!(!result.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&result.stdout).unwrap()["status"],
        "CONFLICT"
    );
    server.join().unwrap();
    let mut command = socket.cli();
    command.args([
        "daemon",
        "pause",
        "--expected-revision",
        "5",
        "--mutation-id",
        &id,
    ]);
    let result = output(command);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("INVALID_REQUEST"));
    socket.listener.set_nonblocking(true).unwrap();
    assert_eq!(
        socket.listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[path = "governor/execution.rs"]
mod execution;

#[cfg(target_os = "macos")]
#[allow(dead_code)] // Reuse the maintained owned-process/coordination fixture ports.
#[path = "../../arc-daemon/tests/support/native_fixture.rs"]
mod native;
#[cfg(target_os = "macos")]
#[allow(dead_code)]
#[path = "../../arc-daemon/tests/support/native_exec.rs"]
mod native_process;
#[cfg(target_os = "macos")]
#[path = "governor/native_recovery.rs"]
mod native_recovery;
#[cfg(target_os = "macos")]
#[path = "../../arc-daemon/tests/support/mod.rs"]
mod serialization;

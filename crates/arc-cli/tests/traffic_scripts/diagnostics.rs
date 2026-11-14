use crate::support::*;
use arc_ipc::traffic::{self, diagnostics::*};
use serde_json::{json, Value};
use std::thread;

fn wire() -> Value {
    serde_json::from_str(include_str!(
        "../../../../apps/desktop/tests/fixtures/traffic-scripts-wire.json"
    ))
    .unwrap()
}

#[test]
fn optional_cli_diagnostics_preserve_negotiated_typed_facts_and_freshness() {
    for result in wire()["results"].as_array().unwrap() {
        for json_output in [false, true] {
            let socket = Socket::new();
            let listener = socket.listener.try_clone().unwrap();
            let expected = result.clone();
            let observed = expected.clone();
            let id = result["job_id"].as_str().unwrap();
            let server = thread::spawn(move || {
                let (mut stream, request) = next(&listener);
                assert_eq!(request.operation, OPERATION_CAPABILITIES);
                assert!(request.payload.is_null());
                reply(
                    &mut stream,
                    &request,
                    json!(traffic::Capabilities {
                        capability: CAPABILITY.into()
                    }),
                );
                let (mut stream, request) = next(&listener);
                assert_eq!(request.operation, OPERATION_JOB);
                assert_eq!(request.payload, json!({"job_id":observed["job_id"]}));
                reply(&mut stream, &request, observed);
            });
            let mut command = socket.cli();
            command.args(["job", "diagnostics", id]);
            if json_output {
                command.arg("--json");
            }
            let result = output(command);
            server.join().unwrap();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            if json_output {
                assert_eq!(
                    serde_json::from_slice::<Value>(&result.stdout).unwrap(),
                    expected
                );
            } else {
                let text = String::from_utf8(result.stdout).unwrap();
                assert!(text.contains("Last known: true\n"));
                assert!(text.contains("Liveness refreshed: false\n"));
                if expected["classification"].is_null() {
                    assert!(text.contains("Classification: not recorded\n"));
                    assert!(text.contains("Admission: not recorded\n"));
                } else {
                    assert!(text.contains("Classification: TYPECHECK\n"));
                    assert!(text.contains("Classification reason: BUN_SCRIPT_TYPECHECK\n"));
                    assert!(text.contains(&format!(
                        "Admission: {}\n",
                        expected["admission"]["state"].as_str().unwrap()
                    )));
                }
            }
        }
    }
}

#[test]
fn unsupported_unavailable_invalid_and_mismatched_reads_never_become_not_recorded_facts() {
    for mode in [
        "unsupported-probe",
        "unsupported-read",
        "unavailable",
        "wrong-id",
        "invalid-freshness",
        "wrong-capability",
    ] {
        let socket = Socket::new();
        let listener = socket.listener.try_clone().unwrap();
        let id = wire()["queries"][0]["job_id"].as_str().unwrap().to_string();
        let server = thread::spawn(move || {
            let (mut stream, request) = next(&listener);
            if mode == "unsupported-probe" {
                super::execution::unsupported(&mut stream, &request);
                return;
            }
            if mode == "unavailable" {
                return;
            }
            reply(
                &mut stream,
                &request,
                if mode == "wrong-capability" {
                    json!(traffic::Capabilities::supported())
                } else {
                    json!({"capability":CAPABILITY})
                },
            );
            if mode == "wrong-capability" {
                return;
            }
            let (mut stream, request) = next(&listener);
            if mode == "unsupported-read" {
                super::execution::unsupported(&mut stream, &request);
                return;
            }
            let mut result = wire()["results"][0].clone();
            if mode == "wrong-id" {
                result["job_id"] = json!("a".repeat(32));
            } else {
                result["liveness_fresh"] = json!(true);
            }
            reply(&mut stream, &request, result);
        });
        let mut command = socket.cli();
        command.args(["job", "diagnostics", &id, "--json"]);
        let result = output(command);
        server.join().unwrap();
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&result.stderr).contains("not recorded"));
    }
}

#[test]
fn queue_causes_remain_historical_and_keep_class_cpu_and_memory_distinct() {
    for reason in ["class_window", "cpu_budget", "memory_budget", "fifo"] {
        let socket = Socket::new();
        let listener = socket.listener.try_clone().unwrap();
        let mut result = wire()["results"][0].clone();
        result["admission"]["wait_reason"] = json!(reason);
        result["admission"]["last_decision"] = json!({
            "policy_version":arc_ipc::policy::POLICY_VERSION,"kind":"wait","reason":reason,
            "decided_at_ms":10,"permit_generation":null,"observation":crate::fixture::fallback().observation
        });
        let id = result["job_id"].as_str().unwrap().to_string();
        let server = thread::spawn(move || {
            let (mut stream, request) = next(&listener);
            reply(&mut stream, &request, json!({"capability":CAPABILITY}));
            let (mut stream, request) = next(&listener);
            reply(&mut stream, &request, result);
        });
        let mut command = socket.cli();
        command.args(["job", "diagnostics", &id]);
        let result = output(command);
        server.join().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(String::from_utf8_lossy(&result.stdout)
            .contains(&format!("Last recorded queue cause: {reason}\n")));
    }
}

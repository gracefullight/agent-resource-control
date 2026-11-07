//! Shared literal fixtures for detector, IPC and real-daemon/CLI checks.
use serde_json::{json, Value};

#[path = "ssh.rs"]
mod ssh;
pub use ssh::fixtures as ssh_fixtures;

pub fn fixtures() -> Vec<Value> {
    let contract: Value = serde_json::from_str(include_str!(
        "../../../../.arc-verify/command-classification/contract.json"
    ))
    .unwrap();
    let mut cases = contract["fixtures"].as_array().unwrap().clone();
    for source in contract["dispositionFixtures"].as_array().unwrap() {
        let mut case = source.clone();
        if let Some(generator) = source.get("generator") {
            // Explicit generators only: never evaluate an expression or shell text.
            let command = match generator["expression"].as_str().unwrap() {
                "'cargo test ' + 'a' * 16374" => format!("cargo test {}", "a".repeat(16374)),
                "'cargo test ' + '가' * 5458" => format!("cargo test {}", "가".repeat(5458)),
                "' ' * 16385" => " ".repeat(16385),
                _ => panic!("unrecognized disposition fixture generator"),
            };
            assert_eq!(
                command.len(),
                generator["utf8Bytes"].as_u64().unwrap() as usize
            );
            case["payload"] = json!({"command":command});
        }
        cases.push(case);
    }
    // Pin the amended precedence rules without changing the input artifacts.
    for (id, command, reason) in [
        (
            "empty-unsupported-shell",
            String::new(),
            "UNSUPPORTED_SHELL",
        ),
        (
            "oversized-unsupported-shell",
            " ".repeat(16385),
            "INPUT_LIMIT",
        ),
    ] {
        cases.push(
            json!({"id":id,"payload":{"command":command,"shell_kind":"powershell"},
            "result":{"classification":"UNKNOWN","locality":"UNKNOWN",
            "reason":reason,"uncertainty":"OBSERVE"}}),
        );
    }
    // Exact CLI matrix literals supplement equivalent original contract cases.
    for (index, (command, class, locality, reason)) in [
        ("  cargo test  ", "TEST", "HOST_LOCAL", "CARGO_TEST"),
        ("cargo\ttest", "TEST", "HOST_LOCAL", "CARGO_TEST"),
        (
            "cargo test -- \"검증 λ\"",
            "TEST",
            "HOST_LOCAL",
            "CARGO_TEST",
        ),
        ("gh run view", "REMOTE_CI", "REMOTE", "GH_RUN_VIEW"),
        (
            "ssh build01 'cargo test'",
            "REMOTE_EXEC",
            "REMOTE",
            "SSH_REMOTE_COMMAND",
        ),
        (
            "(cargo build && cargo test) && gh run watch",
            "MIXED",
            "MIXED",
            "COMPOUND_MIXED",
        ),
        (
            "cargo test -- \"gh workflow run ci.yml\"",
            "TEST",
            "HOST_LOCAL",
            "CARGO_TEST",
        ),
        (
            "cargo test -- \"ssh host cargo build\"",
            "TEST",
            "HOST_LOCAL",
            "CARGO_TEST",
        ),
        (
            "printf '%s' 'cargo build && gh workflow run ci.yml'",
            "UNKNOWN",
            "UNKNOWN",
            "UNRESOLVED_COMMAND",
        ),
        ("./verify.sh", "UNKNOWN", "UNKNOWN", "UNRESOLVED_SCRIPT"),
        ("npm run verify", "UNKNOWN", "UNKNOWN", "UNRESOLVED_SCRIPT"),
        ("$TOOL test", "UNKNOWN", "UNKNOWN", "UNRESOLVED_EXPANSION"),
        ("--json", "UNKNOWN", "UNKNOWN", "UNRESOLVED_COMMAND"),
        ("--shell", "UNKNOWN", "UNKNOWN", "UNRESOLVED_COMMAND"),
    ]
    .into_iter()
    .enumerate()
    {
        cases.push(
            json!({"id":format!("cli-literal-{index}"),"payload":{"command":command},
            "result":{"classification":class,"locality":locality,"reason":reason,
            "uncertainty":if class == "UNKNOWN" { "OBSERVE" } else { "NONE" }}}),
        );
    }
    cases
}

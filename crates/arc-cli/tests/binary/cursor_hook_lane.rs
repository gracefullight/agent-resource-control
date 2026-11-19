//! Cursor lane: recorded contract evidence and the built hook's response.
//!
//! The verified-dispatch tests below use a controlled fixture surface; the
//! operator's separate execution-surface recording supports production wiring.
use super::{hook_scheduled::FixtureChild, hooks};
use serde_json::{json, Value};
use std::{
    fs,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

const CARGO_EVENT: &str =
    include_str!("../fixtures/agent_hooks/cursor/recorded/preToolUse-cargo-build-rewrite.json");
const BUN_EVENT: &str =
    include_str!("../fixtures/agent_hooks/cursor/recorded/preToolUse-bun-run-build-rewrite.json");
const CARGO_RESPONSE: &str =
    include_str!("../fixtures/agent_hooks/cursor/recorded/response-cargo-build-rewrite.json");
const BUN_RESPONSE: &str =
    include_str!("../fixtures/agent_hooks/cursor/recorded/response-bun-run-build-rewrite.json");
const CARGO_BEFORE_SHELL: &str = include_str!(
    "../fixtures/agent_hooks/cursor/recorded/beforeShellExecution-cargo-build-rewrite.json"
);
const BUN_BEFORE_SHELL: &str = include_str!(
    "../fixtures/agent_hooks/cursor/recorded/beforeShellExecution-bun-run-build-rewrite.json"
);
const CARGO_RESULT: &str =
    include_str!("../fixtures/agent_hooks/cursor/recorded/run-cargo-build-rewrite.result.json");
const BUN_RESULT: &str =
    include_str!("../fixtures/agent_hooks/cursor/recorded/run-bun-run-build-rewrite.result.json");

struct Case {
    literal: &'static str,
    event: &'static str,
    response: &'static str,
    before_shell: &'static str,
    result: &'static str,
}

fn cases() -> [Case; 2] {
    [
        Case {
            literal: "cargo build",
            event: CARGO_EVENT,
            response: CARGO_RESPONSE,
            before_shell: CARGO_BEFORE_SHELL,
            result: CARGO_RESULT,
        },
        Case {
            literal: "bun run build",
            event: BUN_EVENT,
            response: BUN_RESPONSE,
            before_shell: BUN_BEFORE_SHELL,
            result: BUN_RESULT,
        },
    ]
}

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = PathBuf::from("/tmp").join(format!(
            "arc-cursor-lane-{}",
            arc_core::process_identity::random_job_id().unwrap()
        ));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let bin = root.join("bin");
        fs::create_dir(&bin).unwrap();
        for name in ["cargo", "bun"] {
            let executable = bin.join(name);
            fs::write(&executable, "#!/bin/sh\nprintf executed > \"$MARKER\"\n").unwrap();
            fs::set_permissions(executable, fs::Permissions::from_mode(0o755)).unwrap();
        }
        Self(root)
    }

    fn run_hook(&self, input: &[u8], extra_env: &[(&str, &str)]) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_arcctl"));
        command
            .args(["hooks", "run", "cursor"])
            .env_clear()
            .env("HOME", &self.0)
            .env("PATH", self.0.join("bin"))
            .env("MARKER", self.0.join("executed"))
            .current_dir(&self.0)
            .stdin(Stdio::piped());
        if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
            command.env("LLVM_PROFILE_FILE", profile);
        }
        for (key, value) in extra_env {
            command.env(key, value);
        }
        let mut child = FixtureChild::spawn(command, &self.0);
        child.write_input(input);
        child.output()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("retained failing Cursor lane fixture {}", self.0.display());
        } else {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
}

fn json_value(text: &str) -> Value {
    serde_json::from_str(text).unwrap()
}

fn assert_fail_open(output: Output, label: &str) {
    assert_eq!(output.status.code(), Some(1), "{label}: {output:?}");
    assert!(output.stdout.is_empty(), "{label}: {output:?}");
    assert!(output.stderr.is_empty(), "{label}: {output:?}");
}

#[test]
fn recorded_cursor_session_correlates_input_response_and_later_shell_event() {
    for case in cases() {
        let event = json_value(case.event);
        let response = json_value(case.response);
        let later = json_value(case.before_shell);
        let result = json_value(case.result);
        assert_eq!(event["cursor_version"], "2026.11.13-eacade1");
        assert_eq!(later["cursor_version"], event["cursor_version"]);
        assert_eq!(event["hook_event_name"], "preToolUse");
        assert_eq!(event["tool_name"], "Shell");
        assert_eq!(event["tool_input"]["command"], case.literal);
        assert!(response.get("permission").is_none());
        let mut expected_input = event["tool_input"].clone();
        expected_input["command"] = response["updated_input"]["command"].clone();
        assert_eq!(response, json!({"updated_input": expected_input}));
        assert_eq!(later["hook_event_name"], "beforeShellExecution");
        assert_eq!(later["tool_use_id"], event["tool_use_id"]);
        assert_eq!(later["command"], response["updated_input"]["command"]);
        assert_eq!(result["subtype"], "success");
        assert!(result["result"]
            .as_str()
            .unwrap()
            .contains(&format!("ARC_REWRITE_ACCEPTED original={}", case.literal)));
    }
}

#[test]
fn recorded_inputs_and_mutations_preserve_complete_tool_input_at_fixture_seam() {
    let fixture = Fixture::new();
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "cursor_hook_lane::cursor_fixture_seam_worker",
            "--nocapture",
        ])
        .env_clear()
        .env("HOME", &fixture.0)
        .env("PATH", fixture.0.join("bin"))
        .stdin(Stdio::null());
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile);
    }
    let _handshake = hooks::fixture::worker_handshake(&mut command);
    let output = FixtureChild::spawn(command, &fixture.0).output();
    assert!(output.status.success(), "fixture seam worker: {output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("ARC_CURSOR_FIXTURE_SEAM_COMPLETE"),
        "{stdout}"
    );
    assert!(stdout.contains("test result: ok. 1 passed;"), "{stdout}");
}

#[test]
fn cursor_fixture_seam_worker() {
    if !hooks::fixture::worker_enabled() {
        return;
    }
    let fixture = Fixture::new();
    let surface = hooks::VerifiedSurface;
    let context = hooks::HookContext {
        executable: Some(Path::new(env!("CARGO_BIN_EXE_arcctl")).to_path_buf()),
    };
    for case in cases() {
        let recorded = json_value(case.event);
        let mut mutation = recorded.clone();
        mutation["tool_input"]["future_field"] = json!({
            "keep": [null, false, {"text": "quote ' and \n newline", "integer": u64::MAX}]
        });
        // The second event is a derived mutation, never a recorded payload.
        for event in [recorded, mutation] {
            let bytes = serde_json::to_vec(&event).unwrap();
            let response =
                hooks::dispatch_verified_event(hooks::Vendor::Cursor, &bytes, &context, &surface)
                    .expect("controlled fixture surface should transform the exact literal");
            assert!(response.get("permission").is_none());
            assert_eq!(response.as_object().unwrap().len(), 1);
            let mut expected = event["tool_input"].clone();
            let rewritten = response["updated_input"]["command"].as_str().unwrap();
            let binary = env!("CARGO_BIN_EXE_arcctl").replace('\'', "'\\''");
            assert_eq!(
                rewritten,
                format!("'{binary}' hook-exec -- {}", case.literal)
            );
            expected["command"] = json!(rewritten);
            assert_eq!(response["updated_input"], expected);
        }
    }
    let mut unsupported = json_value(CARGO_EVENT);
    for literal in ["cargo build --release", "bun run build | cat"] {
        unsupported["tool_input"]["command"] = json!(literal);
        assert!(hooks::dispatch_verified_event(
            hooks::Vendor::Cursor,
            &serde_json::to_vec(&unsupported).unwrap(),
            &context,
            &surface,
        )
        .is_none());
    }
    let duplicate = CARGO_EVENT.replace(
        "\"command\": \"cargo build\"",
        "\"command\": \"cargo build\", \"command\": \"bun run build\"",
    );
    assert_ne!(duplicate, CARGO_EVENT);
    let decimal = CARGO_EVENT.replace("\"timeout\": 30000", "\"timeout\": 1.25");
    assert_ne!(decimal, CARGO_EVENT);
    for invalid in ["{", duplicate.as_str()] {
        assert!(hooks::dispatch_verified_event(
            hooks::Vendor::Cursor,
            invalid.as_bytes(),
            &context,
            &surface,
        )
        .is_none());
    }
    let response = hooks::dispatch_verified_event(
        hooks::Vendor::Cursor,
        decimal.as_bytes(),
        &context,
        &surface,
    )
    .unwrap();
    assert_eq!(response["updated_input"]["timeout"], json!(1.25));
    assert!(!fixture.0.join("executed").exists());
    println!("ARC_CURSOR_FIXTURE_SEAM_COMPLETE");
}

#[test]
fn built_cursor_hook_rewrites_both_recorded_literals_without_a_permission_decision() {
    let fixture = Fixture::new();
    for case in cases() {
        let event = json_value(case.event);
        let output = fixture.run_hook(case.event.as_bytes(), &[("PATH", "/nonexistent")]);
        assert_eq!(output.status.code(), Some(0));
        assert!(output.stderr.is_empty());
        let response: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(response.get("permission").is_none());
        let mut input = response["updated_input"].clone();
        assert!(input["command"]
            .as_str()
            .unwrap()
            .ends_with(&format!(" hook-exec -- {}", case.literal)));
        input["command"] = json!(case.literal);
        assert_eq!(input, event["tool_input"]);
        let mutated = case.event.replacen(
            "\"tool_input\": {",
            "\"tool_input\": {\"precise\":0.12345678901234567890123456789,",
            1,
        );
        assert_ne!(mutated, case.event);
        let output = fixture.run_hook(mutated.as_bytes(), &[]);
        assert_eq!(output.status.code(), Some(0));
        assert!(String::from_utf8(output.stdout)
            .unwrap()
            .contains("0.12345678901234567890123456789"));
    }
    assert!(!fixture.0.join("executed").exists());
}

#[test]
fn built_cursor_hook_preserves_unbounded_json_numbers_and_rejects_duplicates() {
    let fixture = Fixture::new();
    for source in [CARGO_EVENT, BUN_EVENT] {
        // These are mutations of the recorded input, not new recordings.
        let mutated = source.replacen('{', "{\"outer\":1e400,", 1).replacen(
            "\"tool_input\": {",
            "\"tool_input\": {\"numeric\":1e400,\"nested\":[-1e999,184467440737095516161],",
            1,
        );
        assert_ne!(mutated, source);
        let output = fixture.run_hook(mutated.as_bytes(), &[]);
        assert_eq!(output.status.code(), Some(0));
        assert!(output.stderr.is_empty());
        let response = String::from_utf8(output.stdout).unwrap();
        assert!(response.contains("\"numeric\":1e400"), "{response}");
        assert!(response.contains("\"nested\":[-1e999,184467440737095516161]"));
        assert!(response.contains("hook-exec --"));
        assert!(!response.contains("\"permission\""));
        let duplicate = mutated.replacen(
            "\"numeric\":1e400",
            "\"numeric\":1e400,\"nested_duplicate\":{\"x\":1e400,\"x\":2}",
            1,
        );
        assert_fail_open(fixture.run_hook(duplicate.as_bytes(), &[]), "duplicate");
        let invalid = mutated.replacen("1e400", "1e+", 1);
        assert_fail_open(fixture.run_hook(invalid.as_bytes(), &[]), "invalid number");
    }
}

#[test]
fn built_cursor_hook_fails_open_for_unsupported_events_and_errors() {
    let fixture = Fixture::new();
    let base = json_value(CARGO_EVENT);
    for literal in [
        "cargo  build",
        "cargo build --release",
        "cargo build; true",
        "bun run build\n",
        "bun run build | cat",
        "arcctl hook-exec -- cargo build",
    ] {
        let mut mutation = base.clone();
        mutation["tool_input"]["command"] = json!(literal);
        assert_fail_open(
            fixture.run_hook(&serde_json::to_vec(&mutation).unwrap(), &[]),
            literal,
        );
    }
    for (field, value) in [
        ("hook_event_name", json!("postToolUse")),
        ("tool_name", json!("Read")),
        ("tool_input", json!({"command": null})),
        ("tool_input", json!([])),
    ] {
        let mut mutation = base.clone();
        mutation[field] = value;
        assert_fail_open(
            fixture.run_hook(&serde_json::to_vec(&mutation).unwrap(), &[]),
            field,
        );
    }
    for raw in [
        &b"{"[..],
        &b"null"[..],
        &b"{\"tool_input\":{\"command\":\"cargo build\",\"command\":\"bun run build\"}}"[..],
        &b"{\"tool_input\":{\"command\":\"cargo build\",\"number\":1.25}}"[..],
        &b"{\"tool_input\":{\"command\":\"cargo build\",\"number\":18446744073709551616}}"[..],
    ] {
        assert_fail_open(fixture.run_hook(raw, &[]), "invalid input");
    }
    for (key, value) in [
        ("ARC_HOOKS_DISABLED", "1"),
        ("ARC_HOOKS_CURSOR_DISABLED", "1"),
        ("CURSOR_CODE_REMOTE", "true"),
        ("SSH_CONNECTION", "remote"),
    ] {
        assert_fail_open(
            fixture.run_hook(CARGO_EVENT.as_bytes(), &[(key, value)]),
            key,
        );
    }
    assert_fail_open(
        fixture.run_hook(&vec![b' '; 1024 * 1024 + 1], &[]),
        "oversized input",
    );
    assert!(!fixture.0.join("executed").exists());
}

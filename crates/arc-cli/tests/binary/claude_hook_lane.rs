//! Claude recordings and the production hook response.
//!
//! The two included files are unmodified, sanitized operator recordings. Any
//! JSON changed inside a test is a mutation, not another vendor recording.
use super::{hook_scheduled::FixtureChild, hooks};
use serde_json::{json, Value};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Command, Output, Stdio},
};

const CARGO_EVENT: &str =
    include_str!("../fixtures/agent_hooks/claude-recorded-2.1.327-cargo-build.json");
const BUN_EVENT: &str =
    include_str!("../fixtures/agent_hooks/claude-recorded-2.1.327-bun-run-build.json");

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = PathBuf::from("/tmp").join(format!(
            "arc-claude-lane-{}",
            arc_core::process_identity::random_job_id().unwrap()
        ));
        fs::create_dir(&root).unwrap();
        for program in ["cargo", "bun"] {
            let path = root.join(program);
            // The random fixture path contains no shell metacharacters. Embed
            // the witness so an accidental invocation with an empty environment
            // is still recorded.
            fs::write(
                &path,
                format!(
                    "#!/bin/sh\nprintf x >> '{}'\n",
                    root.join("executed").display()
                ),
            )
            .unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        Self(root)
    }

    fn hook(&self, input: &[u8], extra_env: &[(&str, &str)]) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_arcctl"));
        command
            .args(["hooks", "run", "claude"])
            .env_clear()
            .env("PATH", &self.0)
            .env("HOME", &self.0)
            .env("EXECUTION_MARKER", self.0.join("executed"))
            .current_dir(&self.0)
            .stdin(Stdio::piped());
        for (name, value) in extra_env {
            command.env(name, value);
        }
        if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
            command.env("LLVM_PROFILE_FILE", profile);
        }
        let mut child = FixtureChild::spawn(command, &self.0);
        child.write_input(input);
        child.output()
    }

    fn assert_hook_passthrough(&self, input: &[u8], env: &[(&str, &str)]) {
        let output = self.hook(input, env);
        assert_eq!(output.status.code(), Some(0));
        assert!(output.stdout.is_empty(), "{:?}", output.stdout);
        assert!(output.stderr.is_empty(), "{:?}", output.stderr);
        assert!(!self.0.join("executed").exists(), "hook ran a build");
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }
}

#[test]
fn production_hook_rewrites_recordings_and_passes_other_inputs_without_execution() {
    let fixture = Fixture::new();
    for source in [CARGO_EVENT, BUN_EVENT] {
        let event: Value = serde_json::from_str(source).unwrap();
        let output = fixture.hook(source.as_bytes(), &[("PATH", "/nonexistent")]);
        assert_eq!(output.status.code(), Some(0));
        assert!(output.stderr.is_empty());
        let response: Value = serde_json::from_slice(&output.stdout).unwrap();
        let specific = &response["hookSpecificOutput"];
        assert!(specific.get("permissionDecision").is_none());
        let mut input = specific["updatedInput"].clone();
        let literal = event["tool_input"]["command"].as_str().unwrap();
        assert!(input["command"]
            .as_str()
            .unwrap()
            .ends_with(&format!(" hook-exec -- {literal}")));
        input["command"] = json!(literal);
        assert_eq!(input, event["tool_input"]);
        assert!(!fixture.0.join("executed").exists());
        let mutated = source.replacen(
            "\"tool_input\": {",
            "\"tool_input\": {\"precise\":0.12345678901234567890123456789,",
            1,
        );
        assert_ne!(mutated, source);
        let output = fixture.hook(mutated.as_bytes(), &[]);
        assert_eq!(output.status.code(), Some(0));
        assert!(String::from_utf8(output.stdout)
            .unwrap()
            .contains("0.12345678901234567890123456789"));
        for setting in [
            ("ARC_HOOKS_DISABLED", "1"),
            ("ARC_HOOKS_CLAUDE_DISABLED", "1"),
            ("CLAUDE_CODE_REMOTE", "true"),
            ("SSH_CONNECTION", "remote"),
        ] {
            fixture.assert_hook_passthrough(source.as_bytes(), &[setting]);
        }
        // These are deliberate mutations of the recorded payload.
        for command in [
            "cargo build --release",
            "cargo  build",
            "bun run build; true",
            "'/arcctl' hook-exec -- cargo build",
        ] {
            let mut event: Value = serde_json::from_str(source).unwrap();
            event["tool_input"]["command"] = json!(command);
            fixture.assert_hook_passthrough(&serde_json::to_vec(&event).unwrap(), &[]);
        }
    }
    for invalid in [
        &b"{"[..],
        &b"[]"[..],
        &b"{\"tool_input\":{\"command\":\"cargo build\",\"command\":\"bun run build\"}}"[..],
        &b"{\"tool_input\":{\"command\":\"cargo build\",\"timeout\":1.1}}"[..],
    ] {
        fixture.assert_hook_passthrough(invalid, &[]);
    }
}

#[test]
fn built_claude_hook_preserves_unbounded_json_numbers_and_rejects_duplicates() {
    let fixture = Fixture::new();
    for source in [CARGO_EVENT, BUN_EVENT] {
        // These are mutations of the recorded input, not new recordings.
        let mutated = source.replacen('{', "{\"outer\":1e400,", 1).replacen(
            "\"tool_input\": {",
            "\"tool_input\": {\"numeric\":1e400,\"nested\":[-1e999,184467440737095516161],",
            1,
        );
        assert_ne!(mutated, source);
        let output = fixture.hook(mutated.as_bytes(), &[]);
        assert_eq!(output.status.code(), Some(0));
        assert!(output.stderr.is_empty());
        let response = String::from_utf8(output.stdout).unwrap();
        assert!(response.contains("\"numeric\":1e400"), "{response}");
        assert!(response.contains("\"nested\":[-1e999,184467440737095516161]"));
        assert!(response.contains("hook-exec --"));
        assert!(!response.contains("permissionDecision"));
        let duplicate = mutated.replacen(
            "\"numeric\":1e400",
            "\"numeric\":1e400,\"nested_duplicate\":{\"x\":1e400,\"x\":2}",
            1,
        );
        fixture.assert_hook_passthrough(duplicate.as_bytes(), &[]);
        let invalid = mutated.replacen("1e400", "1e+", 1);
        fixture.assert_hook_passthrough(invalid.as_bytes(), &[]);
    }
}

#[test]
fn controlled_surface_response_and_disable_flags() {
    // Environment switches are exercised in isolated, authenticated workers;
    // mutating this test process's environment would race other Rust tests.
    for (case, setting) in [
        ("enabled", None),
        ("disabled", Some(("ARC_HOOKS_DISABLED", "1"))),
        ("disabled", Some(("ARC_HOOKS_CLAUDE_DISABLED", "1"))),
        ("disabled", Some(("CLAUDE_CODE_REMOTE", "true"))),
    ] {
        let fixture = Fixture::new();
        let output_path = fixture.0.join("worker-output");
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "claude_hook_lane::controlled_surface_worker",
                "--nocapture",
            ])
            .env_clear()
            .env("CLAUDE_LANE_CASE", case)
            .stdout(Stdio::from(fs::File::create(&output_path).unwrap()));
        if let Some((name, value)) = setting {
            command.env(name, value);
        }
        if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
            command.env("LLVM_PROFILE_FILE", profile);
        }
        let _handshake = hooks::fixture::worker_handshake(&mut command);
        let mut child = hooks::fixture::OwnedChild::spawn(&mut command, "Claude lane worker");
        assert!(child.wait().success(), "case={case}, setting={setting:?}");
        let output = fs::read_to_string(output_path).unwrap();
        assert!(
            output.contains("ARC_CLAUDE_SURFACE_WORKER_COMPLETE"),
            "{output}"
        );
        assert!(output.contains("test result: ok. 1 passed;"), "{output}");
    }
}

#[test]
fn controlled_surface_worker() {
    if !hooks::fixture::worker_enabled() {
        return;
    }
    let enabled = std::env::var("CLAUDE_LANE_CASE").unwrap() == "enabled";
    let fixture = Fixture::new();
    let context = hooks::HookContext {
        executable: Some(PathBuf::from(env!("CARGO_BIN_EXE_arcctl"))),
    };
    let surface = hooks::VerifiedSurface;
    for (source, literal) in [(CARGO_EVENT, "cargo build"), (BUN_EVENT, "bun run build")] {
        let recorded: Value = serde_json::from_str(source).unwrap();
        assert_eq!(recorded["tool_input"]["command"], literal);
        // Mutate a clone to test preservation of fields beyond the recording.
        let mut event = recorded.clone();
        event["tool_input"]["future_field"] = json!({"nested": [u64::MAX, true, null, "kept"]});
        let response = hooks::fixture::claude::handle_verified(&event, &context, Some(&surface));
        if !enabled {
            assert!(response.is_none());
            continue;
        }
        let recorded_response = hooks::dispatch_verified_event(
            hooks::Vendor::Claude,
            source.as_bytes(),
            &context,
            &surface,
        )
        .unwrap();
        let mut recorded_input = recorded_response["hookSpecificOutput"]["updatedInput"].clone();
        recorded_input["command"] = json!(literal);
        assert_eq!(recorded_input, recorded["tool_input"]);
        assert!(recorded_response["hookSpecificOutput"]
            .get("permissionDecision")
            .is_none());
        let response = response.unwrap();
        let specific = &response["hookSpecificOutput"];
        assert_eq!(specific["hookEventName"], "PreToolUse");
        assert!(specific.get("permissionDecision").is_none());
        assert_eq!(response.as_object().unwrap().len(), 1);
        assert_eq!(specific.as_object().unwrap().len(), 2);
        let mut updated = specific["updatedInput"].clone();
        let quoted = env!("CARGO_BIN_EXE_arcctl").replace('\'', "'\\''");
        assert_eq!(
            updated["command"],
            format!("'{quoted}' hook-exec -- {literal}")
        );
        let rewritten = updated["command"].clone();
        updated["command"] = json!(literal);
        assert_eq!(updated, event["tool_input"]);

        event["tool_input"]["command"] = rewritten;
        assert!(
            hooks::fixture::claude::handle_verified(&event, &context, Some(&surface)).is_none()
        );
        assert!(hooks::fixture::claude::handle_verified(&recorded, &context, None).is_none());
        let unavailable = hooks::HookContext { executable: None };
        assert!(
            hooks::fixture::claude::handle_verified(&recorded, &unavailable, Some(&surface))
                .is_none()
        );

        for (field, value) in [
            ("hook_event_name", json!("PostToolUse")),
            ("tool_name", json!("Read")),
            ("cwd", json!("relative")),
        ] {
            let mut altered = recorded.clone();
            altered[field] = value;
            assert!(
                hooks::fixture::claude::handle_verified(&altered, &context, Some(&surface))
                    .is_none()
            );
        }
        let mut unsupported = recorded.clone();
        unsupported["tool_input"]["command"] = json!(format!("{literal} --release"));
        assert!(
            hooks::fixture::claude::handle_verified(&unsupported, &context, Some(&surface))
                .is_none()
        );

        for extra in [
            r#""duplicate":1,"duplicate":2,"#,
            r#""duplicate":1,"\u0064uplicate":2,"#,
        ] {
            let raw = source.replacen(
                "\"tool_input\": {",
                &format!("\"tool_input\": {{{extra}"),
                1,
            );
            assert_ne!(raw, source);
            assert!(hooks::dispatch_verified_event(
                hooks::Vendor::Claude,
                raw.as_bytes(),
                &context,
                &surface,
            )
            .is_none());
        }
        for extra in [r#""fraction":1.1,"#, r#""too_large":18446744073709551616,"#] {
            let raw = source.replacen(
                "\"tool_input\": {",
                &format!("\"tool_input\": {{{extra}"),
                1,
            );
            let response = hooks::dispatch_verified_event(
                hooks::Vendor::Claude,
                raw.as_bytes(),
                &context,
                &surface,
            )
            .unwrap();
            let mut input = response["hookSpecificOutput"]["updatedInput"].clone();
            input["command"] = json!(literal);
            let expected: Value = serde_json::from_str(&raw).unwrap();
            assert_eq!(input, expected["tool_input"]);
        }
    }
    assert!(
        !fixture.0.join("executed").exists(),
        "transformation ran a build"
    );
    println!("ARC_CLAUDE_SURFACE_WORKER_COMPLETE");
}

#[test]
fn documented_shell_preamble_can_define_functions_outside_the_recorded_surface() {
    let fixture = Fixture::new();
    let preamble = fixture.0.join("claude-env-file");
    fs::write(
        &preamble,
        "cargo() { printf 'cargo:%s\\n' \"$*\" >> \"$FUNCTION_WITNESS\"; printf 'cargo function\\n'; return 23; }\n\
         bun() { printf 'bun:%s\\n' \"$*\" >> \"$FUNCTION_WITNESS\"; printf 'bun function\\n'; return 24; }\n",
    )
    .unwrap();
    for (source, expected, status) in [
        (CARGO_EVENT, "cargo function\n", 23),
        (BUN_EVENT, "bun function\n", 24),
    ] {
        let hook = fixture.hook(source.as_bytes(), &[]);
        assert!(hook.status.success());
        assert!(!hook.stdout.is_empty());
        let event: Value = serde_json::from_str(source).unwrap();
        let literal = event["tool_input"]["command"].as_str().unwrap();
        // Simulate a direct shell preamble; aliases/functions are an accepted
        // limit of the recorded PATH-executable surface.
        let mut shell = Command::new("/bin/bash");
        shell
            .args(["-c", &format!(". \"$CLAUDE_PRELUDE\"\n{literal}")])
            .env_clear()
            .env("PATH", &fixture.0)
            .env("HOME", &fixture.0)
            .env("CLAUDE_PRELUDE", &preamble)
            .env("FUNCTION_WITNESS", fixture.0.join("function-calls"))
            .env("EXECUTION_MARKER", fixture.0.join("executed"))
            .current_dir(&fixture.0)
            .stdin(Stdio::null());
        let output = FixtureChild::spawn(shell, &fixture.0).output();
        assert_eq!(output.status.code(), Some(status));
        assert_eq!(output.stdout, expected.as_bytes());
        assert!(output.stderr.is_empty());
        assert!(!fixture.0.join("executed").exists());
    }
    assert_eq!(
        fs::read_to_string(fixture.0.join("function-calls")).unwrap(),
        "cargo:build\nbun:run build\n"
    );
}

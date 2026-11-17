//! Claude fixture driver against the adapter registered by the dispatcher.
#[allow(dead_code)]
#[path = "../src/hooks.rs"]
mod hooks;
use hooks::fixture::claude;
use hooks::{HookContext, InstallAction, Scope, VerifiedSurface};
#[path = "fixtures/agent_hooks/claude/behavior.rs"]
mod behavior;
#[path = "fixtures/agent_hooks/claude/installer_cases.rs"]
mod installer_cases;
#[path = "fixtures/agent_hooks/claude/installer_concurrency.rs"]
mod installer_concurrency;
#[path = "fixtures/agent_hooks/claude/installer_recovery.rs"]
mod installer_recovery;
#[path = "fixtures/agent_hooks/claude/installer_regressions.rs"]
mod installer_regressions;
#[path = "fixtures/agent_hooks/claude/process.rs"]
mod process;

use serde_json::{json, Value};
use std::{
    env, fs,
    os::unix::{ffi::OsStrExt, fs::PermissionsExt},
    path::PathBuf,
    process::{Command, Stdio},
    time::Duration,
};

struct Fixture {
    root: PathBuf,
    bin: PathBuf,
    home: PathBuf,
    workspace: PathBuf,
    custom: PathBuf,
    executable: PathBuf,
    marker: PathBuf,
    cleanup: bool,
}

impl Fixture {
    fn new() -> Self {
        let root = PathBuf::from("/tmp").join(format!(
            "arc-claude-{}'",
            arc_core::process_identity::random_job_id().unwrap()
        ));
        let bin = root.join("bin");
        let home = root.join("home");
        let workspace = root.join("workspace");
        let custom = root.join("custom settings");
        for dir in [&bin, &home, &workspace, &custom] {
            fs::create_dir_all(dir).unwrap();
        }
        let marker = root.join("executed");
        let executable = bin.join("arcctl's executable");
        let marker_quoted = marker.to_str().unwrap().replace('\'', "'\\''");
        let body = format!("#!/bin/sh\nprintf ran > '{marker_quoted}'\n");
        for path in [&executable, &bin.join("cargo"), &bin.join("bun")] {
            fs::write(path, &body).unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        Self {
            root,
            bin,
            home,
            workspace,
            custom,
            executable,
            marker,
            cleanup: true,
        }
    }

    fn from_env() -> Self {
        let root = PathBuf::from(env::var_os("ARC_CLAUDE_TEST_ROOT").unwrap());
        Self {
            bin: root.join("bin"),
            home: root.join("home"),
            workspace: root.join("workspace"),
            custom: root.join("custom settings"),
            executable: root.join("bin/arcctl's executable"),
            marker: root.join("executed"),
            root,
            cleanup: false,
        }
    }

    fn context(&self) -> HookContext {
        HookContext {
            executable: Some(self.executable.clone()),
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // The parent owns the fixture; the child leaves it for assertions.
        if self.cleanup && !std::thread::panicking() {
            fs::remove_dir_all(&self.root).unwrap();
        }
    }
}

fn run_case(case: &str, extra: &[(&str, &str)]) {
    let fixture = Fixture::new();
    let mut child = Command::new(env::current_exe().unwrap());
    child
        .args(["--exact", "claude_fixture_child", "--nocapture"])
        .env_clear()
        .env("ARC_CLAUDE_TEST_CHILD", case)
        .env("ARC_CLAUDE_TEST_ROOT", &fixture.root)
        .env("HOME", &fixture.home)
        .env("PATH", &fixture.bin);
    // Preserve only the coverage output destination, not behavior inputs.
    forward_profile(&mut child);
    if matches!(case, "profile" | "execute") && env::var_os("LLVM_PROFILE_FILE").is_none() {
        child.env("LLVM_PROFILE_FILE", fixture.root.join("profile-%p.profraw"));
    }
    if case == "user-custom" {
        child.env("CLAUDE_CONFIG_DIR", &fixture.custom);
    }
    for (name, value) in extra {
        child.env(name, value);
    }
    let _handshake = hooks::fixture::worker_handshake(&mut child);
    let output = process::output(&mut child, &fixture.root, case, Duration::from_secs(180));
    assert!(
        output.status.success(),
        "case {case} failed; fixture retained at {}\nstdout: {}\nstderr: {}",
        fixture.root.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(&format!("ARC_CLAUDE_FIXTURE_COMPLETE:{case}\n")),
        "case {case} did not complete its worker body: {stdout}"
    );
    assert!(
        !fixture.marker.exists(),
        "the hook executed a fixture program"
    );
}

fn forward_profile(child: &mut Command) {
    if let Some(profile) = env::var_os("LLVM_PROFILE_FILE") {
        child.env("LLVM_PROFILE_FILE", profile);
    }
}

#[test]
fn verified_events_preserve_complete_input_and_bypass_unsupported_calls() {
    run_case("rewrite", &[]);
    run_case("bypass", &[]);
    run_case("disabled", &[("ARC_HOOKS_DISABLED", "1")]);
    run_case("disabled", &[("ARC_HOOKS_CLAUDE_DISABLED", "1")]);
    run_case("disabled", &[("CLAUDE_CODE_REMOTE", "true")]);
    run_case("unverified", &[("BASH_ENV", "/untrusted/startup")]);
    run_case("unverified", &[("BASH_FUNC_cargo%%", "() { false; }")]);
    run_case("unverified", &[("SSH_CONNECTION", "remote")]);
    run_case("unverified", &[("ARC_HOOKS_VERIFIED", "1")]);
    run_case("execute", &[]);
}

#[test]
fn cleared_child_environment_retains_the_coverage_destination() {
    run_case("profile", &[]);
}

#[test]
fn isolated_installers_preserve_settings_and_hook_order() {
    run_case("user", &[]);
    run_case("user-symlink", &[]);
    run_case("user-custom", &[]);
    run_case("workspace", &[]);
    run_case("user-mixed-install", &[]);
    run_case("user-mixed-uninstall", &[]);
    run_case("workspace-mixed-install", &[]);
    run_case("workspace-mixed-uninstall", &[]);
    run_case("user-authored-commands", &[]);
    run_case("workspace-authored-commands", &[]);
    run_case("user-marker-path", &[]);
    run_case("workspace-marker-path", &[]);
    run_case("user-lossless", &[]);
    run_case("workspace-lossless", &[]);
    run_case("user-acl", &[]);
    run_case("workspace-acl", &[]);
    run_case("user-recovery", &[]);
    run_case("workspace-recovery", &[]);
    run_case("user-moved-markers", &[]);
    run_case("workspace-moved-markers", &[]);
    run_case("user-extended-command", &[]);
    run_case("workspace-extended-command", &[]);
    run_case("user-ambiguous", &[]);
    run_case("workspace-ambiguous", &[]);
    run_case("user-unrelated-edits", &[]);
    run_case("workspace-unrelated-edits", &[]);
    run_case("user-competing", &[]);
    run_case("workspace-competing", &[]);
    run_case("user-invalid-identity", &[]);
    run_case("workspace-invalid-identity", &[]);
    run_case("bad-settings", &[]);
    run_case("missing-settings", &[]);
}

#[test]
fn production_unverified_hook_passes_the_documented_event_through() {
    let fixture = Fixture::new();
    let input = fs::File::open(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/agent_hooks/claude-pre-tool-use.json"
    ))
    .unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_arcctl"));
    child
        .args(["hooks", "run", "claude"])
        .env_clear()
        .env("HOME", &fixture.home)
        .env("PATH", &fixture.bin)
        .stdin(Stdio::from(input));
    forward_profile(&mut child);
    let output = process::output(
        &mut child,
        &fixture.root,
        "production hook",
        Duration::from_secs(90),
    );
    assert!(output.status.success());
    assert!(output.stdout.is_empty());
    assert!(!fixture.marker.exists());
}

// Only a driver with the private handshake runs the worker body, in a cleared
// environment; ordinary test discovery reports this entry as passed.
#[test]
fn claude_fixture_child() {
    if !hooks::fixture::worker_enabled() {
        return;
    }
    let fixture = Fixture::from_env();
    let case = env::var("ARC_CLAUDE_TEST_CHILD").unwrap();
    match case.as_str() {
        "rewrite" => rewrite_cases(&fixture),
        "bypass" => bypass_cases(&fixture),
        "disabled" => assert_no_update(&fixture, true),
        "unverified" => assert_no_update(&fixture, false),
        "execute" => behavior::execution(&fixture),
        "profile" => profile_destination(&fixture),
        "user" => installer_cases::installer_case(&fixture, Scope::User, false, false),
        "user-symlink" => installer_cases::installer_case(&fixture, Scope::User, false, true),
        "user-custom" => installer_cases::installer_case(&fixture, Scope::User, true, false),
        "workspace" => installer_cases::installer_case(&fixture, Scope::Workspace, false, false),
        "user-mixed-install" => installer_cases::mixed_case(&fixture, Scope::User, true),
        "user-mixed-uninstall" => installer_cases::mixed_case(&fixture, Scope::User, false),
        "workspace-mixed-install" => installer_cases::mixed_case(&fixture, Scope::Workspace, true),
        "workspace-mixed-uninstall" => {
            installer_cases::mixed_case(&fixture, Scope::Workspace, false)
        }
        "user-authored-commands" => installer_cases::authored_commands(&fixture, Scope::User),
        "workspace-authored-commands" => {
            installer_cases::authored_commands(&fixture, Scope::Workspace)
        }
        "user-marker-path" => installer_cases::marker_in_executable_path(&fixture, Scope::User),
        "workspace-marker-path" => {
            installer_cases::marker_in_executable_path(&fixture, Scope::Workspace)
        }
        "user-lossless" => installer_regressions::lossless(&fixture, Scope::User),
        "workspace-lossless" => installer_regressions::lossless(&fixture, Scope::Workspace),
        "user-acl" => installer_regressions::acl(&fixture, Scope::User),
        "workspace-acl" => installer_regressions::acl(&fixture, Scope::Workspace),
        "user-recovery" => installer_recovery::recovery(&fixture, Scope::User),
        "workspace-recovery" => installer_recovery::recovery(&fixture, Scope::Workspace),
        "user-moved-markers" => installer_recovery::moved_markers(&fixture, Scope::User),
        "workspace-moved-markers" => installer_recovery::moved_markers(&fixture, Scope::Workspace),
        "user-extended-command" => installer_recovery::extended_command(&fixture, Scope::User),
        "workspace-extended-command" => {
            installer_recovery::extended_command(&fixture, Scope::Workspace)
        }
        "user-ambiguous" => installer_recovery::ambiguous_lookalike(&fixture, Scope::User),
        "workspace-ambiguous" => {
            installer_recovery::ambiguous_lookalike(&fixture, Scope::Workspace)
        }
        "user-unrelated-edits" => installer_concurrency::unrelated_edits(&fixture, Scope::User),
        "workspace-unrelated-edits" => {
            installer_concurrency::unrelated_edits(&fixture, Scope::Workspace)
        }
        "user-competing" => installer_concurrency::competing_updates(&fixture, Scope::User),
        "workspace-competing" => {
            installer_concurrency::competing_updates(&fixture, Scope::Workspace)
        }
        "user-invalid-identity" => installer_regressions::invalid_identity(&fixture, Scope::User),
        "workspace-invalid-identity" => {
            installer_regressions::invalid_identity(&fixture, Scope::Workspace)
        }
        "bad-settings" => installer_cases::bad_settings(&fixture),
        "missing-settings" => installer_cases::missing_settings(&fixture),
        other => panic!("unknown fixture case: {other}"),
    }
    assert!(!fixture.marker.exists());
    println!("ARC_CLAUDE_FIXTURE_COMPLETE:{case}");
}

fn profile_destination(fixture: &Fixture) {
    let expected = env::var_os("LLVM_PROFILE_FILE").unwrap();
    let mut child = Command::new("/bin/sh");
    child
        .args(["-c", "printf %s \"$LLVM_PROFILE_FILE\""])
        .env_clear();
    forward_profile(&mut child);
    let output = process::output(
        &mut child,
        &fixture.root,
        "profile shell",
        Duration::from_secs(90),
    );
    assert!(output.status.success());
    assert_eq!(output.stdout, expected.as_os_str().as_bytes());
}

fn event() -> Value {
    serde_json::from_str(include_str!(
        "fixtures/agent_hooks/claude-pre-tool-use.json"
    ))
    .unwrap()
}

fn local_event(fixture: &Fixture) -> Value {
    let mut event = event();
    event["cwd"] = json!(fixture.workspace);
    event
}

fn rewrite_cases(fixture: &Fixture) {
    let context = fixture.context();
    let surface = VerifiedSurface {
        command_path: fixture.bin.as_os_str(),
    };
    for literal in ["cargo build", "bun run build"] {
        let mut event = local_event(fixture);
        event["tool_input"]["command"] = json!(literal);
        event["tool_input"]["future_exact_integer"] = json!(u64::MAX);
        let original = event.clone();
        let response = claude::handle_verified(&event, &context, Some(&surface)).unwrap();
        let expected = format!(
            "'{}' hook-exec -- {literal}",
            fixture.executable.to_str().unwrap().replace('\'', "'\\''")
        );
        let mut expected_input = event["tool_input"].clone();
        expected_input["command"] = json!(expected);
        assert_eq!(
            response,
            json!({"hookSpecificOutput": {
                "hookEventName": "PreToolUse", "updatedInput": expected_input
            }})
        );
        assert_eq!(event, original);
        assert!(response["hookSpecificOutput"]
            .get("permissionDecision")
            .is_none());
        event["tool_input"] = response["hookSpecificOutput"]["updatedInput"].clone();
        assert!(claude::handle_verified(&event, &context, Some(&surface)).is_none());
        assert!(claude::handle(&original, &context).is_none());
    }
}

fn bypass_cases(fixture: &Fixture) {
    let context = fixture.context();
    let surface = VerifiedSurface {
        command_path: fixture.bin.as_os_str(),
    };
    for command in [
        "cargo  build",
        " cargo build",
        "cargo build ",
        "cargo build --release",
        "cargo build; true",
        "'cargo' build",
        "cargo build\n",
        "FOO=x cargo build",
        "cargo build > out",
        "bun run build | cat",
        "bun run $(echo build)",
        "/usr/bin/cargo build",
        "arcctl hook-exec -- cargo build",
        "bun build",
    ] {
        let mut event = local_event(fixture);
        event["tool_input"]["command"] = json!(command);
        assert!(
            claude::handle_verified(&event, &context, Some(&surface)).is_none(),
            "{command:?}"
        );
    }
    for malformed in [
        json!(null),
        json!({}),
        json!({"hook_event_name": "PreToolUse", "tool_name": "Bash", "tool_input": {"command": "cargo build"}}),
        json!({"hook_event_name": "PreToolUse", "tool_name": "Bash", "cwd": "relative", "tool_input": {"command": "cargo build"}}),
        json!({"hook_event_name": "PostToolUse", "tool_name": "Bash", "cwd": "/fixture", "tool_input": {"command": "cargo build"}}),
        json!({"hook_event_name": "PreToolUse", "tool_name": "Shell", "cwd": "/fixture", "tool_input": {"command": "cargo build"}}),
        json!({"hook_event_name": "PreToolUse", "tool_name": "Bash", "cwd": "/fixture", "tool_input": "cargo build"}),
        json!({"hook_event_name": "PreToolUse", "tool_name": "Bash", "cwd": "/fixture", "tool_input": {"command": 123}}),
    ] {
        assert!(claude::handle_verified(&malformed, &context, Some(&surface)).is_none());
    }
    // The frozen shared dispatcher may already have rounded these values.
    // No response keeps the original raw input at the vendor intact.
    for raw in [
        "18446744073709551617",
        "-9223372036854775809",
        "0.12345678901234567890123456789",
        r#"{"nested":[1,0.5]}"#,
    ] {
        let mut event = local_event(fixture);
        event["tool_input"]["future_field"] = serde_json::from_str(raw).unwrap();
        assert!(
            claude::handle_verified(&event, &context, Some(&surface)).is_none(),
            "{raw}"
        );
    }
    assert!(claude::handle_verified(&local_event(fixture), &context, None).is_none());
    let relative = VerifiedSurface {
        command_path: std::ffi::OsStr::new("bin"),
    };
    assert!(claude::handle_verified(&local_event(fixture), &context, Some(&relative)).is_none());
    assert!(claude::handle_verified(
        &local_event(fixture),
        &HookContext { executable: None },
        Some(&surface)
    )
    .is_none());
    let missing = HookContext {
        executable: Some(fixture.bin.join("missing")),
    };
    assert!(claude::handle_verified(&local_event(fixture), &missing, Some(&surface)).is_none());
    fs::remove_file(fixture.bin.join("cargo")).unwrap();
    assert!(claude::handle_verified(&local_event(fixture), &context, Some(&surface)).is_none());
    fs::set_permissions(fixture.bin.join("bun"), fs::Permissions::from_mode(0o644)).unwrap();
    let mut bun_event = local_event(fixture);
    bun_event["tool_input"]["command"] = json!("bun run build");
    assert!(claude::handle_verified(&bun_event, &context, Some(&surface)).is_none());
}

fn assert_no_update(fixture: &Fixture, verified: bool) {
    let context = fixture.context();
    let surface = VerifiedSurface {
        command_path: fixture.bin.as_os_str(),
    };
    if verified {
        assert!(claude::handle_verified(&local_event(fixture), &context, Some(&surface)).is_none());
    } else {
        assert!(claude::handle(&local_event(fixture), &context).is_none());
    }
}

use super::*;
use std::{fs, os::unix::fs::PermissionsExt};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = PathBuf::from("/tmp").join(format!(
            "arc-hook-contract-{}'",
            arc_core::process_identity::random_job_id().unwrap()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn executable(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);
        // Running this file would create the marker; rewriting must not run it.
        let marker = self
            .0
            .join("executed")
            .to_str()
            .unwrap()
            .replace('\'', "'\\''");
        fs::write(&path, format!("#!/bin/sh\nprintf x > '{marker}'\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        path
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
fn fixtures_dispatch_without_unverified_approval_or_execution() {
    let context = HookContext { executable: None };
    for (vendor, source, name) in [
        (
            Vendor::Claude,
            include_str!("../../tests/fixtures/agent_hooks/claude-pre-tool-use.json"),
            "Bash",
        ),
        (
            Vendor::Cursor,
            include_str!("../../tests/fixtures/agent_hooks/cursor-pre-tool-use.json"),
            "Shell",
        ),
    ] {
        let event: Value = serde_json::from_str(source).unwrap();
        assert_eq!(event["tool_name"], name);
        assert!(dispatch(vendor, &event, &context).is_none());
        let surface = VerifiedSurface;
        assert!(dispatch_verified(vendor, &event, &context, &surface).is_none());
    }
}

#[test]
fn dispatcher_preserves_exact_inputs_or_declines_ambiguous_json() {
    let thread = std::thread::current();
    let name = thread.name().unwrap();
    let worker = name
        .strip_suffix("dispatcher_preserves_exact_inputs_or_declines_ambiguous_json")
        .map(|prefix| format!("{prefix}exact_event_worker"))
        .unwrap();
    let receipt = Fixture::new();
    let output_path = receipt.0.join("worker-output");
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", &worker, "--nocapture"])
        .env_clear()
        .stdout(std::process::Stdio::from(
            fs::File::create(&output_path).unwrap(),
        ));
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile);
    }
    let _handshake = super::fixture::worker_handshake(&mut command);
    let mut child = super::fixture::OwnedChild::spawn(&mut command, "exact-event fixture");
    let status = child.wait();
    assert!(status.success(), "exact-event fixture failed: {status}");
    let output = fs::read_to_string(output_path).unwrap();
    assert!(
        output.contains(&format!("test {worker} ..."))
            && output.contains("test result: ok. 1 passed;")
            && output.contains("ARC_EXACT_EVENT_WORKER_COMPLETE"),
        "fixture worker did not execute: {output}"
    );
}

#[test]
fn ambient_worker_token_cannot_activate_exact_event_worker() {
    let thread = std::thread::current();
    let name = thread.name().unwrap();
    let worker = name
        .strip_suffix("ambient_worker_token_cannot_activate_exact_event_worker")
        .map(|prefix| format!("{prefix}exact_event_worker"))
        .unwrap();
    let receipt = Fixture::new();
    let output_path = receipt.0.join("ambient-worker-output");
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", &worker, "--nocapture"])
        .env_clear()
        .env(
            "ARC_HOOK_TEST_WORKER_TOKEN",
            "00000000000000000000000000000000",
        )
        .stdout(std::process::Stdio::from(
            fs::File::create(&output_path).unwrap(),
        ));
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile);
    }
    let mut child = super::fixture::OwnedChild::spawn(&mut command, "ambient exact-event worker");
    assert!(child.wait().success());
    let output = fs::read_to_string(output_path).unwrap();
    assert!(output.contains("test result: ok. 1 passed;"), "{output}");
    assert!(
        !output.contains("ARC_EXACT_EVENT_WORKER_COMPLETE"),
        "{output}"
    );
}

#[test]
fn exact_event_worker() {
    if !super::fixture::worker_enabled() {
        return;
    }
    let fixture = Fixture::new();
    let context = HookContext {
        executable: Some(fixture.executable("arcctl")),
    };
    fixture.executable("cargo");
    fixture.executable("bun");
    let surface = VerifiedSurface;
    for (vendor, source, field, literal) in [
        (
            Vendor::Claude,
            include_str!("../../tests/fixtures/agent_hooks/claude-pre-tool-use.json"),
            "updatedInput",
            "cargo build",
        ),
        (
            Vendor::Cursor,
            include_str!("../../tests/fixtures/agent_hooks/cursor-pre-tool-use.json"),
            "updated_input",
            "bun run build",
        ),
        (
            Vendor::Codex,
            include_str!("../../tests/fixtures/agent_hooks/codex-pre-tool-use.json"),
            "updatedInput",
            "bun run build",
        ),
    ] {
        let mut event: Value = serde_json::from_str(source).unwrap();
        event["tool_input"]["max_integer"] = serde_json::json!(u64::MAX);
        event["tool_input"]["nested"] = serde_json::json!([{"key": "value"}, true, null]);
        let input = serde_json::to_vec(&event).unwrap();
        assert!(dispatch(vendor, &event, &context).is_some());
        let response = dispatch_verified_event(vendor, &input, &context, &surface).unwrap();
        let updated = if field == "updatedInput" {
            if matches!(vendor, Vendor::Codex) {
                assert_eq!(
                    response["hookSpecificOutput"]["permissionDecision"],
                    "allow"
                );
            } else {
                assert!(response["hookSpecificOutput"]
                    .get("permissionDecision")
                    .is_none());
            }
            &response["hookSpecificOutput"][field]
        } else {
            assert!(response.get("permission").is_none());
            &response[field]
        };
        let mut original = updated.clone();
        // Only an event with both ids names itself in the command.
        let adapter = match vendor {
            Vendor::Claude => event_key::CLAUDE,
            Vendor::Cursor => event_key::CURSOR,
            Vendor::Codex => event_key::CODEX,
        };
        assert!(original["command"]
            .as_str()
            .unwrap()
            .ends_with(&format!("{}{literal}", event_key::route(adapter, &event))));
        original["command"] = serde_json::json!(literal);
        assert_eq!(original, event["tool_input"]);
        event["tool_input"] = updated.clone();
        assert!(dispatch_verified_event(
            vendor,
            &serde_json::to_vec(&event).unwrap(),
            &context,
            &surface
        )
        .is_none());
        for extra in [
            r#""duplicate":1,"duplicate":2"#,
            r#""duplicate":1,"\u0064uplicate":2"#,
            r#""nested":{"x":1,"x":2}"#,
        ] {
            let raw = source.replacen(
                "\"tool_input\": {",
                &format!("\"tool_input\": {{{extra},"),
                1,
            );
            assert!(
                dispatch_verified_event(vendor, raw.as_bytes(), &context, &surface).is_none(),
                "{raw}"
            );
        }
        for extra in [
            r#""precise":0.12345678901234567890123456789"#,
            r#""decimal":1.0"#,
            r#""exponent":1e0"#,
            r#""negative_zero":-0"#,
            r#""large":18446744073709551616"#,
        ] {
            let raw = source.replacen(
                "\"tool_input\": {",
                &format!("\"tool_input\": {{{extra},"),
                1,
            );
            let expected: Value = serde_json::from_str(&raw).unwrap();
            let response =
                dispatch_verified_event(vendor, raw.as_bytes(), &context, &surface).unwrap();
            let wire = event::wire_response(vendor, raw.as_bytes(), &response).unwrap();
            assert!(
                String::from_utf8(wire).unwrap().contains(extra),
                "raw number changed: {extra}"
            );
            let mut actual = if field == "updatedInput" {
                response["hookSpecificOutput"][field].clone()
            } else {
                response[field].clone()
            };
            actual["command"] = serde_json::json!(literal);
            assert_eq!(actual, expected["tool_input"], "{raw}");
        }
        assert!(!fixture.0.join("executed").exists());
    }
    println!("ARC_EXACT_EVENT_WORKER_COMPLETE");
}

#[test]
fn run_and_installer_dispatch_select_the_requested_vendor() {
    let fixture = Fixture::new();
    let context = HookContext { executable: None };
    let event = serde_json::json!({"command":"cargo build", "unrelated": 17});
    for (vendor, expected) in [(Vendor::Claude, "claude"), (Vendor::Cursor, "cursor")] {
        let response = dispatch_with(
            vendor,
            &event,
            &context,
            |input, _| Some(serde_json::json!({"route":"claude", "input":input})),
            |input, _| Some(serde_json::json!({"route":"cursor", "input":input})),
        )
        .unwrap();
        assert_eq!(response["route"], expected);
        assert_eq!(response["input"], event);
        for (action, scope, workspace) in [
            (InstallAction::Install, Scope::User, None),
            (
                InstallAction::Uninstall,
                Scope::Workspace,
                Some(fixture.0.clone()),
            ),
        ] {
            let called = std::cell::Cell::new("");
            let check =
                |route, received_action, received_scope, received_workspace: Option<&Path>| {
                    assert_eq!(
                        matches!(received_action, InstallAction::Install),
                        matches!(action, InstallAction::Install)
                    );
                    assert_eq!(
                        matches!(received_scope, Scope::User),
                        matches!(scope, Scope::User)
                    );
                    assert_eq!(received_workspace, workspace.as_deref());
                    called.set(route);
                    Ok(())
                };
            assert_eq!(
                configure_with(
                    vendor,
                    action,
                    scope,
                    workspace.clone(),
                    &context,
                    |a, s, w, _| check("claude", a, s, w),
                    |a, s, w, _| check("cursor", a, s, w),
                ),
                ExitCode::SUCCESS
            );
            assert_eq!(called.get(), expected);
        }
    }
    assert!(!fixture.0.join("executed").exists());
}

#[test]
fn literal_rewrite_requires_surface_and_available_arcctl_but_not_hook_path() {
    let fixture = Fixture::new();
    let arcctl = fixture.executable("arcctl");
    fixture.executable("cargo");
    fixture.executable("bun");
    let context = HookContext {
        executable: Some(arcctl),
    };
    let surface = VerifiedSurface;
    assert!(rewrite_literal("cargo build", &context, None).is_none());
    for command in ["cargo build", "bun run build", "gh workflow run build.yml"] {
        let rewritten = rewrite_literal(command, &context, Some(&surface)).unwrap();
        assert!(
            rewritten.contains("'\\''"),
            "quote in executable path: {rewritten}"
        );
        if command == "gh workflow run build.yml" {
            assert!(rewritten.starts_with("case "));
            assert!(rewritten.contains("--arc-check-gh-path"));
            assert_eq!(
                rewritten
                    .matches(" hook-exec -- gh workflow run build.yml")
                    .count(),
                4
            );
        } else {
            assert!(rewritten.ends_with(&format!(" hook-exec -- {command}")));
        }
        assert!(rewrite_literal(&rewritten, &context, Some(&surface)).is_none());
    }
    for unsupported in [
        "cargo  build",
        " cargo build",
        "cargo build ",
        "cargo build --release",
        "cargo build; true",
        "'cargo' build",
        "bun run build\n",
        "bun run build | cat",
        "gh workflow run build.yml --ref main",
        "gh workflow run build.yml; true",
        "gh workflow run 'build.yml'",
        "arcctl exec -- cargo build",
    ] {
        assert!(
            rewrite_literal(unsupported, &context, Some(&surface)).is_none(),
            "{unsupported:?}"
        );
    }
    assert!(!fixture.0.join("executed").exists());
    fs::remove_file(fixture.0.join("cargo")).unwrap();
    assert!(rewrite_literal("cargo build", &context, Some(&surface)).is_some());
    let relative = HookContext {
        executable: Some(PathBuf::from("relative")),
    };
    assert!(rewrite_literal("bun run build", &relative, Some(&surface)).is_none());
    fs::remove_file(context.executable.as_ref().unwrap()).unwrap();
    assert!(rewrite_literal("bun run build", &context, Some(&surface)).is_none());
}

#[test]
fn gh_rewrite_executes_through_an_apostrophe_in_the_arc_path() {
    let fixture = Fixture::new();
    let arcctl = fixture.executable("arcctl");
    fs::write(
        &arcctl,
        "#!/bin/sh\nif [ \"$3\" = --arc-check-gh-path ]; then exit 0; fi\nshift 2\nexec \"$@\"\n",
    )
    .unwrap();
    fixture.executable("gh");
    let context = HookContext {
        executable: Some(arcctl),
    };
    let rewritten = rewrite_literal(
        "gh workflow run build.yml",
        &context,
        Some(&VerifiedSurface),
    )
    .unwrap();
    let mut command = std::process::Command::new("/bin/sh");
    command
        .args(["-c", &rewritten])
        .env_clear()
        .env("HOME", &fixture.0)
        .env("PATH", &fixture.0)
        .current_dir(&fixture.0);
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile);
    }
    let mut child = super::fixture::OwnedChild::spawn(&mut command, "quoted gh rewrite");
    assert!(child.wait().success());
    assert_eq!(fs::read(fixture.0.join("executed")).unwrap(), b"x");
}

#[test]
fn controlled_shell_executes_rewritten_literal_once_after_hook_returns() {
    let fixture = Fixture::new();
    let arcctl = fixture.executable("arcctl");
    fs::write(&arcctl, "#!/bin/sh\nshift 2\nexec \"$@\"\n").unwrap();
    fixture.executable("cargo");
    let context = HookContext {
        executable: Some(arcctl),
    };
    // The driver, rather than a vendor event or environment marker, controls
    // this shell: a fresh non-login shell with no startup variables/functions.
    let surface = VerifiedSurface;
    let rewritten = rewrite_literal("cargo build", &context, Some(&surface)).unwrap();
    let marker = fixture.0.join("executed");
    assert!(!marker.exists());
    let mut command = std::process::Command::new("/bin/sh");
    command
        .args(["-c", &rewritten])
        .env_clear()
        .env("PATH", &fixture.0)
        .current_dir(&fixture.0);
    let mut child = super::fixture::OwnedChild::spawn(&mut command, "controlled hook shell");
    let status = child.wait();
    assert!(status.success());
    assert_eq!(fs::read(marker).unwrap(), b"x");
}

#[path = "surface_tests.rs"]
mod surface_tests;

#[test]
fn hook_exec_grammar_is_exact() {
    let argv = |words: &[&str]| words.iter().map(OsString::from).collect::<Vec<_>>();
    assert!(literal_argv(&argv(&["cargo", "build"])));
    assert!(literal_argv(&argv(&["bun", "run", "build"])));
    assert!(literal_argv(&argv(&["gh", "workflow", "run", "build.yml"])));
    for words in [
        &["cargo", "build", "--release"][..],
        &["/bin/cargo", "build"],
        &["bun", "build"],
        &["sh", "-c", "cargo build"],
        &["gh", "workflow", "run", "build.yml", "--ref", "main"],
    ] {
        assert!(!literal_argv(&argv(words)));
    }
}

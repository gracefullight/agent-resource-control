use std::{env, path::Path, process::Command};

const OUTAGE_DIAGNOSTIC: &[u8] = b"TRACKING_UNAVAILABLE\nCommand started. ARC tracking is unavailable. A lifecycle record may be absent.\nDo not launch another copy to recover tracking.\n";

// The child and the asynchronous ARC diagnostic share stderr. Remove exactly
// one known child message, then verify every remaining byte as an outage notice.
fn fixture_stderr_matches(direct: &[u8], wrapped: &[u8]) -> bool {
    if direct != b"child-stderr" {
        return false;
    }
    let mut matches = wrapped
        .windows(direct.len())
        .enumerate()
        .filter_map(|(index, bytes)| (bytes == direct).then_some(index));
    let Some(start) = matches.next() else {
        return false;
    };
    if matches.next().is_some() {
        return false;
    }
    let mut diagnostic = Vec::with_capacity(wrapped.len() - direct.len());
    diagnostic.extend_from_slice(&wrapped[..start]);
    diagnostic.extend_from_slice(&wrapped[start + direct.len()..]);
    diagnostic.is_empty() || diagnostic == OUTAGE_DIAGNOSTIC
}

#[test]
fn stderr_comparison_rejects_unrecognized_or_duplicated_output() {
    let child = b"child-stderr";
    assert!(fixture_stderr_matches(child, child));
    assert!(fixture_stderr_matches(
        child,
        &[OUTAGE_DIAGNOSTIC, child].concat()
    ));
    assert!(fixture_stderr_matches(
        child,
        &[child, OUTAGE_DIAGNOSTIC].concat()
    ));
    assert!(!fixture_stderr_matches(child, b"extra child-stderr"));
    assert!(!fixture_stderr_matches(child, b"child-stderr extra"));
    assert!(!fixture_stderr_matches(child, b"child-stderrchild-stderr"));
    assert!(!fixture_stderr_matches(
        child,
        &[OUTAGE_DIAGNOSTIC, child, OUTAGE_DIAGNOSTIC].concat()
    ));
    assert!(!fixture_stderr_matches(b"", b""));
}

// Integration fixtures can compile the same source without a production
// command-line switch or environment variable that manufactures surface proof.
#[path = "binary/claude_hook_lane.rs"]
mod claude_hook_lane;
#[path = "binary/cursor_hook_lane.rs"]
mod cursor_hook_lane;
#[path = "../src/event_digest.rs"]
mod event_digest;
#[path = "fixtures/agent_hooks/event_key.rs"]
mod event_key;
#[path = "binary/hook_resolution.rs"]
mod hook_resolution;
#[path = "binary/hook_scheduled.rs"]
mod hook_scheduled;
#[allow(dead_code)]
#[path = "../src/hooks.rs"]
mod hooks;
#[path = "binary/hooks_install.rs"]
mod hooks_install;
#[path = "binary/installer_hardening.rs"]
mod installer_hardening;

#[test]
fn fixture_driver_can_compile_the_frozen_dispatch_interface() {
    let context = hooks::HookContext { executable: None };
    let surface = hooks::VerifiedSurface;
    let event: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/agent_hooks/claude-pre-tool-use.json"
    ))
    .unwrap();
    assert!(hooks::dispatch_verified(hooks::Vendor::Claude, &event, &context, &surface).is_none());
}

#[test]
fn arcctl_binary_is_built_next_to_the_test_harness() {
    let path = env!("CARGO_BIN_EXE_arcctl");
    assert!(Path::new(path).is_file(), "missing arcctl binary at {path}");
}

#[test]
fn no_arguments_names_both_commands_and_exits_nonzero() {
    let output = Command::new(env!("CARGO_BIN_EXE_arcctl")).output().unwrap();
    assert!(!output.status.success(), "{output:?}");
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    for expected in ["status", "doctor"] {
        assert!(
            combined.contains(expected),
            "missing {expected:?} in {combined}"
        );
    }
    assert!(!combined.contains("\u{1b}["), "colored output: {combined}");
}

#[test]
fn legacy_usage_errors_and_version_keep_their_exit_contract() {
    for args in [&["unknown-operation"][..], &["status", "--unknown"]] {
        let output = Command::new(env!("CARGO_BIN_EXE_arcctl"))
            .args(args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).contains("Usage:"));
        assert!(!output.stderr.contains(&27));
    }
    let output = Command::new(env!("CARGO_BIN_EXE_arcctl"))
        .arg("--version")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    assert_eq!(output.stdout, b"arcctl 0.1.0\n");
}

#[test]
fn exec_short_and_long_help_describe_fallback_and_non_launching_outcomes() {
    for flag in ["-h", "--help"] {
        let output = Command::new(env!("CARGO_BIN_EXE_arcctl"))
            .args(["exec", flag])
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        assert!(output.stderr.is_empty());
        let help = String::from_utf8(output.stdout).unwrap();
        assert!(help.contains("If the daemon is unavailable or tracking is unconfirmed due to an outage or exhausted DB_BUSY retries, ARC still attempts the command once."), "{help}");
        assert!(help.contains("Cancellation, invalid arguments, invalid protocol replies, local preparation failures and explicit authenticated rejection do not start a child."), "{help}");
        assert!(
            !help.contains("If the daemon is unavailable or tracking cannot be confirmed"),
            "{help}"
        );
    }
}

#[test]
fn shared_hook_dispatch_and_exec_preserve_literal_child_behavior() {
    use std::process::Stdio;
    let output_path = std::path::PathBuf::from("/tmp").join(format!(
        "arc-hook-semantics-worker-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    let temporary_home = output_path.with_extension("home");
    std::fs::create_dir(&temporary_home).unwrap();
    let mut command = Command::new(env::current_exe().unwrap());
    command
        .args(["--exact", "vendor_event_semantics_worker", "--nocapture"])
        .env_clear()
        .env("HOME", &temporary_home)
        .stdout(Stdio::from(std::fs::File::create(&output_path).unwrap()));
    if let Some(profile) = env::var_os("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile);
    }
    let _handshake = hooks::fixture::worker_handshake(&mut command);
    let mut child =
        hooks::fixture::OwnedChild::spawn(&mut command, "vendor-event semantics worker");
    let status = child.wait();
    assert!(
        status.success(),
        "vendor-event semantics worker failed: {status}"
    );
    let output = std::fs::read_to_string(&output_path).unwrap();
    assert!(
        output.contains("test vendor_event_semantics_worker ...")
            && output.contains("test result: ok. 1 passed;")
            && output.contains("ARC_VENDOR_SEMANTICS_WORKER_COMPLETE"),
        "fixture worker did not execute: {output}"
    );
    std::fs::remove_file(output_path).unwrap();
    std::fs::remove_dir_all(temporary_home).unwrap();
}

#[test]
fn hook_exec_propagates_child_signal_once_for_both_literals() {
    use std::process::Stdio;

    let output_path = std::path::PathBuf::from("/tmp").join(format!(
        "arc-hook-signal-worker-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    let mut command = Command::new(env::current_exe().unwrap());
    command
        .args(["--exact", "hook_exec_signal_worker", "--nocapture"])
        .env_clear()
        .stdout(Stdio::from(std::fs::File::create(&output_path).unwrap()));
    if let Some(profile) = env::var_os("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile);
    }
    let _handshake = hooks::fixture::worker_handshake(&mut command);
    // The marker and owned group survive the nested hook-exec process group.
    // If the worker or nested wait expires, this supervisor owns cleanup.
    let mut child = hooks::fixture::OwnedChild::spawn(&mut command, "hook signal worker");
    let status = child.wait();
    let output = std::fs::read_to_string(&output_path).unwrap();
    assert!(
        status.success(),
        "hook signal worker failed: {status}: {output}"
    );
    assert!(
        output.contains("test hook_exec_signal_worker ...")
            && output.contains("test result: ok. 1 passed;")
            && output.contains("ARC_HOOK_SIGNAL_WORKER_COMPLETE"),
        "fixture worker did not execute: {output}"
    );
    std::fs::remove_file(output_path).unwrap();
}

#[test]
fn hook_exec_signal_worker() {
    if !hooks::fixture::worker_enabled() {
        return;
    }
    use std::{
        fs,
        os::unix::{fs::PermissionsExt, process::ExitStatusExt},
        process::Stdio,
    };

    struct Fixture(std::path::PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            if !std::thread::panicking() {
                fs::remove_dir_all(&self.0).unwrap();
            }
        }
    }
    let fixture = Fixture(std::path::PathBuf::from("/tmp").join(format!(
        "arc-hook-signal-{}'",
        arc_core::process_identity::random_job_id().unwrap()
    )));
    fs::create_dir(&fixture.0).unwrap();
    for name in ["cargo", "bun"] {
        let path = fixture.0.join(name);
        fs::write(
            &path,
            b"#!/bin/sh\nprintf 'start\\n' >> \"$WITNESS\"\nprintf child-out\nprintf child-stderr >&2\nkill -TERM $$\n",
        )
        .unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let marker = fixture.0.join("starts");
    let ownership = env::var_os("ARC_TEST_OWNERSHIP").expect("owned signal worker marker");
    let arcctl = env!("CARGO_BIN_EXE_arcctl").replace('\'', "'\\''");
    let execute = |command: &str| {
        let mut process = Command::new("/bin/sh");
        process
            .args(["-c", command])
            .env_clear()
            .env("HOME", &fixture.0)
            .env("PATH", &fixture.0)
            .env("ARC_SOCKET_PATH", fixture.0.join("missing.sock"))
            .env("ARC_TEST_OWNERSHIP", &ownership)
            .env("WITNESS", &marker)
            .current_dir(&fixture.0)
            .stdin(Stdio::null());
        if let Some(profile) = env::var_os("LLVM_PROFILE_FILE") {
            process.env("LLVM_PROFILE_FILE", profile);
        }
        hook_scheduled::FixtureChild::spawn_nested(process, &fixture.0).output()
    };
    for literal in ["cargo build", "bun run build"] {
        let direct = execute(literal);
        let wrapped = execute(&format!("'{arcctl}' hook-exec -- {literal}"));
        assert_eq!(direct.status.signal(), Some(libc::SIGTERM));
        assert_eq!(wrapped.status.signal(), direct.status.signal());
        assert_eq!(wrapped.stdout, direct.stdout);
        assert!(fixture_stderr_matches(&direct.stderr, &wrapped.stderr));
    }
    assert_eq!(fs::read_to_string(marker).unwrap().lines().count(), 4);
    println!("ARC_HOOK_SIGNAL_WORKER_COMPLETE");
}

#[test]
fn vendor_event_semantics_worker() {
    if !hooks::fixture::worker_enabled() {
        return;
    }
    use std::{
        ffi::{OsStr, OsString},
        fs,
        os::unix::{
            ffi::{OsStrExt, OsStringExt},
            fs::PermissionsExt,
        },
        process::{Output, Stdio},
    };

    struct Fixture(std::path::PathBuf);
    impl Drop for Fixture {
        fn drop(&mut self) {
            if !std::thread::panicking() {
                fs::remove_dir_all(&self.0).unwrap();
            }
        }
    }
    let fixture = Fixture(std::path::PathBuf::from("/tmp").join(format!(
        "arc-hook-cli-{}",
        arc_core::process_identity::random_job_id().unwrap()
    )));
    fs::create_dir(&fixture.0).unwrap();
    let cargo = fixture.0.join("cargo");
    fs::write(&cargo, b"#!/bin/sh\nprintf 'underscore=%s\\n' \"$_\"\nprintf 'started\\n' >> \"$ARC_HOOK_TEST_MARKER\"\nprintf '%s\\n' \"${ARC_TRAFFIC_LINEAGE+x}\" \"${ARC_TRAFFIC_LINEAGE-absent}\" \"$PRIVATE_ENV\" \"$PWD\" \"$1\" \"${2-}\"\n/bin/cat\nprintf child-stderr >&2\nexit 17\n").unwrap();
    fs::set_permissions(&cargo, fs::Permissions::from_mode(0o755)).unwrap();
    fs::copy(&cargo, fixture.0.join("bun")).unwrap();
    let marker = fixture.0.join("starts");
    let missing_socket = fixture.0.join("missing.sock");

    let execute =
        |program: &OsStr, args: &[&str], lineage: Option<&OsStr>, input: &[u8]| -> Output {
            let mut cmd = Command::new(program);
            cmd.env_clear()
                .env("HOME", &fixture.0)
                .env("XDG_CONFIG_HOME", fixture.0.join("xdg"))
                .env("GIT_CONFIG_GLOBAL", fixture.0.join("gitconfig"))
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .args(args)
                .current_dir(&fixture.0)
                .env("PATH", &fixture.0)
                .env("ARC_SOCKET_PATH", &missing_socket)
                .env("ARC_HOOK_TEST_MARKER", &marker)
                .env("PRIVATE_ENV", OsString::from_vec(vec![b'p', 0xff, b'q']))
                .env_remove("ARC_TRAFFIC_LINEAGE")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            if let Some(value) = lineage {
                cmd.env("ARC_TRAFFIC_LINEAGE", value);
            }
            // Keep the coverage runner's output destination while controlling
            // every variable that can affect the fixture's behavior.
            if let Some(profile) = env::var_os("LLVM_PROFILE_FILE") {
                cmd.env("LLVM_PROFILE_FILE", profile);
            }
            let mut child = hook_scheduled::FixtureChild::spawn_nested(cmd, &fixture.0);
            child.write_input(input);
            child.output()
        };

    let cli = OsStr::new(env!("CARGO_BIN_EXE_arcctl"));
    for (vendor, event) in [
        (
            "claude",
            include_bytes!("fixtures/agent_hooks/claude-pre-tool-use.json").as_slice(),
        ),
        (
            "cursor",
            include_bytes!("fixtures/agent_hooks/cursor-pre-tool-use.json").as_slice(),
        ),
        (
            "codex",
            include_bytes!("fixtures/agent_hooks/codex-pre-tool-use.json").as_slice(),
        ),
    ] {
        let output = execute(cli, &["hooks", "run", vendor], None, event);
        assert_eq!(output.status.code(), Some(0));
        let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert!(response.is_object());
        assert!(!marker.exists(), "a hook executed the requested program");
    }
    let invalid = execute(
        cli,
        &["hook-exec", "--", "cargo", "build", "--release"],
        None,
        b"",
    );
    assert!(!invalid.status.success());
    assert!(!marker.exists());

    let shells: Vec<&str> = ["/bin/sh", "/bin/bash", "/bin/zsh"]
        .into_iter()
        .filter(|shell| Path::new(shell).is_file())
        .collect();
    assert!(shells.contains(&"/bin/sh"));
    for (vendor, fixture_event) in [
        (
            hooks::Vendor::Claude,
            include_str!("fixtures/agent_hooks/claude-pre-tool-use.json"),
        ),
        (
            hooks::Vendor::Cursor,
            include_str!("fixtures/agent_hooks/cursor-pre-tool-use.json"),
        ),
        (
            hooks::Vendor::Codex,
            include_str!("fixtures/agent_hooks/codex-pre-tool-use.json"),
        ),
    ] {
        for literal in ["cargo build", "bun run build"] {
            let mut event: serde_json::Value = serde_json::from_str(fixture_event).unwrap();
            event["cwd"] = serde_json::json!(&fixture.0);
            event["tool_input"]["command"] = serde_json::json!(literal);
            let vendor_name = match vendor {
                hooks::Vendor::Claude => "claude",
                hooks::Vendor::Codex => "codex",
                hooks::Vendor::Cursor => "cursor",
            };
            let hook = execute(
                cli,
                &["hooks", "run", vendor_name],
                None,
                &serde_json::to_vec(&event).unwrap(),
            );
            assert_eq!(hook.status.code(), Some(0));
            let response: serde_json::Value = serde_json::from_slice(&hook.stdout).unwrap();
            if matches!(vendor, hooks::Vendor::Codex) {
                assert_eq!(
                    response["hookSpecificOutput"]["permissionDecision"],
                    "allow"
                );
            }
            let input = match vendor {
                hooks::Vendor::Claude | hooks::Vendor::Codex => {
                    &response["hookSpecificOutput"]["updatedInput"]
                }
                hooks::Vendor::Cursor => &response["updated_input"],
            };
            let rewritten = input["command"].as_str().unwrap();
            let mut restored = input.clone();
            restored["command"] = serde_json::json!(literal);
            assert_eq!(restored, event["tool_input"]);
            event["tool_input"] = input.clone();
            let repeated = execute(
                cli,
                &["hooks", "run", vendor_name],
                None,
                &serde_json::to_vec(&event).unwrap(),
            );
            assert!(repeated.stdout.is_empty());
            for shell in &shells {
                for lineage in [
                    None,
                    Some(OsStr::new("")),
                    Some(OsStr::new("supplied-value")),
                    Some(OsStr::from_bytes(b"supplied-\xff")),
                ] {
                    let direct = execute(
                        OsStr::new(shell),
                        &["-c", literal],
                        lineage,
                        b"stdin bytes\n",
                    );
                    let wrapped = execute(
                        OsStr::new(shell),
                        &["-c", rewritten],
                        lineage,
                        b"stdin bytes\n",
                    );
                    assert_eq!(direct.status.code(), Some(17), "{shell}");
                    assert_eq!(wrapped.status.code(), direct.status.code(), "{shell}");
                    assert_eq!(wrapped.stdout, direct.stdout, "{shell}");
                    assert!(
                        fixture_stderr_matches(&direct.stderr, &wrapped.stderr),
                        "{shell}: child stderr or outage diagnostic differed: direct={:?}, wrapped={:?}",
                        direct.stderr,
                        wrapped.stderr
                    );
                }
            }
        }
    }
    assert_eq!(
        fs::read_to_string(marker).unwrap().lines().count(),
        48 * shells.len()
    );
    println!("ARC_VENDOR_SEMANTICS_WORKER_COMPLETE");
}

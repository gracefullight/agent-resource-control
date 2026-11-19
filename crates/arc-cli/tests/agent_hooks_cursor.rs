use std::{
    cell::RefCell,
    env, fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

// Exercise the Cursor adapter registered by the shared dispatcher.
#[allow(dead_code)]
#[path = "../src/hooks.rs"]
mod hooks;
use hooks::fixture::cursor;
use hooks::{HookContext, InstallAction, Scope, VerifiedSurface};

#[path = "fixtures/agent_hooks/cursor/installer_concurrency.rs"]
mod concurrency;
#[path = "fixtures/agent_hooks/cursor/installer_tests.rs"]
mod installer;
#[path = "fixtures/agent_hooks/cursor/process.rs"]
mod process;
#[path = "fixtures/agent_hooks/cursor/response_tests.rs"]
mod response;

use serde_json::{json, Value};

struct Fixture {
    root: PathBuf,
    bin: PathBuf,
    home: PathBuf,
    workspace: PathBuf,
    arcctl: PathBuf,
    completions: RefCell<Vec<(String, PathBuf)>>,
}

impl Fixture {
    fn new() -> Self {
        let root = PathBuf::from("/tmp").join(format!(
            "arc-cursor-{}'",
            arc_core::process_identity::random_job_id().unwrap()
        ));
        fs::create_dir(&root).unwrap();
        let bin = root.join("bin");
        let home = root.join("home");
        let workspace = root.join("workspace");
        for path in [&bin, &home, &workspace] {
            fs::create_dir(path).unwrap();
        }
        let arcctl = bin.join("arc'ctl");
        write_executable(&arcctl, "#!/bin/sh\nshift 2\nexec \"$@\"\n");
        for name in ["cargo", "bun"] {
            write_executable(
                &bin.join(name),
                "#!/bin/sh\nprintf 'start\\n' >> \"$CURSOR_TEST_MARKER\"\nprintf '%s\\n' \"$PWD\" \"$CURSOR_TEST_VALUE\" \"$1\" \"${2-}\"\nprintf 'tool-stderr' >&2\nexit 17\n",
            );
        }
        Self {
            root,
            bin,
            home,
            workspace,
            arcctl,
            completions: RefCell::new(Vec::new()),
        }
    }

    fn spawn_worker(&self, scenario: &str, extra: &[(&str, &str)]) -> process::MonitoredChild {
        let completion = self.root.join(format!(
            "completion-{}",
            arc_core::process_identity::random_job_id().unwrap()
        ));
        self.completions
            .borrow_mut()
            .push((scenario.to_owned(), completion.clone()));
        let mut command = Command::new(env::current_exe().unwrap());
        command
            .args(["--exact", "cursor_worker", "--nocapture"])
            .env_clear()
            .env("CURSOR_TEST_SCENARIO", scenario)
            .env("CURSOR_TEST_ROOT", &self.root)
            .env("CURSOR_TEST_BIN", &self.bin)
            .env("CURSOR_TEST_ARC", &self.arcctl)
            .env("CURSOR_TEST_WORKSPACE", &self.workspace)
            .env("HOME", &self.home)
            .env("PATH", &self.bin);
        for (key, value) in extra {
            command.env(key, value);
        }
        if let Some(profile) = env::var_os("LLVM_PROFILE_FILE") {
            command.env("LLVM_PROFILE_FILE", profile);
        }
        command.env("CURSOR_TEST_COMPLETION", completion);
        let _handshake = hooks::fixture::worker_handshake(&mut command);
        process::spawn(&mut command, &format!("worker {scenario}"))
    }

    fn worker(&self, scenario: &str, extra: &[(&str, &str)]) {
        let output = self.spawn_worker(scenario, extra).finish();
        assert!(
            output.status.success(),
            "worker {scenario} failed: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("retained failing Cursor fixture {}", self.root.display());
        } else {
            for (scenario, path) in self.completions.get_mut() {
                let actual = fs::read_to_string(&*path)
                    .unwrap_or_else(|error| panic!("worker {scenario} did not complete: {error}"));
                assert_eq!(actual, format!("completed:{scenario}"));
            }
            fs::remove_dir_all(&self.root).unwrap();
        }
    }
}

fn write_executable(path: &Path, content: &str) {
    fs::write(path, content).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn event() -> Value {
    serde_json::from_str(include_str!(
        "fixtures/agent_hooks/cursor-pre-tool-use.json"
    ))
    .unwrap()
}

fn research() -> Value {
    serde_json::from_str(include_str!("fixtures/agent_hooks/cursor/research.json")).unwrap()
}

fn shell(command: &str, root: &Path, bin: &Path) -> std::process::Output {
    process::nested_output(
        Command::new("/bin/sh")
            .args(["-c", command])
            .env_clear()
            .env("PATH", bin)
            .env("CURSOR_TEST_MARKER", root.join("starts"))
            .env("CURSOR_TEST_VALUE", "preserved value")
            .current_dir(root),
        "shell command exit",
    )
}

fn transformations(root: &Path, bin: &Path, arcctl: &Path) {
    let context = HookContext {
        executable: Some(arcctl.to_path_buf()),
    };
    let surface = VerifiedSurface;
    let marker = root.join("starts");
    for literal in research()["supported_literals"].as_array().unwrap() {
        let literal = literal.as_str().unwrap();
        let mut input = event();
        input["tool_input"]["command"] = json!(literal);
        assert!(cursor::handle(&input, &context).is_some());
        assert!(cursor::handle_verified(&input, &context, None).is_none());
        let response = cursor::handle_verified(&input, &context, Some(&surface)).unwrap();
        assert!(response.get("permission").is_none());
        assert_eq!(response.as_object().unwrap().len(), 1);
        let rewritten = response["updated_input"]["command"].as_str().unwrap();
        assert!(rewritten.ends_with(&format!(" hook-exec -- {literal}")));
        assert!(rewritten.contains("'\\''"));
        let mut preserved = response["updated_input"].clone();
        preserved["command"] = json!(literal);
        assert_eq!(preserved, input["tool_input"]);
        assert!(!marker.exists() || fs::read_to_string(&marker).unwrap().lines().count() == 2);
        input["tool_input"] = response["updated_input"].clone();
        assert!(cursor::handle_verified(&input, &context, Some(&surface)).is_none());
        let direct = shell(literal, root, bin);
        let wrapped = shell(rewritten, root, bin);
        assert_eq!(direct.status.code(), Some(17));
        assert_eq!(wrapped.status.code(), direct.status.code());
        assert_eq!(wrapped.stdout, direct.stdout);
        assert_eq!(wrapped.stderr, direct.stderr);
    }
    assert_eq!(fs::read_to_string(marker).unwrap().lines().count(), 4);

    for literal in research()["unsupported_literals"].as_array().unwrap() {
        let mut input = event();
        input["tool_input"]["command"] = literal.clone();
        assert!(cursor::handle_verified(&input, &context, Some(&surface)).is_none());
    }
    for (field, value) in [
        ("hook_event_name", json!("postToolUse")),
        ("tool_name", json!("Read")),
        ("tool_input", json!({"command": null})),
        ("tool_input", json!([])),
    ] {
        let mut input = event();
        input[field] = value;
        assert!(cursor::handle_verified(&input, &context, Some(&surface)).is_none());
    }
    assert!(cursor::handle_verified(&json!({}), &context, Some(&surface)).is_none());
    let missing = HookContext {
        executable: Some(root.join("missing")),
    };
    assert!(cursor::handle_verified(&event(), &missing, Some(&surface)).is_none());
    let bad_path = HookContext {
        executable: Some(PathBuf::from("relative")),
    };
    assert!(cursor::handle_verified(&event(), &bad_path, Some(&surface)).is_none());
    assert_eq!(
        fs::read_to_string(root.join("starts"))
            .unwrap()
            .lines()
            .count(),
        4
    );
}

#[test]
fn cursor_worker() {
    if !hooks::fixture::worker_enabled() {
        return;
    }
    let scenario = env::var_os("CURSOR_TEST_SCENARIO").unwrap();
    let root = PathBuf::from(env::var_os("CURSOR_TEST_ROOT").unwrap());
    let bin = PathBuf::from(env::var_os("CURSOR_TEST_BIN").unwrap());
    let arcctl = PathBuf::from(env::var_os("CURSOR_TEST_ARC").unwrap());
    let workspace = PathBuf::from(env::var_os("CURSOR_TEST_WORKSPACE").unwrap());
    let context = HookContext {
        executable: Some(arcctl.clone()),
    };
    match scenario.to_str().unwrap() {
        "transformations" => transformations(&root, &bin, &arcctl),
        "schema-candidates" => response::schema_candidates(&root, &bin, &arcctl),
        "approval-decisions" => response::approval_decisions(&root, &bin, &arcctl),
        "bypass" => {
            let surface = VerifiedSurface;
            assert!(cursor::handle_verified(&event(), &context, Some(&surface)).is_none());
            assert!(!root.join("starts").exists());
        }
        "unverified" => {
            assert!(cursor::handle(&event(), &context).is_some());
            assert!(cursor::handle_verified(&event(), &context, None).is_none());
            assert!(!root.join("starts").exists());
        }
        "install-user" | "uninstall-user" | "install-workspace" | "uninstall-workspace" => {
            if env::var_os("CURSOR_TEST_RESTRICTIVE_UMASK").is_some() {
                // SAFETY: this isolated worker owns its process umask.
                unsafe { libc::umask(0o300) };
            }
            let scenario = scenario.to_str().unwrap();
            let action = if scenario.starts_with("install-") {
                InstallAction::Install
            } else {
                InstallAction::Uninstall
            };
            let (scope, workspace) = if scenario.ends_with("-user") {
                (Scope::User, None)
            } else {
                (Scope::Workspace, Some(workspace.as_path()))
            };
            let result = if env::var_os("CURSOR_TEST_CONTROL_SOCKET").is_some() {
                concurrency::controlled_configure(action, scope, workspace, &context)
            } else if env::var_os("CURSOR_INTERVENING_EDIT").is_some() {
                let dir = if matches!(scope, Scope::User) {
                    PathBuf::from(env::var_os("HOME").unwrap())
                } else {
                    workspace.unwrap().to_path_buf()
                };
                let path = dir.join(".cursor/hooks.json");
                cursor::configure_intercepted(
                    action,
                    scope,
                    workspace,
                    &context,
                    || {},
                    || {
                        use std::os::unix::fs::MetadataExt;
                        let old = fs::metadata(&path).unwrap();
                        let source = fs::read_to_string(&path).unwrap();
                        let edited = source.replacen('{', "{\"intervening\":\"preserve me\",", 1);
                        fs::write(&path, edited).unwrap();
                        let current = fs::metadata(&path).unwrap();
                        assert_eq!((old.dev(), old.ino()), (current.dev(), current.ino()));
                    },
                )
            } else {
                cursor::configure(action, scope, workspace, &context)
            };
            if let Ok(expected) = env::var("CURSOR_EXPECT_ERROR_MESSAGE") {
                assert_eq!(result.unwrap_err(), expected);
            } else if env::var_os("CURSOR_EXPECT_ERROR").is_some() {
                assert!(result.is_err());
            } else {
                result.unwrap();
            }
        }
        other => panic!("unknown worker scenario {other}"),
    }
    fs::write(
        env::var_os("CURSOR_TEST_COMPLETION").unwrap(),
        format!("completed:{}", scenario.to_str().unwrap()),
    )
    .unwrap();
}

#[test]
fn fixture_transforms_only_supported_verified_events_without_running_them() {
    Fixture::new().worker("transformations", &[]);
}

#[test]
fn disabled_and_remote_events_pass_through_even_with_a_fixture_surface() {
    for variable in [
        ("ARC_HOOKS_DISABLED", "1"),
        ("ARC_HOOKS_CURSOR_DISABLED", "1"),
        ("CURSOR_CODE_REMOTE", "true"),
        ("SSH_CONNECTION", "remote"),
    ] {
        Fixture::new().worker("bypass", &[variable]);
    }
}

#[test]
fn shell_startup_does_not_run_during_the_hook() {
    let fixture = Fixture::new();
    let startup = fixture.root.join("startup");
    fs::write(
        &startup,
        "printf unexpected > \"$CURSOR_TEST_ROOT/starts\"\n",
    )
    .unwrap();
    fixture.worker(
        "unverified",
        &[
            ("BASH_ENV", startup.to_str().unwrap()),
            ("ENV", startup.to_str().unwrap()),
            ("BASH_FUNC_cargo%%", "() { echo overridden; }"),
        ],
    );
}

#[test]
fn production_cli_rewrites_valid_events_and_fails_open_for_invalid_json() {
    let fixture = Fixture::new();
    let event_path = fixture.root.join("event.json");
    for (input, status) in [
        (
            include_str!("fixtures/agent_hooks/cursor-pre-tool-use.json"),
            0,
        ),
        ("{invalid json", 1),
    ] {
        fs::write(&event_path, input).unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_arcctl"));
        command
            .args(["hooks", "run", "cursor"])
            .env_clear()
            .env("HOME", &fixture.home)
            .env("PATH", &fixture.bin)
            .env("CURSOR_TEST_MARKER", fixture.root.join("starts"))
            .current_dir(&fixture.workspace)
            .stdin(Stdio::from(fs::File::open(&event_path).unwrap()));
        if let Some(profile) = env::var_os("LLVM_PROFILE_FILE") {
            command.env("LLVM_PROFILE_FILE", profile);
        }
        let output = process::output(&mut command, "production hook exit");
        assert_eq!(output.status.code(), Some(status));
        if status == 0 {
            let response: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert!(response.get("permission").is_none());
        } else {
            assert!(output.stdout.is_empty());
        }
        assert!(!fixture.root.join("starts").exists());
    }
}

fn installer_roundtrip(scope: &str, source: &str) {
    let fixture = Fixture::new();
    let root = if scope == "user" {
        &fixture.home
    } else {
        &fixture.workspace
    };
    let dir = root.join(".cursor");
    fs::create_dir(&dir).unwrap();
    let path = dir.join("hooks.json");
    fs::write(&path, source).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
    let original: Value = serde_json::from_str(source).unwrap();
    let install = format!("install-{scope}");
    let uninstall = format!("uninstall-{scope}");
    fixture.worker(&install, &[]);
    let installed: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    let entries = installed["hooks"]["preToolUse"].as_array().unwrap();
    assert_eq!(
        &entries[..2],
        original["hooks"]["preToolUse"].as_array().unwrap()
    );
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[2]["matcher"], "^Shell$");
    assert!(entries[2].get("failClosed").is_none());
    assert_eq!(
        entries[2]["command"],
        format!(
            "'{}' hooks run cursor # ARC_CURSOR_HOOK_V1",
            fixture.arcctl.to_str().unwrap().replace('\'', "'\\''")
        )
    );
    fixture.worker(&install, &[]);
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(&path).unwrap()).unwrap(),
        installed
    );
    let mut with_unrelated = installed.clone();
    let unrelated = json!({
        "command": "/usr/bin/printf unrelated-hook",
        "matcher": "^Shell$",
        "timeout": 17
    });
    with_unrelated["hooks"]["preToolUse"]
        .as_array_mut()
        .unwrap()
        .push(unrelated.clone());
    fs::write(&path, serde_json::to_vec_pretty(&with_unrelated).unwrap()).unwrap();
    let mut expected = original.clone();
    expected["hooks"]["preToolUse"]
        .as_array_mut()
        .unwrap()
        .push(unrelated);
    fixture.worker(&uninstall, &[]);
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(&path).unwrap()).unwrap(),
        expected
    );
    fixture.worker(&uninstall, &[]);
    assert_eq!(
        serde_json::from_slice::<Value>(&fs::read(&path).unwrap()).unwrap(),
        expected
    );
    assert_eq!(
        fs::metadata(path).unwrap().permissions().mode() & 0o777,
        0o640
    );
    assert!(!fixture.root.join("starts").exists());
}

#[test]
fn isolated_user_install_and_uninstall_preserve_settings_and_order() {
    installer_roundtrip(
        "user",
        include_str!("fixtures/agent_hooks/cursor/user.json"),
    );
}

#[test]
fn isolated_workspace_install_and_uninstall_preserve_settings_and_order() {
    installer_roundtrip(
        "workspace",
        include_str!("fixtures/agent_hooks/cursor/workspace.json"),
    );
}

#[test]
fn installer_handles_absent_and_invalid_configuration_without_corruption() {
    let fixture = Fixture::new();
    let dir = fixture.home.join(".cursor");
    let path = dir.join("hooks.json");
    fixture.worker("uninstall-user", &[]);
    assert!(!dir.exists());
    fixture.worker("install-user", &[]);
    let installed: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(installed["version"], 1);
    assert_eq!(
        installed["hooks"]["preToolUse"].as_array().unwrap().len(),
        1
    );
    fixture.worker("uninstall-user", &[]);
    let uninstalled: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    assert_eq!(uninstalled["hooks"]["preToolUse"], json!([]));
    for invalid in [
        "{",
        "[]",
        "{\"version\":2,\"hooks\":{}}",
        "{\"version\":1,\"hooks\":[]}",
        "{\"version\":1,\"hooks\":{\"preToolUse\":{}}}",
    ] {
        fs::write(&path, invalid).unwrap();
        fixture.worker("install-user", &[("CURSOR_EXPECT_ERROR", "1")]);
        assert_eq!(fs::read(&path).unwrap(), invalid.as_bytes());
    }
    let valid = include_str!("fixtures/agent_hooks/cursor/user.json");
    fs::write(&path, valid).unwrap();
    fs::set_permissions(&fixture.arcctl, fs::Permissions::from_mode(0o644)).unwrap();
    fixture.worker("install-user", &[("CURSOR_EXPECT_ERROR", "1")]);
    assert_eq!(fs::read(&path).unwrap(), valid.as_bytes());
}

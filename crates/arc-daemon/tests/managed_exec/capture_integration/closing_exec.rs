use super::*;
use std::os::unix::net::UnixListener;

#[path = "closing_exec/edge_cases.rs"]
mod edge_cases;
#[path = "closing_exec/stages.rs"]
mod stages;

fn output_with_input(command: &mut Command, input: &[u8], label: &str) -> std::process::Output {
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = child_watchdog::Owned::spawn(command).unwrap();
    child.child.stdin.take().unwrap().write_all(input).unwrap();
    let status = child.wait(label).unwrap();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    child
        .child
        .stdout
        .take()
        .unwrap()
        .read_to_end(&mut stdout)
        .unwrap();
    child
        .child
        .stderr
        .take()
        .unwrap()
        .read_to_end(&mut stderr)
        .unwrap();
    std::process::Output {
        status,
        stdout,
        stderr,
    }
}

fn shell_command(s: &Sandbox, f: &Fixture, source: &str) -> Command {
    shell_command_for("/bin/sh", s, f, source)
}

fn shell_command_for(shell: &str, s: &Sandbox, f: &Fixture, source: &str) -> Command {
    let template = s.command(f, &[]);
    let mut command = Command::new(shell);
    command
        .env_clear()
        .env("ZDOTDIR", &s.home)
        .env("LOGNAME", "arc-fixture")
        .env("USER", "arc-fixture")
        .args(["-c", source])
        .current_dir(&s.repo);
    for (key, value) in template.get_envs() {
        if let Some(value) = value {
            command.env(key, value);
        }
    }
    command
}

fn job(f: &Fixture) -> Value {
    let id = cli::recent(f);
    let response = protocol::call(
        f,
        OPERATION_JOB_STATUS_LOCALITY,
        json!(StatusRequest::ById { job_id: id }),
    );
    assert_eq!(response.status, "ok", "{response:?}");
    response.result.unwrap()["job"].clone()
}

#[test]
fn installed_claude_hook_executes_same_remote_child_with_held_build_permit() {
    installed_claude_hook_with_held_build_permit("/bin/sh", false, None);
}

#[test]
fn installed_claude_hook_bash_executes_same_remote_child_with_held_build_permit() {
    installed_claude_hook_with_held_build_permit("/bin/bash", false, None);
}

#[test]
fn installed_claude_hook_zsh_executes_same_remote_child_with_held_build_permit() {
    installed_claude_hook_with_held_build_permit("/bin/zsh", false, None);
}

#[test]
fn installed_claude_hook_zsh_preserves_environment_when_shell_continues() {
    installed_claude_hook_with_held_build_permit("/bin/zsh", true, None);
}

#[test]
fn installed_claude_hook_zsh_executes_with_builtin_function() {
    installed_claude_hook_with_held_build_permit(
        "/bin/zsh",
        false,
        Some("builtin() { print -u2 unexpected-builtin; return 42; }\n"),
    );
}

#[test]
fn installed_claude_hook_zsh_manages_gh_with_shadowed_helpers() {
    installed_claude_hook_with_held_build_permit(
        "/bin/zsh",
        true,
        Some("builtin() { print -u2 unexpected-builtin; return 42; }\neval() { print -u2 unexpected-eval; return 43; }\nunset() { print -u2 unexpected-unset; return 44; }\nprintf() { print -u2 unexpected-printf; return 45; }\n"),
    );
}

#[test]
fn installed_claude_hook_zsh_manages_gh_with_posix_builtins_already_enabled() {
    installed_claude_hook_with_held_build_permit(
        "/bin/zsh",
        true,
        Some("setopt POSIX_BUILTINS\nbuiltin() { print -u2 unexpected-builtin; return 42; }\neval() { print -u2 unexpected-eval; return 43; }\nunset() { print -u2 unexpected-unset; return 44; }\n"),
    );
}

#[test]
fn installed_claude_hook_zsh_manages_gh_with_shadowed_exit() {
    installed_claude_hook_with_held_build_permit(
        "/bin/zsh",
        false,
        Some("exit() { print -u2 unexpected-exit; return 46; }\n"),
    );
}

fn installed_claude_hook_with_held_build_permit(
    shell: &str,
    continues: bool,
    startup: Option<&str>,
) {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    if let Some(startup) = startup {
        write(&s.home.join(".zshenv"), startup);
    }
    super::endpoints::install(&s);
    write(
        &s.repo.join(".git/config"),
        "[remote \"origin\"]\n url = https://github.com/example/app.git\n",
    );
    symlink(cli::binary("arc-exec-fixture"), s.bin.join("cargo")).unwrap();
    let mut daemon = f.spawn_isolated("test");
    let listener = UnixListener::bind(f.root.join("held-build")).unwrap();
    let mut held = s.command(&f, &[]);
    held.env("ARC_FIXTURE_CONTROL", f.root.join("held-build"))
        .env("ARC_FIXTURE_MARKER", f.root.join("held-calls"))
        .args(["exec", "--", "cargo", "build"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut local =
        super::super::runtime::OwnedChild::spawn_with_descendants(held, f.root.join("held-owners"))
            .unwrap();
    let (mut release, child) = cli::accept(&listener);
    local.own_process(serde_json::from_value(child["identity"].clone()).unwrap());
    let db = rusqlite::Connection::open(&f.database).unwrap();
    let counts = || {
        db.query_row("SELECT (SELECT count(*) FROM traffic_entries), (SELECT count(*) FROM traffic_entries WHERE state='QUEUED'), (SELECT count(*) FROM permits), (SELECT count(*) FROM permits WHERE state='ACTIVE'), (SELECT count(*) FROM jobs WHERE queue_at IS NOT NULL OR reserved_at IS NOT NULL)", [], |r| Ok((r.get::<_, i64>(0)?,r.get::<_, i64>(1)?,r.get::<_, i64>(2)?,r.get::<_, i64>(3)?,r.get::<_, i64>(4)?))).unwrap()
    };
    let before = counts();
    assert_eq!((before.1, before.2, before.3), (0, 1, 1));

    let install = child_watchdog::output(
        s.command(&f, &[])
            .args(["hooks", "install", "claude", "--scope", "user"]),
        "install Claude hook",
    )
    .unwrap();
    assert!(install.status.success(), "{install:?}");
    let settings: Value =
        serde_json::from_slice(&fs::read(s.home.join(".claude/settings.json")).unwrap()).unwrap();
    let installed = settings["hooks"]["PreToolUse"][0]["hooks"][0]["command"]
        .as_str()
        .unwrap();
    assert_eq!(settings["hooks"]["PreToolUse"][0]["matcher"], "Bash");
    let mut event: Value = serde_json::from_str(include_str!(
        "../../../../arc-cli/tests/fixtures/agent_hooks/claude-recorded-2.1.327-cargo-build.json"
    ))
    .unwrap();
    event["cwd"] = json!(s.repo);
    event["tool_input"]["command"] = json!("gh workflow run build.yml");
    let response = output_with_input(
        &mut shell_command_for(shell, &s, &f, installed),
        &serde_json::to_vec(&event).unwrap(),
        "installed hook",
    );
    assert!(response.status.success(), "{response:?}");
    assert!(response.stderr.is_empty());
    let response: Value = serde_json::from_slice(&response.stdout).unwrap();
    let hook = &response["hookSpecificOutput"];
    assert!(hook.get("permissionDecision").is_none());
    let updated = &hook["updatedInput"];
    let rewritten = updated["command"].as_str().unwrap();
    assert!(rewritten.contains("--arc-check-gh-path"));
    assert!(rewritten.contains(" hook-exec -- gh workflow run build.yml"));
    let mut restored = updated.clone();
    restored["command"] = event["tool_input"]["command"].clone();
    assert_eq!(restored, event["tool_input"]);
    assert_eq!(s.calls(), 0, "hook itself must not execute the command");

    let key = f.root.join("hook-capture-key");
    write(&key, b"arc-hook-fixture-capture-v1");
    let marker = f.root.join("hook-gh-calls");
    let record = f.root.join("hook-gh-observation.json");
    let option_record = f.root.join("hook-zsh-option");
    let source = if continues {
        format!("{rewritten}; _arc_test_status=$?; print -r -- \"${{options[POSIX_BUILTINS]}}\" > \"$ARC_HOOK_OPTION_RECORD\"; exit $_arc_test_status")
    } else {
        rewritten.to_owned()
    };
    let mut native_command = shell_command_for(shell, &s, &f, &source);
    native_command
        .env("ARC_FIXTURE_MODE", "hook-capture-pid")
        .env("ARC_HOOK_CAPTURE_KEY", &key)
        .env("ARC_HOOK_CAPTURE_MARKER", &marker)
        .env("ARC_HOOK_CAPTURE_RECORD", &record);
    if continues {
        native_command.env("ARC_HOOK_OPTION_RECORD", &option_record);
    }
    // Observe the vendor shell's environment separately. In particular, the
    // platform shell may add PWD/SHLVL or strip loader variables on startup.
    let probe_source = if continues {
        "/usr/bin/env -0; exit $?"
    } else {
        "/usr/bin/env -0"
    };
    let mut probe = shell_command_for(shell, &s, &f, probe_source);
    probe
        .env("ARC_FIXTURE_MODE", "hook-capture-pid")
        .env("ARC_HOOK_CAPTURE_KEY", &key)
        .env("ARC_HOOK_CAPTURE_MARKER", &marker)
        .env("ARC_HOOK_CAPTURE_RECORD", &record);
    if continues {
        probe.env("ARC_HOOK_OPTION_RECORD", &option_record);
    }
    let shell_env = output_with_input(&mut probe, b"", "shell environment probe");
    assert!(shell_env.status.success());
    let mut expected_environment = std::collections::BTreeMap::new();
    for entry in shell_env
        .stdout
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
    {
        let entry = std::str::from_utf8(entry).unwrap();
        let (name, value) = entry.split_once('=').unwrap();
        expected_environment.insert(name.to_owned(), value.to_owned());
    }
    // Shells that export `_` set it to the command being invoked.
    if expected_environment
        .get("_")
        .is_some_and(|value| value == "/usr/bin/env")
    {
        expected_environment.insert("_".into(), s.bin.join("gh").to_str().unwrap().to_owned());
    }
    instrumentation::include_expected(&mut expected_environment);
    let output = output_with_input(&mut native_command, b"input\n", "returned hook command");
    assert_eq!(output.status.code(), Some(17), "{shell}: {output:?}");
    assert_eq!(output.stdout, b"hook-stdout\0payload\n");
    assert_eq!(output.stderr, b"child-stderr\n");
    if continues {
        let expected = if startup.is_some_and(|source| source.contains("setopt POSIX_BUILTINS")) {
            "on\n"
        } else {
            "off\n"
        };
        assert_eq!(fs::read_to_string(&option_record).unwrap(), expected);
    }
    let native: Value = serde_json::from_slice(&fs::read(&record).unwrap()).unwrap();
    assert_eq!(
        native["args"],
        json!([b"workflow".to_vec(), b"run".to_vec(), b"build.yml".to_vec()])
    );
    assert_eq!(
        native["cwd"],
        json!(fs::canonicalize(&s.repo).unwrap().as_os_str().as_bytes())
    );
    assert_eq!(native["stdin"], json!(b"input\n".to_vec()));
    let mut actual_environment = std::collections::BTreeMap::new();
    for entry in native["environment"].as_array().unwrap() {
        let bytes = |value: &Value| -> String {
            let bytes: Vec<u8> = serde_json::from_value(value.clone()).unwrap();
            String::from_utf8(bytes).unwrap()
        };
        actual_environment.insert(bytes(&entry[0]), bytes(&entry[1]));
    }
    instrumentation::normalize_actual(&mut actual_environment, &expected_environment);
    assert_eq!(actual_environment, expected_environment);
    let calls = fs::read_to_string(&marker).unwrap();
    assert_eq!(calls.lines().count(), 1, "offline gh must run exactly once");
    let pid: u64 = calls.trim().parse().unwrap();
    assert_eq!(native["pid"], pid);
    let recorded = job(&f);
    assert_eq!(
        recorded["classification"]["classification"], "REMOTE_CI",
        "{recorded}"
    );
    assert_eq!(recorded["classification"]["locality"], "REMOTE");
    assert_eq!(recorded["child"]["pid"], pid);
    assert_eq!(recorded["outcome"]["code"], 17);
    assert_eq!(
        counts(),
        before,
        "remote execution created local scheduling state"
    );
    drop(db);
    release.write_all(b"x").unwrap();
    assert_eq!(local.wait_with_output().unwrap().status.code(), Some(17));
    daemon.stop(libc::SIGTERM);
    f.report();
}

include!("closing_exec/regressions.rs");
include!("closing_exec/zsh_resolution.rs");

#[test]
fn injected_capture_delay_keeps_daemon_registration_and_child_identity() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    super::endpoints::install(&s);
    write(
        &s.repo.join(".git/config"),
        "[remote \"origin\"]\n url = https://github.com/example/app.git\n",
    );
    let mut daemon = f.spawn_isolated("test");
    let template = s.command(&f, &[]);
    let mut command = Command::new(cli::binary("arc-exec-fixture"));
    command.env_clear().current_dir(&s.repo).args([
        "--delayed-capture-exec",
        "gh",
        "workflow",
        "run",
        "build.yml",
    ]);
    for (key, value) in template.get_envs() {
        if let Some(value) = value {
            command.env(key, value);
        }
    }
    let output = output_with_input(&mut command, b"input\n", "injected capture delay");
    assert_eq!(output.status.code(), Some(17), "{output:?}");
    assert_eq!(output.stderr, b"fixture-stderr\n", "{output:?}");
    assert_eq!(s.calls(), 1);
    let pid: u64 = fs::read_to_string(&s.marker)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let recorded = job(&f);
    assert_eq!(
        recorded["classification"]["classification"], "REMOTE_CI",
        "{recorded}"
    );
    assert_eq!(recorded["child"]["pid"], pid);
    assert_eq!(recorded["outcome"]["code"], 17);
    assert_eq!(admission_counts(&f), (0, 0));
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn nested_bash_startup_changes_are_unknown_in_cli_and_execution() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    super::endpoints::install(&s);
    write(
        &s.repo.join(".git/config"),
        "[remote \"origin\"]\n url = https://github.com/example/app.git\n",
    );
    symlink("/bin/bash", s.bin.join("bash")).unwrap();
    let startup = s.home.join("bash-env");
    let profile = s.home.join(".bash_profile");
    let body = "export GH_HOST=localhost\nexport PATH=\"$FIXTURE_BIN\"\nprintf 'startup\\n' >> \"$STARTUP_MARKER\"\n";
    write(&startup, body);
    write(&profile, body);
    let mut daemon = f.spawn_isolated("test");
    for (flag, env_file) in [("-c", Some(startup.as_path())), ("-lc", None)] {
        let marker = s.home.join(format!("startup-{}", &flag[1..]));
        let marker_text = marker.to_str().unwrap();
        let bin_text = s.bin.to_str().unwrap();
        let env_text = env_file.map(|p| p.to_str().unwrap());
        let env = [
            ("BASH_ENV", env_text),
            ("FIXTURE_BIN", Some(bin_text)),
            ("STARTUP_MARKER", Some(marker_text)),
        ];
        let argv = ["bash", flag, "gh workflow run build.yml"];
        let inspection = s.classify(&f, &argv, &env);
        assert_eq!(
            inspection["classification"], "UNKNOWN",
            "{flag}: {inspection}"
        );
        assert_eq!(inspection["uncertainty"], "OBSERVE");
        assert!(!marker.exists(), "classification sourced startup file");
        let mut command = s.command(&f, &env);
        command.args(["exec", "--"]).args(argv);
        let output = output_with_input(&mut command, b"input\n", "nested bash execution");
        assert_eq!(output.status.code(), Some(17), "{flag}: {output:?}");
        assert_eq!(output.stderr, b"fixture-stderr\n", "{flag}: {output:?}");
        assert_eq!(
            fs::read_to_string(&marker).unwrap(),
            "startup\n",
            "{flag} startup did not run"
        );
        let recorded = job(&f);
        assert_routes(flag, &inspection, &recorded, "UNKNOWN");
        assert_eq!(recorded["outcome"]["code"], 17);
        assert_eq!(admission_counts(&f), (0, 0));
    }
    assert_eq!(s.calls(), 2);
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn actual_daemon_outage_still_runs_the_offline_child_once() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    assert!(!f.socket.exists());
    let mut command = s.command(&f, &[]);
    command.args(["exec", "--", "gh", "workflow", "run", "build.yml"]);
    let output = output_with_input(&mut command, b"input\n", "genuine daemon outage");
    assert_eq!(output.status.code(), Some(17), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("TRACKING_UNAVAILABLE"));
    assert!(output.stderr.ends_with(b"fixture-stderr\n"));
    assert_eq!(s.calls(), 1);
}

#[test]
fn literal_sh_child_is_local_test_on_both_production_routes() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    symlink("/bin/sh", s.bin.join("sh")).unwrap();
    symlink(cli::binary("arc-exec-fixture"), s.bin.join("cargo")).unwrap();
    let mut daemon = f.spawn_isolated("test");
    let argv = ["sh", "-c", "cargo test"];
    let inspection = s.classify(&f, &argv, &[]);
    assert_eq!(inspection["classification"], "TEST", "{inspection}");
    let mut command = s.command(&f, &[]);
    command.args(["exec", "--"]).args(argv);
    let output = output_with_input(&mut command, b"input\n", "native sh execution");
    assert_eq!(output.status.code(), Some(17), "{output:?}");
    assert_eq!(output.stderr, b"fixture-stderr\n", "{output:?}");
    let recorded = job(&f);
    assert_eq!(
        recorded["classification"]["classification"], "TEST",
        "{recorded}"
    );
    assert_eq!(recorded["classification"]["locality"], "HOST_LOCAL");
    assert_eq!(recorded["outcome"]["code"], 17);
    assert_eq!(s.calls(), 1);
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn unknown_child_runs_once_with_native_arguments_and_recorded_outcome() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    symlink(cli::binary("arc-exec-fixture"), s.bin.join("custom")).unwrap();
    let mut daemon = f.spawn_isolated("test");
    let argv = ["custom", "two words", "--literal"];
    let inspection = s.classify(&f, &argv, &[]);
    let (job, _) = s.exec(&f, &argv, &[]);
    assert!(!assert_routes("custom", &inspection, &job, "UNKNOWN"));
    assert_eq!(job["outcome"]["code"], 17);
    assert_eq!(s.calls(), 1);
    assert_eq!(admission_counts(&f), (0, 0));
    daemon.stop(libc::SIGTERM);
    f.report();
}

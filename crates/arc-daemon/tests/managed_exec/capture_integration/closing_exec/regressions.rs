fn hook_status_variable(rewritten: &str) -> String {
    format!(
        "_ARC_HOOK_STATUS_{}",
        rewritten
            .split_once("case \"${_ARC_HOOK_STATUS_")
            .unwrap()
            .1
            .split_once("+x}")
            .unwrap()
            .0
    )
}

#[test]
fn installed_hook_preserves_sh_functions_and_aliases() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    let install = child_watchdog::output(
        s.command(&f, &[])
            .args(["hooks", "install", "claude", "--scope", "user"]),
        "install Claude hook for sh resolution",
    )
    .unwrap();
    assert!(install.status.success(), "{install:?}");
    let settings: Value =
        serde_json::from_slice(&fs::read(s.home.join(".claude/settings.json")).unwrap()).unwrap();
    let installed = settings["hooks"]["PreToolUse"][0]["hooks"][0]["command"]
        .as_str()
        .unwrap();
    let mut event: Value = serde_json::from_str(include_str!(
        "../../../../../arc-cli/tests/fixtures/agent_hooks/claude-recorded-2.1.327-cargo-build.json"
    ))
    .unwrap();
    event["cwd"] = json!(s.repo);
    event["tool_input"]["command"] = json!("gh workflow run build.yml");
    let response = output_with_input(
        &mut shell_command(&s, &f, installed),
        &serde_json::to_vec(&event).unwrap(),
        "installed hook for sh resolution",
    );
    assert!(response.status.success(), "{response:?}");
    let response: Value = serde_json::from_slice(&response.stdout).unwrap();
    assert!(response["hookSpecificOutput"]
        .get("permissionDecision")
        .is_none());
    let rewritten = response["hookSpecificOutput"]["updatedInput"]["command"]
        .as_str()
        .unwrap();
    for (name, setup, expected_code) in [
        (
            "function",
            "gh() { printf 'function:%s:%s:%s:%s:%s:%s\\n' \"$?\" \"$1\" \"$2\" \"$3\" \"$PWD\" \"$PRIVATE_ENV\"; IFS= read -r line; printf 'stdin:%s\\n' \"$line\"; printf 'function-stderr\\n' >&2; return 37; }\nfalse",
            37,
        ),
        (
            "alias",
            "fixture_gh() { printf 'alias:%s:%s:%s:%s:%s:%s\\n' \"$?\" \"$1\" \"$2\" \"$3\" \"$PWD\" \"$PRIVATE_ENV\"; IFS= read -r line; printf 'stdin:%s\\n' \"$line\"; printf 'alias-stderr\\n' >&2; return 38; }\nalias gh=fixture_gh\nfalse",
            38,
        ),
    ] {
        let run = |source: &str| {
            output_with_input(
                &mut shell_command(&s, &f, &format!("{setup}\n{source}")),
                b"input\n",
                name,
            )
        };
        let direct = run("gh workflow run build.yml");
        let wrapped = run(rewritten);
        assert_eq!(direct.status.code(), Some(expected_code), "{name}: {direct:?}");
        assert_eq!(wrapped.status, direct.status, "{name}: {wrapped:?}");
        assert_eq!(wrapped.stdout, direct.stdout, "{name}");
        assert_eq!(wrapped.stderr, direct.stderr, "{name}");
        assert_eq!(s.calls(), 0, "{name}: PATH stub must not be invoked");
    }
}

#[test]
fn installed_hook_preserves_bash_startup_functions_aliases_and_hashed_commands() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    let install = child_watchdog::output(
        s.command(&f, &[])
            .args(["hooks", "install", "claude", "--scope", "user"]),
        "install Claude hook for shell resolution",
    )
    .unwrap();
    assert!(install.status.success(), "{install:?}");
    let settings: Value =
        serde_json::from_slice(&fs::read(s.home.join(".claude/settings.json")).unwrap()).unwrap();
    let installed = settings["hooks"]["PreToolUse"][0]["hooks"][0]["command"]
        .as_str()
        .unwrap();
    let mut event: Value = serde_json::from_str(include_str!(
        "../../../../../arc-cli/tests/fixtures/agent_hooks/claude-recorded-2.1.327-cargo-build.json"
    ))
    .unwrap();
    event["cwd"] = json!(s.repo);
    event["tool_input"]["command"] = json!("gh workflow run build.yml");
    let response = output_with_input(
        &mut shell_command(&s, &f, installed),
        &serde_json::to_vec(&event).unwrap(),
        "installed hook for shell resolution",
    );
    assert!(response.status.success(), "{response:?}");
    let response: Value = serde_json::from_slice(&response.stdout).unwrap();
    assert!(response["hookSpecificOutput"]
        .get("permissionDecision")
        .is_none());
    let rewritten = response["hookSpecificOutput"]["updatedInput"]["command"]
        .as_str()
        .unwrap();
    let status_variable = hook_status_variable(rewritten);
    let alternative = s.home.join("other-gh");
    write(
        &alternative,
        "#!/bin/sh\nprintf 'hashed:%s:%s:%s\\n' \"$1\" \"$2\" \"$3\"\nIFS= read -r line\nprintf 'stdin:%s\\n' \"$line\"\nprintf 'hashed-stderr\\n' >&2\nexit 39\n",
    );
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&alternative, fs::Permissions::from_mode(0o755)).unwrap();
    for (name, startup, expected_code) in [
        (
            "function",
            "gh() { printf 'function:%s:%s:%s\\n' \"$1\" \"$2\" \"$3\"; IFS= read -r line; printf 'stdin:%s\\n' \"$line\"; printf 'function-stderr\\n' >&2; return 37; }\n".to_owned(),
            37,
        ),
        (
            "function-prior-status",
            "gh() { printf 'prior-status:%s underscore:%s\\n' \"$?\" \"$_\"; return 37; }\nfalse\n".to_owned(),
            37,
        ),
        (
            "function-quoted-underscore",
            "gh() { printf 'prior-status:%s underscore:%s\\n' \"$?\" \"$_\"; return 37; }\n: 'two words; $(touch forbidden)'\n".to_owned(),
            37,
        ),
        (
            "builtin-and-eval-functions",
            "builtin() { printf 'unexpected-builtin\\n' >&2; return 42; }\neval() { printf 'unexpected-eval\\n' >&2; return 43; }\ngh() { printf 'gh-function:%s:%s:%s:%s\\n' \"$?\" \"$1\" \"$2\" \"$3\"; IFS= read -r line; printf 'stdin:%s\\n' \"$line\"; printf 'gh-stderr\\n' >&2; return 37; }\nfalse\n".to_owned(),
            37,
        ),
        (
            "builtin-and-unset-functions",
            "builtin() { printf 'unexpected-builtin\\n' >&2; return 42; }\nunset() { printf 'unexpected-unset\\n' >&2; return 44; }\ngh() { printf 'gh-function:%s:%s:%s:%s\\n' \"$?\" \"$1\" \"$2\" \"$3\"; IFS= read -r line; printf 'stdin:%s\\n' \"$line\"; printf 'gh-stderr\\n' >&2; return 37; }\nfalse\n".to_owned(),
            37,
        ),
        (
            "existing-temporary-name-and-shadowed-eval",
            format!("export {status_variable}=caller-value\nbuiltin() {{ printf 'unexpected-builtin\\n' >&2; return 42; }}\neval() {{ printf 'unexpected-eval\\n' >&2; return 43; }}\ngh() {{ printf 'value:%s status:%s underscore:%s\\n' \"${{{status_variable}}}\" \"$?\" \"$_\"; return 40; }}\nfalse\n"),
            40,
        ),
        (
            "existing-temporary-name-and-shell-syntax-alias",
            format!("export {status_variable}=caller-value\neval() {{ printf 'unexpected-eval\\n' >&2; return 43; }}\nshopt -s expand_aliases\nalias gh='printf alias-output; #'\n"),
            0,
        ),
        (
            "colon-alias",
            "shopt -s expand_aliases\nalias :='printf unexpected-alias'\ngh() { printf 'prior-status:%s\\n' \"$?\"; return 37; }\nfalse\n".to_owned(),
            37,
        ),
        (
            "alias",
            "shopt -s expand_aliases\nfixture_gh() { printf 'alias:%s:%s:%s\\n' \"$1\" \"$2\" \"$3\"; IFS= read -r line; printf 'stdin:%s\\n' \"$line\"; printf 'alias-stderr\\n' >&2; return 38; }\nalias gh=fixture_gh\n".to_owned(),
            38,
        ),
        (
            "alias-shell-syntax",
            "shopt -s expand_aliases\nalias gh='printf alias-output; #'\n".to_owned(),
            0,
        ),
        (
            "hash",
            format!("hash -p '{}' gh\n", alternative.display()),
            39,
        ),
    ] {
        let startup_path = s.home.join(format!("startup-{name}"));
        write(&startup_path, startup);
        let run = |source: &str| {
            let template = s.command(&f, &[]);
            let mut command = Command::new("/bin/bash");
            command
                .env_clear()
                .env("BASH_ENV", &startup_path)
                .args(["-c", source])
                .current_dir(&s.repo);
            for (key, value) in template.get_envs() {
                if let Some(value) = value {
                    command.env(key, value);
                }
            }
            output_with_input(&mut command, b"input\n", name)
        };
        let direct = run("gh workflow run build.yml");
        let wrapped = run(rewritten);
        assert_eq!(direct.status.code(), Some(expected_code), "{name}");
        assert_eq!(wrapped.status.code(), direct.status.code(), "{name}");
        assert_eq!(wrapped.stdout, direct.stdout, "{name}");
        assert_eq!(wrapped.stderr, direct.stderr, "{name}");
        assert_eq!(s.calls(), 0, "{name}: PATH stub must not be invoked");
        assert!(!s.repo.join("forbidden").exists(), "{name}");
    }
}

#[test]
fn installed_hook_preserves_zsh_functions_aliases_and_hashed_commands() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    let install = child_watchdog::output(
        s.command(&f, &[])
            .args(["hooks", "install", "claude", "--scope", "user"]),
        "install Claude hook for zsh resolution",
    )
    .unwrap();
    assert!(install.status.success(), "{install:?}");
    let settings: Value =
        serde_json::from_slice(&fs::read(s.home.join(".claude/settings.json")).unwrap()).unwrap();
    let installed = settings["hooks"]["PreToolUse"][0]["hooks"][0]["command"]
        .as_str()
        .unwrap();
    let mut event: Value = serde_json::from_str(include_str!(
        "../../../../../arc-cli/tests/fixtures/agent_hooks/claude-recorded-2.1.327-cargo-build.json"
    ))
    .unwrap();
    event["cwd"] = json!(s.repo);
    event["tool_input"]["command"] = json!("gh workflow run build.yml");
    let response = output_with_input(
        &mut shell_command_for("/bin/zsh", &s, &f, installed),
        &serde_json::to_vec(&event).unwrap(),
        "installed hook for zsh resolution",
    );
    assert!(response.status.success(), "{response:?}");
    let response: Value = serde_json::from_slice(&response.stdout).unwrap();
    assert!(response["hookSpecificOutput"]
        .get("permissionDecision")
        .is_none());
    let rewritten = response["hookSpecificOutput"]["updatedInput"]["command"]
        .as_str()
        .unwrap();
    let status_variable = hook_status_variable(rewritten);

    let alternative = s.home.join("other-gh");
    write(
        &alternative,
        "#!/bin/sh\nprintf 'hashed:%s:%s:%s\\n' \"$1\" \"$2\" \"$3\"\nIFS= read -r line\nprintf 'stdin:%s\\n' \"$line\"\nprintf 'hashed-stderr\\n' >&2\nexit 39\n",
    );
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&alternative, fs::Permissions::from_mode(0o755)).unwrap();
    for (name, startup, expected_code) in [
        (
            "function",
            "gh() { printf 'function:%s:%s:%s\\n' \"$1\" \"$2\" \"$3\"; IFS= read -r line; printf 'stdin:%s\\n' \"$line\"; printf 'function-stderr\\n' >&2; return 37; }".to_owned(),
            37,
        ),
        (
            "function-prior-status",
            "gh() { printf 'prior-status:%s underscore:%s\\n' \"$?\" \"$_\"; return 37; }\nfalse".to_owned(),
            37,
        ),
        (
            "alias-shell-syntax",
            "alias gh='printf alias-output; #'".to_owned(),
            0,
        ),
        (
            "hash",
            format!("hash gh='{}'", alternative.display()),
            39,
        ),
        (
            "existing-temporary-name",
            format!(
                "export {status_variable}=caller-value\ngh() {{ printf 'value:%s status:%s\\n' \"${status_variable}\" \"$?\"; return 40; }}\nfalse"
            ),
            40,
        ),
        (
            "builtin-function",
            "builtin() { printf 'unexpected-builtin\\n' >&2; return 42; }\ngh() { printf 'gh-function:%s\\n' \"$?\"; return 37; }\nfalse".to_owned(),
            37,
        ),
        (
            "builtin-and-eval-functions",
            "builtin() { print -u2 unexpected-builtin; return 42; }\neval() { print -u2 unexpected-eval; return 43; }\ngh() { print -r -- gh-function:$?:$1:$2:$3; IFS= read -r line; print -r -- stdin:$line; print -u2 gh-stderr; return 37; }\nfalse".to_owned(),
            37,
        ),
        (
            "builtin-and-unset-functions",
            "builtin() { print -u2 unexpected-builtin; return 42; }\nunset() { print -u2 unexpected-unset; return 44; }\ngh() { print -r -- gh-function:$?:$1:$2:$3; IFS= read -r line; print -r -- stdin:$line; print -u2 gh-stderr; return 37; }\nfalse".to_owned(),
            37,
        ),
        (
            "builtin-eval-and-shell-syntax-alias",
            "builtin() { print -u2 unexpected-builtin; return 42; }\neval() { print -u2 unexpected-eval; return 43; }\nalias gh='print alias-output; #'".to_owned(),
            0,
        ),
        (
            "command-function",
            "command() { print -u2 unexpected-command; return 45; }\ngh() { print -r -- gh-function:$?:$1:$2:$3; return 37; }\nfalse".to_owned(),
            37,
        ),
        (
            "hashed-command-and-shadowed-helpers",
            format!("builtin() {{ print -u2 unexpected-builtin; return 42; }}\neval() {{ print -u2 unexpected-eval; return 43; }}\nhash gh='{}'", alternative.display()),
            39,
        ),
        (
            "hashed-command-and-shadowed-exit",
            format!("exit() {{ print -u2 unexpected-exit; return 46; }}\nbuiltin() {{ print -u2 unexpected-builtin; return 42; }}\neval() {{ print -u2 unexpected-eval; return 43; }}\nhash gh='{}'", alternative.display()),
            39,
        ),
        (
            "hashed-command-and-existing-temporary-name",
            format!("export {status_variable}=caller-value\neval() {{ print -u2 unexpected-eval; return 43; }}\nhash gh='{}'", alternative.display()),
            39,
        ),
        (
            "existing-temporary-name-and-shadowed-eval",
            format!(
                "export {status_variable}=caller-value\nbuiltin() {{ print -u2 unexpected-builtin; return 42; }}\neval() {{ print -u2 unexpected-eval; return 43; }}\ngh() {{ print -r -- \"value:${{{status_variable}}} status:$?\"; return 40; }}\nfalse"
            ),
            40,
        ),
    ] {
        // Zsh expands aliases while parsing -c, so define these in its
        // isolated startup file rather than earlier in the same -c string.
        write(&s.home.join(".zshenv"), &startup);
        let run = |source: &str| {
            output_with_input(
                &mut shell_command_for("/bin/zsh", &s, &f, source),
                b"input\n",
                name,
            )
        };
        let direct = run("gh workflow run build.yml");
        let wrapped = run(rewritten);
        assert_eq!(direct.status.code(), Some(expected_code), "{name}");
        assert_eq!(wrapped.status.code(), direct.status.code(), "{name}: {wrapped:?}");
        assert_eq!(wrapped.stdout, direct.stdout, "{name}");
        assert_eq!(wrapped.stderr, direct.stderr, "{name}");
        assert_eq!(s.calls(), 0, "{name}: PATH stub must not be invoked");
    }
}

#[test]
fn managed_sh_enforces_full_argv_byte_depth_token_and_work_budgets() {
    // The finite detector contract fixes these limits; keep this test in the
    // daemon crate without adding a shared-manifest dependency in this wave.
    const INPUT_LIMIT: usize = 16_384;
    const DEPTH_LIMIT: usize = 8;
    const TOKEN_LIMIT: usize = 1_024;
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    symlink("/bin/sh", s.bin.join("sh")).unwrap();
    symlink(cli::binary("arc-exec-fixture"), s.bin.join("cargo")).unwrap();
    let key = f.root.join("budget-capture-key");
    write(&key, b"arc-hook-fixture-capture-v1");
    let marker = f.root.join("budget-calls");
    let observation = f.root.join("budget-observation.json");
    let mut daemon = f.spawn_isolated("test");
    let cases = [
        (
            "bytes",
            format!("{}cargo test", " ".repeat(INPUT_LIMIT - 10)),
            "INPUT_LIMIT",
        ),
        (
            "depth",
            format!(
                "{}cargo test{}",
                "( ".repeat(DEPTH_LIMIT),
                " )".repeat(DEPTH_LIMIT)
            ),
            "NESTING_LIMIT",
        ),
        (
            "tokens",
            format!("cargo test{}", " x".repeat(TOKEN_LIMIT - 2)),
            "TOKEN_LIMIT",
        ),
        (
            "work",
            format!(
                "cargo test {}",
                "x".repeat(INPUT_LIMIT - 4 - "cargo test ".len())
            ),
            "WORK_LIMIT",
        ),
    ];
    for (index, (name, source, reason)) in cases.into_iter().enumerate() {
        let mut command = s.command(&f, &[]);
        command
            .env("ARC_FIXTURE_MODE", "hook-capture-pid")
            .env("ARC_HOOK_CAPTURE_KEY", &key)
            .env("ARC_HOOK_CAPTURE_MARKER", &marker)
            .env("ARC_HOOK_CAPTURE_RECORD", &observation)
            .args(["exec", "--", "sh", "-c", &source]);
        let output = output_with_input(&mut command, b"input\n", name);
        assert_eq!(output.status.code(), Some(17), "{name}: {output:?}");
        assert_eq!(output.stdout, b"hook-stdout\0payload\n", "{name}");
        assert_eq!(output.stderr, b"child-stderr\n", "{name}");
        let recorded = job(&f);
        assert_eq!(
            recorded["classification"]["classification"], "UNKNOWN",
            "{name}: {recorded}"
        );
        assert_eq!(
            recorded["classification"]["uncertainty"], "OBSERVE",
            "{name}: {recorded}"
        );
        assert_eq!(
            recorded["classification"]["reason"], reason,
            "{name}: {recorded}"
        );
        assert_eq!(
            fs::read_to_string(&marker).unwrap().lines().count(),
            index + 1,
            "{name}"
        );
    }
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn installed_hook_preserves_zsh_function_with_inherited_table_names() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    let install = child_watchdog::output(
        s.command(&f, &[])
            .args(["hooks", "install", "claude", "--scope", "user"]),
        "install Claude hook for inherited zsh parameters",
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
        "installed hook for inherited zsh parameters",
    );
    assert!(response.status.success(), "{response:?}");
    assert!(response.stderr.is_empty());
    let response: Value = serde_json::from_slice(&response.stdout).unwrap();
    assert!(response["hookSpecificOutput"]
        .get("permissionDecision")
        .is_none());
    let rewritten = response["hookSpecificOutput"]["updatedInput"]["command"]
        .as_str()
        .unwrap();
    let function = "gh() { print -r -- \"function:$?:$1:$2:$3:$PWD:$PRIVATE_ENV\"; IFS= read -r line; print -r -- \"stdin:$line\"; print -u2 -- function-stderr; return 37; }\n";
    write(&s.home.join(".zshenv"), format!("{function}false\n"));

    // Exported scalars can replace zsh's otherwise-special parameter tables.
    // Neither the hook nor the fallback may depend on those tables existing.
    for names in [
        &["functions"][..],
        &["options"],
        &["aliases"],
        &["galiases"],
        &["functions", "options", "aliases", "galiases"],
    ] {
        let run = |source: &str| {
            let mut command = shell_command_for("/bin/zsh", &s, &f, source);
            for name in names {
                command.env(name, "abc");
            }
            output_with_input(&mut command, b"input\n", "inherited zsh parameters")
        };
        let direct = run("gh workflow run build.yml");
        let wrapped = run(rewritten);
        assert_eq!(direct.status.code(), Some(37), "{names:?}: {direct:?}");
        assert_eq!(
            wrapped.status.code(),
            direct.status.code(),
            "{names:?}: {wrapped:?}"
        );
        assert_eq!(wrapped.stdout, direct.stdout, "{names:?}");
        assert_eq!(wrapped.stderr, direct.stderr, "{names:?}");
        assert_eq!(direct.stderr, b"function-stderr\n");
        assert!(
            String::from_utf8_lossy(&direct.stdout).contains("function:1:workflow:run:build.yml:")
        );
        assert!(String::from_utf8_lossy(&direct.stdout).contains("stdin:input\n"));
        assert_eq!(s.calls(), 0, "{names:?}: PATH stub must not be invoked");
    }

    // With NO_UNSET, even asking for an absent parameter's type raises an
    // error. A startup file may unset one or more special parameters.
    for names in [
        &["functions"][..],
        &["options"],
        &["aliases"],
        &["galiases"],
        &["functions", "options", "aliases", "galiases"],
    ] {
        write(
            &s.home.join(".zshenv"),
            format!(
                "{function}unset {}\nsetopt NO_UNSET\nfalse\n",
                names.join(" ")
            ),
        );
        let run = |source: &str| {
            output_with_input(
                &mut shell_command_for("/bin/zsh", &s, &f, source),
                b"input\n",
                "unset zsh parameters",
            )
        };
        let direct = run("gh workflow run build.yml");
        let wrapped = run(rewritten);
        assert_eq!(direct.status.code(), Some(37), "{names:?}: {direct:?}");
        assert_eq!(
            wrapped.status.code(),
            direct.status.code(),
            "{names:?}: {wrapped:?}"
        );
        assert_eq!(wrapped.stdout, direct.stdout, "{names:?}");
        assert_eq!(wrapped.stderr, direct.stderr, "{names:?}");
        assert_eq!(s.calls(), 0, "{names:?}: PATH stub must not be invoked");
    }

    // A shell-syntax alias must still be expanded by zsh in the fallback.
    write(
        &s.home.join(".zshenv"),
        "alias gh='print -r -- alias-output; #'\n",
    );
    let run_alias = |source: &str| {
        let mut command = shell_command_for("/bin/zsh", &s, &f, source);
        for name in ["functions", "options", "aliases", "galiases"] {
            command.env(name, "abc");
        }
        output_with_input(&mut command, b"", "inherited zsh parameters and gh alias")
    };
    let direct = run_alias("gh workflow run build.yml");
    let wrapped = run_alias(rewritten);
    assert!(direct.status.success(), "{direct:?}");
    assert_eq!(direct.stdout, b"alias-output\n");
    assert_eq!(wrapped.status, direct.status, "{wrapped:?}");
    assert_eq!(wrapped.stdout, direct.stdout);
    assert_eq!(wrapped.stderr, direct.stderr);
    assert_eq!(s.calls(), 0, "alias must bypass the PATH stub");
}

#[test]
fn installed_hook_preserves_zsh_xtrace_streams_and_shell_resolution() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    write(
        &s.repo.join(".git/config"),
        "[remote \"origin\"]\n url = https://github.com/example/app.git\n",
    );
    let mut daemon = f.spawn_isolated("test");
    let install = child_watchdog::output(
        s.command(&f, &[])
            .args(["hooks", "install", "claude", "--scope", "user"]),
        "install traced hook",
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
        "traced hook event",
    );
    assert!(response.status.success(), "{response:?}");
    let response: Value = serde_json::from_slice(&response.stdout).unwrap();
    let rewritten = response["hookSpecificOutput"]["updatedInput"]["command"]
        .as_str()
        .unwrap();
    for startup in [
        "set -x\n",
        "gh() { print -r -- \"function:$?:$1:$2:$3\"; return 37; }\nset -x\nfalse\n",
        "alias gh='print -r -- alias-output; #'\nset -x\n",
    ] {
        write(&s.home.join(".zshenv"), startup);
        let run = |source: &str| {
            output_with_input(
                &mut shell_command_for("/bin/zsh", &s, &f, source),
                b"input\n",
                "traced shell",
            )
        };
        let direct = run("gh workflow run build.yml");
        let wrapped = run(rewritten);
        assert_eq!(wrapped.status, direct.status, "{startup}");
        assert_eq!(wrapped.stdout, direct.stdout, "{startup}");
        assert_eq!(
            wrapped.stderr,
            direct.stderr,
            "{startup}:\nwrapped {}\ndirect {}",
            String::from_utf8_lossy(&wrapped.stderr),
            String::from_utf8_lossy(&direct.stderr)
        );
    }
    use std::os::unix::process::CommandExt;
    write(
        &s.home.join(".zshenv"),
        "gh() { print -r -- \"function:$?:$1:$2:$3\"; return 37; }\nset -x\nfalse\n",
    );
    let run_closed = |source: &str| {
        let mut command = shell_command_for("/bin/zsh", &s, &f, source);
        unsafe {
            command.pre_exec(|| {
                libc::close(libc::STDERR_FILENO);
                Ok(())
            });
        }
        output_with_input(&mut command, b"", "traced shell with closed stderr")
    };
    let direct = run_closed("gh workflow run build.yml");
    let wrapped = run_closed(rewritten);
    assert_eq!(wrapped.status, direct.status);
    assert_eq!(wrapped.stdout, direct.stdout);
    assert_eq!(wrapped.stderr, direct.stderr);
    daemon.stop(libc::SIGTERM);
    f.report();
}

fn installed_gh_rewrite_for_trace(s: &Sandbox, f: &Fixture) -> String {
    let install = child_watchdog::output(
        s.command(f, &[])
            .args(["hooks", "install", "claude", "--scope", "user"]),
        "install hook for trace regression",
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
        &mut shell_command(s, f, installed),
        &serde_json::to_vec(&event).unwrap(),
        "trace regression hook event",
    );
    assert!(response.status.success(), "{response:?}");
    let response: Value = serde_json::from_slice(&response.stdout).unwrap();
    assert!(response["hookSpecificOutput"]
        .get("permissionDecision")
        .is_none());
    response["hookSpecificOutput"]["updatedInput"]["command"]
        .as_str()
        .unwrap()
        .to_owned()
}

#[test]
fn installed_hook_preserves_bash_xtrace_streams_and_shell_resolution() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    let rewritten = installed_gh_rewrite_for_trace(&s, &f);
    let startup = s.home.join("bash-trace-startup");
    let function = "gh() { printf 'function:%s:%s:%s:%s:%s\\n' \"$?\" \"$_\" \"$1\" \"$2\" \"$3\"; printf 'function-stderr\\n' >&2; return 17; }\n";
    for (name, contents, exit) in [
        ("function", format!("{function}set -x\nfalse\n"), 17),
        (
            "multiline-underscore",
            format!("{function}set -x\n: $'line one\\n: > forbidden'\n"),
            17,
        ),
        ("errexit", format!("{function}set -ex\n! true\n"), 17),
        (
            "trace-fd",
            format!("{function}exec 3>&2\nBASH_XTRACEFD=3\nset -x\nfalse\n"),
            17,
        ),
        (
            "alias",
            "shopt -s expand_aliases\nalias gh='printf alias-output; #'\nset -x\nfalse\n"
                .to_owned(),
            0,
        ),
    ] {
        write(&startup, contents);
        let run = |source: &str| {
            let mut shell = shell_command_for("/bin/bash", &s, &f, source);
            shell.env("BASH_ENV", &startup);
            output_with_input(&mut shell, b"input\n", "bash xtrace")
        };
        let direct = run("gh workflow run build.yml");
        let wrapped = run(&rewritten);
        assert_eq!(direct.status.code(), Some(exit), "{name}");
        assert_eq!(wrapped.status, direct.status, "{name}");
        assert_eq!(wrapped.stdout, direct.stdout, "{name}");
        assert_eq!(wrapped.stderr, direct.stderr, "{name}");
        assert_eq!(s.calls(), 0, "{name}");
        assert!(!s.repo.join("forbidden").exists(), "{name}");
    }
}

#[test]
fn installed_hook_preserves_zsh_xtrace_with_unset_options_table() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    let rewritten = installed_gh_rewrite_for_trace(&s, &f);
    for (name, startup, exit) in [
        ("function", "gh() { print -r -- \"function:$?:$1:$2:$3\"; print -u2 -- function-stderr; return 17; }\nunset options\nset -x\nfalse\n", 17),
        ("alias", "alias gh='print -r -- alias-output; #'\nunset options\nset -x\nfalse\n", 0),
    ] {
        write(&s.home.join(".zshenv"), startup);
        let run = |source: &str| {
            output_with_input(
                &mut shell_command_for("/bin/zsh", &s, &f, source),
                b"input\n",
                "zsh xtrace without options table",
            )
        };
        let direct = run("gh workflow run build.yml");
        let wrapped = run(&rewritten);
        assert_eq!(direct.status.code(), Some(exit), "{name}");
        assert_eq!(wrapped.status, direct.status, "{name}");
        assert_eq!(wrapped.stdout, direct.stdout, "{name}");
        assert_eq!(wrapped.stderr, direct.stderr, "{name}");
        assert_eq!(s.calls(), 0, "{name}");
    }
}

use super::*;

const ORIGINAL: &str = "gh workflow run build.yml";

#[test]
fn installed_hook_preserves_later_zsh_line_number_in_default_shell() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    let rewritten = installed_gh_rewrite_for_trace(&s, &f);
    write(
        &s.home.join(".zshenv"),
        "gh() { print -r -- function-output; return 17; }\n",
    );
    let run = |source: &str| {
        let continued = format!(
            "{source}; _arc_test_status=$?; print -r -- \"line:$LINENO status:$_arc_test_status\""
        );
        output_with_input(
            &mut shell_command_for("/bin/zsh", &s, &f, &continued),
            b"",
            "continued zsh shell line number",
        )
    };
    let direct = run(ORIGINAL);
    let wrapped = run(&rewritten);
    assert_eq!(direct.stdout, b"function-output\nline:1 status:17\n");
    assert_eq!(wrapped.status, direct.status);
    assert_eq!(wrapped.stdout, direct.stdout);
    assert_eq!(wrapped.stderr, direct.stderr);
    assert_eq!(s.calls(), 0);
}

fn assert_same(direct: &std::process::Output, rewritten: &std::process::Output, label: &str) {
    assert_eq!(direct.status.code(), Some(17), "{label}: {direct:?}");
    assert_eq!(rewritten.status, direct.status, "{label}: {rewritten:?}");
    assert_eq!(rewritten.stdout, direct.stdout, "{label}");
    assert_eq!(rewritten.stderr, direct.stderr, "{label}");
}

fn environment(observation: &Value) -> std::collections::BTreeMap<String, String> {
    observation["environment"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            let value = |part: &Value| {
                String::from_utf8(serde_json::from_value::<Vec<u8>>(part.clone()).unwrap()).unwrap()
            };
            (value(&entry[0]), value(&entry[1]))
        })
        .collect()
}

#[test]
fn installed_hook_preserves_zsh_function_with_unset_underscore_and_nounset() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    let rewritten = installed_gh_rewrite_for_trace(&s, &f);
    let prefix = "gh() { print -rl -- \"$?\" \"${_+x}\" \"${_-}\" \"$1\" \"$2\" \"$3\" \"$PWD\" \"$PRIVATE_ENV\"; print -u2 -- function-stderr; return 17; }; unset _; set -u; ";
    for before in ["", "false; ", "set -x; "] {
        let run = |source: &str| {
            output_with_input(
                &mut shell_command_for("/bin/zsh", &s, &f, &format!("{prefix}{before}{source}")),
                b"input\n",
                "unset underscore with nounset",
            )
        };
        assert_same(&run(ORIGINAL), &run(&rewritten), before);
    }
    assert_eq!(s.calls(), 0);
}

#[test]
fn installed_hook_preserves_default_shell_state_after_managed_execution() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    write(
        &s.repo.join(".git/config"),
        "[remote \"origin\"]\n url = https://github.com/example/app.git\n",
    );
    let mut daemon = f.spawn_isolated("test");
    let rewritten = installed_gh_rewrite_for_trace(&s, &f);
    for shell in ["/bin/sh", "/bin/bash", "/bin/zsh"] {
        let run = |source: &str| {
            let continued = format!(
                "{source}; printf 'after:%s:%s:%s\\n' \"$?\" \"$_\" \"$LINENO\"; command -v gh"
            );
            output_with_input(
                &mut shell_command_for(shell, &s, &f, &continued),
                b"input\n",
                "continued shell after managed gh",
            )
        };
        let direct = run(ORIGINAL);
        let wrapped = run(&rewritten);
        assert_eq!(wrapped.status, direct.status, "{shell}");
        assert_eq!(wrapped.stdout, direct.stdout, "{shell}");
        assert_eq!(wrapped.stderr, direct.stderr, "{shell}");
        assert_eq!(job(&f)["classification"]["classification"], "REMOTE_CI");
    }
    assert_eq!(s.calls(), 6);
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn installed_hook_preserves_zsh_environment_when_command_ends_with_semicolon() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    write(
        &s.repo.join(".git/config"),
        "[remote \"origin\"]\n url = https://github.com/example/app.git\n",
    );
    let mut daemon = f.spawn_isolated("test");
    let rewritten = installed_gh_rewrite_for_trace(&s, &f);
    let key = f.root.join("continuation-key");
    let marker = f.root.join("continuation-calls");
    let record = f.root.join("continuation-observation.json");
    write(&key, b"arc-hook-fixture-capture-v1");
    let run = |source: &str, suffix: &str| {
        let mut shell = shell_command_for("/bin/zsh", &s, &f, &format!("{source}{suffix}"));
        shell
            .env("ARC_FIXTURE_MODE", "hook-capture-pid")
            .env("ARC_HOOK_CAPTURE_KEY", &key)
            .env("ARC_HOOK_CAPTURE_MARKER", &marker)
            .env("ARC_HOOK_CAPTURE_RECORD", &record);
        let output = output_with_input(&mut shell, b"input\n", "zsh semicolon");
        let observation: Value = serde_json::from_slice(&fs::read(&record).unwrap()).unwrap();
        (output, environment(&observation))
    };
    for suffix in [";", ";  ", "  "] {
        let (direct, mut direct_environment) = run(ORIGINAL, suffix);
        let (wrapped, mut wrapped_environment) = run(&rewritten, suffix);
        instrumentation::include_expected(&mut direct_environment);
        instrumentation::normalize_actual(&mut wrapped_environment, &direct_environment);
        instrumentation::normalize_actual(&mut direct_environment, &wrapped_environment);
        assert_eq!(wrapped.status, direct.status, "{suffix:?}");
        assert_eq!(wrapped.stdout, direct.stdout, "{suffix:?}");
        assert_eq!(wrapped.stderr, direct.stderr, "{suffix:?}");
        assert_eq!(wrapped_environment, direct_environment, "{suffix:?}");
        assert_eq!(job(&f)["classification"]["classification"], "REMOTE_CI");
    }
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn installed_hook_passes_through_zsh_script_context_when_exec_position_is_unknown() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    let mut daemon = f.spawn_isolated("test");
    let rewritten = installed_gh_rewrite_for_trace(&s, &f);
    let script = s.home.join("shell-source.zsh");
    let key = f.root.join("script-key");
    let marker = f.root.join("script-calls");
    let record = f.root.join("script-observation.json");
    write(&key, b"arc-hook-fixture-capture-v1");
    let run = |source: &str| {
        write(&script, format!("{source}\n"));
        let template = s.command(&f, &[]);
        let mut shell = Command::new("/bin/zsh");
        shell
            .env_clear()
            .env("ZDOTDIR", &s.home)
            .env("LOGNAME", "arc-fixture")
            .env("USER", "arc-fixture")
            .arg(&script)
            .current_dir(&s.repo);
        for (key, value) in template.get_envs() {
            if let Some(value) = value {
                shell.env(key, value);
            }
        }
        shell
            .env("ARC_FIXTURE_MODE", "hook-capture-pid")
            .env("ARC_HOOK_CAPTURE_KEY", &key)
            .env("ARC_HOOK_CAPTURE_MARKER", &marker)
            .env("ARC_HOOK_CAPTURE_RECORD", &record);
        let output = output_with_input(&mut shell, b"input\n", "zsh script");
        let observation: Value = serde_json::from_slice(&fs::read(&record).unwrap()).unwrap();
        (output, environment(&observation))
    };
    let (direct, mut direct_environment) = run(ORIGINAL);
    let (wrapped, mut wrapped_environment) = run(&rewritten);
    instrumentation::include_expected(&mut direct_environment);
    instrumentation::normalize_actual(&mut wrapped_environment, &direct_environment);
    instrumentation::normalize_actual(&mut direct_environment, &wrapped_environment);
    assert_eq!(wrapped.status, direct.status);
    assert_eq!(wrapped.stdout, direct.stdout);
    assert_eq!(wrapped.stderr, direct.stderr);
    assert_eq!(wrapped_environment, direct_environment);
    assert_eq!(fs::read_to_string(&marker).unwrap().lines().count(), 2);
    let db = rusqlite::Connection::open(&f.database).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM jobs", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        0
    );
    drop(db);
    daemon.stop(libc::SIGTERM);
    f.report();
}

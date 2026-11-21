use super::*;

#[test]
fn nested_bash_startup_cannot_borrow_captured_endpoint_facts_on_either_route() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    write(
        &s.repo.join(".git/config"),
        "[remote \"origin\"]\n url = https://github.com/example/app.git\n",
    );
    symlink("/bin/bash", s.bin.join("bash")).unwrap();
    let startup = s.home.join("startup.sh");
    let startup_contents = "export PATH=\"$ARC_FIXTURE_BIN\"\nexport GH_HOST=localhost\nprintf '%s\\n' \"$GH_HOST\" >> \"$ARC_STARTUP_MARKER\"\n";
    write(&startup, startup_contents);
    write(&s.home.join(".bash_profile"), startup_contents);
    let mut daemon = f.spawn_isolated("test");
    for option in ["-c", "-lc"] {
        let marker = f.root.join(format!("startup-{}", &option[1..]));
        let marker_str = marker.to_str().unwrap();
        let bin_str = s.bin.to_str().unwrap();
        let startup_str = startup.to_str().unwrap();
        let env = [
            ("ARC_FIXTURE_BIN", Some(bin_str)),
            ("ARC_STARTUP_MARKER", Some(marker_str)),
            ("BASH_ENV", (option == "-c").then_some(startup_str)),
        ];
        let argv = ["bash", option, "gh workflow run build.yml"];
        let inspection = s.classify(&f, &argv, &env);
        assert_eq!(inspection["classification"], "UNKNOWN");
        assert_eq!(inspection["locality"], "UNKNOWN");
        assert_eq!(inspection["uncertainty"], "OBSERVE");
        assert!(!marker.exists(), "classification ran shell startup");

        let before = s.calls();
        let mut command = s.command(&f, &env);
        command
            .args(["exec", "--"])
            .args(argv)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = child_watchdog::Owned::spawn(&mut command).unwrap();
        child
            .child
            .stdin
            .take()
            .unwrap()
            .write_all(b"input\n")
            .unwrap();
        assert_eq!(
            child.wait("nested bash execution").unwrap().code(),
            Some(17)
        );
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
        assert_eq!(stderr, b"fixture-stderr\n");
        let output: Value =
            serde_json::from_slice(stdout.strip_prefix(b"input\n").unwrap()).unwrap();
        assert_eq!(output["args"], json!([b"workflow", b"run", b"build.yml"]));
        assert_eq!(
            output["cwd"],
            json!(fs::canonicalize(&s.repo).unwrap().as_os_str().as_bytes())
        );
        assert_eq!(output["environment"], json!(b"offline-fixture-value"));
        assert_eq!(s.calls(), before + 1);
        assert_eq!(admission_counts(&f), (0, 0));
        assert_eq!(fs::read_to_string(&marker).unwrap(), "localhost\n");
        let id = cli::recent(&f);
        let response = protocol::call(
            &f,
            OPERATION_JOB_STATUS_LOCALITY,
            json!(StatusRequest::ById { job_id: id }),
        );
        let job = response.result.unwrap()["job"].clone();
        assert_eq!(job["classification"]["classification"], "UNKNOWN");
        assert_eq!(job["classification"]["locality"], "UNKNOWN");
        assert_eq!(job["classification"]["uncertainty"], "OBSERVE");
        assert!(job["child"]["pid"].is_u64());
        assert_eq!(job["outcome"]["code"], 17);
    }
    daemon.stop(libc::SIGTERM);
    f.report();
}

fn shell_sandbox(f: &Fixture) -> Sandbox {
    use std::os::unix::fs::PermissionsExt;
    let s = Sandbox::new(f);
    symlink("/bin/sh", s.bin.join("sh")).unwrap();
    symlink("/bin/bash", s.bin.join("bash")).unwrap();
    symlink("/bin/zsh", s.bin.join("zsh")).unwrap();
    for name in ["cargo", "bun", "make", "just", "task", "pnpm", "npm"] {
        symlink(cli::binary("arc-exec-fixture"), s.bin.join(name)).unwrap();
    }
    for name in ["env", "xargs"] {
        symlink(format!("/usr/bin/{name}"), s.bin.join(name)).unwrap();
    }
    for (name, script) in [
        ("timeout", "#!/bin/sh\nshift\nexec \"$@\"\n"),
        ("nice", "#!/bin/sh\nexec \"$@\"\n"),
        ("sudo", "#!/bin/sh\nexec \"$@\"\n"),
    ] {
        let path = s.bin.join(name);
        write(&path, script);
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    s
}

fn inspect_shell(
    s: &Sandbox,
    f: &Fixture,
    source: &str,
    env: &[(&str, Option<&str>)],
) -> Vec<Value> {
    let before = s.calls();
    let mut results = Vec::new();
    for dialect in ["bash", "zsh"] {
        let mut command = s.command(f, env);
        command.args(["classify", "--json", "--shell", dialect, "--", source]);
        let output = child_watchdog::output(&mut command, "shell inspection").unwrap();
        assert!(output.status.success(), "{dialect}: {source}: {output:?}");
        assert!(output.stderr.is_empty(), "{dialect}: {source}: {output:?}");
        assert_eq!(s.calls(), before, "inspection executed a fixture: {source}");
        results.push(serde_json::from_slice(&output.stdout).unwrap());
    }
    results
}

fn execute_shell(
    s: &Sandbox,
    f: &Fixture,
    source: &str,
    env: &[(&str, Option<&str>)],
    calls: usize,
) -> (Value, std::process::Output) {
    execute_dialect(s, f, "sh", source, env, calls)
}

fn execute_dialect(
    s: &Sandbox,
    f: &Fixture,
    dialect: &str,
    source: &str,
    env: &[(&str, Option<&str>)],
    calls: usize,
) -> (Value, std::process::Output) {
    let before = s.calls();
    let mut command = s.command(f, env);
    command
        .env("ARC_FIXTURE_MODE", "success")
        .args(["exec", "--", dialect, "-c", source]);
    let output = child_watchdog::output(&mut command, "shell execution").unwrap();
    assert!(output.status.success(), "{source}: {output:?}");
    assert_eq!(s.calls(), before + calls, "{source}: {output:?}");
    let id = cli::recent(f);
    let response = protocol::call(
        f,
        OPERATION_JOB_STATUS_LOCALITY,
        json!(StatusRequest::ById { job_id: id }),
    );
    assert_eq!(response.status, "ok", "{response:?}");
    let job = response.result.unwrap()["job"].clone();
    assert!(job["child"]["pid"].is_u64(), "{source}: {job}");
    assert_eq!(job["outcome"]["code"], 0, "{source}: {job}");
    (job, output)
}

fn assert_shell_case(
    s: &Sandbox,
    f: &Fixture,
    source: &str,
    env: &[(&str, Option<&str>)],
    expected: &str,
    calls: usize,
) -> std::process::Output {
    let inspected = inspect_shell(s, f, source, env);
    let (locality, uncertainty) = if expected == "UNKNOWN" {
        ("UNKNOWN", "OBSERVE")
    } else {
        ("HOST_LOCAL", "NONE")
    };
    let mut output = None;
    for (dialect, inspection) in ["bash", "zsh"].into_iter().zip(inspected) {
        let (job, result_output) = execute_dialect(s, f, dialect, source, env, calls);
        for result in [&inspection, &job["classification"]] {
            assert_eq!(
                result["classification"], expected,
                "{dialect}: {source}: {result}"
            );
            assert_eq!(
                result["locality"], locality,
                "{dialect}: {source}: {result}"
            );
            assert_eq!(
                result["uncertainty"], uncertainty,
                "{dialect}: {source}: {result}"
            );
        }
        output = Some(result_output);
    }
    output.unwrap()
}

#[test]
fn direct_commands_are_classified_on_both_routes() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = shell_sandbox(&f);
    let mut daemon = f.spawn_isolated("test");
    for (source, expected) in [
        ("cargo build", "BUILD"),
        ("bun test", "TEST"),
        ("cargo test 'two words'", "TEST"),
        ("MODE=test cargo test", "TEST"),
    ] {
        let output = assert_shell_case(&s, &f, source, &[], expected, 1);
        let child: Value = serde_json::from_slice(&output.stdout).unwrap();
        if source.contains("two words") {
            assert_eq!(child["args"], json!([b"test".as_slice(), b"two words"]));
        }
    }
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn test_pipelines_and_compounds_are_classified_on_both_routes() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = shell_sandbox(&f);
    let mut daemon = f.spawn_isolated("test");
    for (source, calls) in [
        ("cargo test | cargo test", 2),
        ("cargo test && cargo test", 2),
        ("cargo test || cargo test", 1),
        ("cargo test; cargo test", 2),
        ("(cargo test)", 1),
    ] {
        assert_shell_case(&s, &f, source, &[], "TEST", calls);
    }
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn command_substitution_is_observed_without_evaluation() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = shell_sandbox(&f);
    let mut daemon = f.spawn_isolated("test");
    assert_shell_case(&s, &f, "cargo test $(printf x)", &[], "UNKNOWN", 1);
    let marker = s.repo.join("substitution-marker");
    let source = "cargo test $(/usr/bin/touch substitution-marker)";
    let inspection = inspect_shell(&s, &f, source, &[]);
    assert!(inspection
        .iter()
        .all(|value| value["classification"] == "UNKNOWN"));
    assert!(
        !marker.exists(),
        "inspection evaluated command substitution"
    );
    let (job, _) = execute_shell(&s, &f, source, &[], 1);
    assert_eq!(job["classification"]["uncertainty"], "OBSERVE");
    assert!(
        marker.exists(),
        "the real shell did not evaluate its substitution"
    );
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn literal_paths_and_directory_changes_are_classified_on_both_routes() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = shell_sandbox(&f);
    fs::create_dir_all(s.repo.join("검증")).unwrap();
    fs::create_dir_all(s.repo.join("fixtures")).unwrap();
    for path in [s.repo.join("cargo"), s.repo.join("검증/cargo")] {
        symlink(cli::binary("arc-exec-fixture"), path).unwrap();
    }
    let mut daemon = f.spawn_isolated("test");
    let absolute = format!("{}/cargo build", s.bin.display());
    for source in [
        "./cargo build",
        absolute.as_str(),
        "./검증/cargo build",
        "cd fixtures && cargo build",
    ] {
        let output = assert_shell_case(&s, &f, source, &[], "BUILD", 1);
        if source.starts_with("cd ") {
            let child: Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(
                child["cwd"],
                json!(fs::canonicalize(s.repo.join("fixtures"))
                    .unwrap()
                    .as_os_str()
                    .as_bytes())
            );
        }
    }
    assert_shell_case(
        &s,
        &f,
        "cd \"$TARGET\" && cargo build",
        &[("TARGET", Some("fixtures"))],
        "UNKNOWN",
        1,
    );
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn literal_and_unproved_wrappers_are_classified_on_both_routes() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = shell_sandbox(&f);
    let mut daemon = f.spawn_isolated("test");
    for (source, expected) in [
        ("env cargo test", "TEST"),
        ("timeout 10 cargo build", "BUILD"),
        ("nice cargo test", "TEST"),
        ("sudo cargo build", "UNKNOWN"),
        ("printf x | xargs cargo test", "UNKNOWN"),
    ] {
        assert_shell_case(&s, &f, source, &[], expected, 1);
    }
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn nested_sh_literal_is_local_test_on_both_routes() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = shell_sandbox(&f);
    let mut daemon = f.spawn_isolated("test");
    assert_shell_case(&s, &f, "sh -c 'cargo test'", &[], "TEST", 1);
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn dynamic_wrappers_and_targets_are_observed_on_both_routes() {
    use std::os::unix::fs::PermissionsExt;
    let _serial = support::serial();
    let f = Fixture::new();
    let s = shell_sandbox(&f);
    let script = s.repo.join("build.sh");
    write(&script, "#!/bin/sh\nexec \"$RUNNER\"\n");
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    write(
        &s.repo.join("package.json"),
        r#"{"scripts":{"deploy":"cargo build && $DEPLOY_STEP"}}"#,
    );
    write(&s.repo.join("Makefile"), "test:\n\t$(TEST_RUNNER) test\n");
    write(
        &s.repo.join("Justfile"),
        "test:\n  {{env_var(\"TEST_RUNNER\")}} test\n",
    );
    write(
        &s.repo.join("Taskfile.yml"),
        "version: '3'\ntasks:\n  test:\n    cmds:\n      - '{{.TEST_RUNNER}} test'\n",
    );
    let cargo = s.bin.join("cargo");
    let env = [
        ("RUNNER", Some(cargo.to_str().unwrap())),
        ("TEST_RUNNER", Some(cargo.to_str().unwrap())),
        ("DEPLOY_STEP", Some("gh workflow run ci.yml")),
    ];
    let mut daemon = f.spawn_isolated("test");
    for source in [
        "./build.sh",
        "pnpm run deploy",
        "make test",
        "just test",
        "task test",
    ] {
        assert_shell_case(&s, &f, source, &env, "UNKNOWN", 1);
    }
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn npm_verify_with_dynamic_lifecycle_is_observed_on_both_routes() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = shell_sandbox(&f);
    write(
        &s.repo.join("package.json"),
        r#"{"scripts":{"preverify":"$PREVERIFY","verify":"eslint && vitest"}}"#,
    );
    let cargo = s.bin.join("cargo");
    let env = [("PREVERIFY", Some(cargo.to_str().unwrap()))];
    let mut daemon = f.spawn_isolated("test");
    assert_shell_case(&s, &f, "npm run verify", &env, "UNKNOWN", 1);
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn bun_test_and_remote_ci_inspection_is_mixed_without_claiming_stage_control() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = shell_sandbox(&f);
    write(
        &s.repo.join(".git/config"),
        "[remote \"origin\"]\n url = https://github.com/example/app.git\n",
    );
    let mut daemon = f.spawn_isolated("test");
    let inspected = inspect_shell(&s, &f, "bun test && gh workflow run ci.yml", &[]);
    for value in inspected {
        assert_eq!(value["classification"], "MIXED", "{value}");
        assert_eq!(value["locality"], "MIXED", "{value}");
        assert_eq!(value["uncertainty"], "NONE", "{value}");
    }
    let (job, _) = execute_shell(&s, &f, "bun test && gh workflow run ci.yml", &[], 2);
    assert_eq!(job["classification"]["classification"], "MIXED");
    assert_eq!(job["classification"]["locality"], "MIXED");
    assert_eq!(job["classification"]["uncertainty"], "NONE");
    assert_eq!(s.calls(), 2);
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn make_test_and_remote_ci_are_mixed_on_both_routes() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = shell_sandbox(&f);
    write(
        &s.repo.join(".git/config"),
        "[remote \"origin\"]\n url = https://github.com/example/app.git\n",
    );
    write(&s.repo.join("Makefile"), "test:\n\tcargo test\n");
    fs::remove_file(s.bin.join("make")).unwrap();
    symlink("/usr/bin/make", s.bin.join("make")).unwrap();
    let mut daemon = f.spawn_isolated("test");
    let source = "make test && gh workflow run ci.yml";
    let inspected = inspect_shell(&s, &f, source, &[]);
    let (job, _) = execute_shell(&s, &f, source, &[], 2);
    eprintln!(
        "Make inspection: {inspected:?}; execution: {}",
        job["classification"]
    );
    for result in inspected.iter().chain([&job["classification"]]) {
        assert_eq!(result["classification"], "MIXED", "{result}");
        assert_eq!(result["locality"], "MIXED", "{result}");
        assert_eq!(result["uncertainty"], "NONE", "{result}");
    }
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[path = "closing_shell/actual.rs"]
mod actual;

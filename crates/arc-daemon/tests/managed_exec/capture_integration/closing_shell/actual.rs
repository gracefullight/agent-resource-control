use super::*;

#[test]
fn bash_and_zsh_execute_the_same_direct_pipeline_compound_and_mixed_commands() {
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
    for (source, class, locality, calls) in [
        ("cargo build", "BUILD", "HOST_LOCAL", 1),
        ("cargo test", "TEST", "HOST_LOCAL", 1),
        ("cargo test | cargo test", "TEST", "HOST_LOCAL", 2),
        ("cargo test && cargo test", "TEST", "HOST_LOCAL", 2),
        ("cargo test; cargo test", "TEST", "HOST_LOCAL", 2),
        ("sh -c 'cargo test'", "TEST", "HOST_LOCAL", 1),
        ("bun test && gh workflow run ci.yml", "MIXED", "MIXED", 2),
        ("make test && gh workflow run ci.yml", "MIXED", "MIXED", 2),
    ] {
        let inspections = inspect_shell(&s, &f, source, &[]);
        for (dialect, inspected) in ["bash", "zsh"].into_iter().zip(inspections) {
            let (job, _) = execute_dialect(&s, &f, dialect, source, &[], calls);
            // `arcctl exec -- bash -c` separates the two mixed compounds into
            // stages (closing_shell/stages.rs). Its own record stays the MIXED
            // request record; zsh keeps the single pass-through record.
            let job = if dialect == "bash" && class == "MIXED" {
                assert_eq!(job["classification"]["classification"], "REMOTE_CI");
                super::stages::request_record(&f, &job)
            } else {
                job
            };
            for result in [&inspected, &job["classification"]] {
                assert_eq!(
                    result["classification"], class,
                    "{dialect}: {source}: {result}"
                );
                assert_eq!(
                    result["locality"], locality,
                    "{dialect}: {source}: {result}"
                );
                assert_eq!(
                    result["uncertainty"], "NONE",
                    "{dialect}: {source}: {result}"
                );
            }
        }
    }
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn zsh_startup_that_changes_endpoints_is_observed_on_both_routes() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = shell_sandbox(&f);
    write(
        &s.repo.join(".git/config"),
        "[remote \"origin\"]\n url = https://github.com/example/app.git\n",
    );
    write(
        &s.home.join(".zshenv"),
        "export GH_HOST=localhost\nprintf '%s\\n' \"$GH_HOST\" > \"$HOME/zsh-startup-marker\"\n",
    );
    let mut daemon = f.spawn_isolated("test");
    let source = "gh workflow run build.yml";
    let inspected = inspect_shell(&s, &f, source, &[]);
    assert_eq!(inspected[1]["classification"], "UNKNOWN");
    assert_eq!(inspected[1]["uncertainty"], "OBSERVE");
    assert!(!s.home.join("zsh-startup-marker").exists());
    let nested = s.classify(&f, &["zsh", "-c", source], &[]);
    assert_eq!(nested["classification"], "UNKNOWN");
    assert_eq!(nested["uncertainty"], "OBSERVE");
    let (job, _) = execute_dialect(&s, &f, "zsh", source, &[], 1);
    assert_eq!(job["classification"]["classification"], "UNKNOWN");
    assert_eq!(job["classification"]["uncertainty"], "OBSERVE");
    assert_eq!(
        fs::read_to_string(s.home.join("zsh-startup-marker")).unwrap(),
        "localhost\n"
    );
    assert_eq!(admission_counts(&f), (0, 0));
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn makefile_selection_and_post_capture_changes_fail_closed_on_both_routes() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = shell_sandbox(&f);
    write(
        &s.repo.join(".git/config"),
        "[remote \"origin\"]\n url = https://github.com/example/app.git\n",
    );
    let makefile = s.repo.join("Makefile");
    let literal = "test:\n\tcargo test\n";
    write(&makefile, literal);
    let mut daemon = f.spawn_isolated("test");
    let source = "make test && gh workflow run ci.yml";
    let replacement = makefile.to_str().unwrap();
    let env = [("ARC_TEST_CAPTURE_REPLACE_MAKEFILE", Some(replacement))];
    let mut inspect = s.command(&f, &env);
    inspect.args(["classify", "--json", "--", source]);
    let output = child_watchdog::output(&mut inspect, "changed Makefile inspection").unwrap();
    assert!(output.status.success(), "{output:?}");
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["classification"], "UNKNOWN");
    assert_eq!(result["uncertainty"], "OBSERVE");
    assert_eq!(s.calls(), 0);
    assert_ne!(fs::read_to_string(&makefile).unwrap(), literal);
    write(&makefile, literal);
    let (job, _) = execute_shell(&s, &f, source, &env, 2);
    assert_eq!(job["classification"]["classification"], "UNKNOWN");
    assert_eq!(job["classification"]["uncertainty"], "OBSERVE");

    write(&makefile, literal);
    write(
        &s.repo.join("GNUmakefile"),
        "test:\n\t$(TEST_RUNNER) test\n",
    );
    let inspected = s.classify(&f, &["make", "test"], &[]);
    let (job, _) = s.exec(&f, &["make", "test"], &[]);
    assert!(!assert_routes(
        "higher precedence Makefile",
        &inspected,
        &job,
        "UNKNOWN"
    ));
    fs::remove_file(s.repo.join("GNUmakefile")).unwrap();
    let env = [("MAKEFLAGS", Some("--always-make"))];
    let inspected = s.classify(&f, &["make", "test"], &env);
    let (job, _) = s.exec(&f, &["make", "test"], &env);
    assert!(!assert_routes(
        "Make environment",
        &inspected,
        &job,
        "UNKNOWN"
    ));
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn managed_bash_and_zsh_records_name_the_real_shell_child() {
    use std::{
        os::unix::net::UnixListener,
        time::{Duration, Instant},
    };
    let _serial = support::serial();
    let f = Fixture::new();
    let s = shell_sandbox(&f);
    let mut daemon = f.spawn_isolated("test");
    for dialect in ["bash", "zsh"] {
        let socket = f.root.join(format!("{dialect}-control"));
        let listener = UnixListener::bind(&socket).unwrap();
        let mut command = s.command(&f, &[]);
        command
            .env("ARC_FIXTURE_CONTROL", &socket)
            .env("ARC_FIXTURE_MODE", "success")
            .args(["exec", "--", dialect, "-c", "cargo build"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut managed = child_watchdog::Owned::spawn(&mut command).unwrap();
        let (mut release, child) = cli::accept(&listener);
        let id = cli::recent(&f);
        let deadline = Instant::now() + Duration::from_secs(60);
        let job = loop {
            let response = protocol::call(
                &f,
                OPERATION_JOB_STATUS_LOCALITY,
                json!(StatusRequest::ById { job_id: id.clone() }),
            );
            let job = response.result.unwrap()["job"].clone();
            if job["child"]["pid"].is_u64() {
                break job;
            }
            assert!(Instant::now() < deadline, "shell child was not registered");
            std::thread::sleep(Duration::from_millis(10));
        };
        let shell_pid = job["child"]["pid"].as_u64().unwrap();
        assert!(
            child["pid"] == shell_pid || child["ppid"] == shell_pid,
            "{dialect}: stub={child}, job={job}"
        );
        assert_eq!(job["classification"]["classification"], "BUILD");
        release.write_all(b"x").unwrap();
        assert!(managed.wait("managed shell completion").unwrap().success());
    }
    assert_eq!(s.calls(), 2);
    daemon.stop(libc::SIGTERM);
    f.report();
}

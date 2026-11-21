//! Mixed command stages through `arcctl exec` (AT-TRF-12, DET-016).
//! Lane B owns this module; `closing_shell.rs` only registers it.
use super::*;

#[allow(clippy::duplicate_mod)] // Shared with the installed-hook route's tests.
#[path = "stages/admission.rs"]
mod admission;
#[allow(clippy::duplicate_mod)]
#[path = "stages/harness.rs"]
mod harness;
use harness::{
    installed_hook_rewrite, linked as linked_stages, managed, native, observe, output_with_input,
    row, rows, status, Route, BUN, MAKE,
};

/// D2 on the `arcctl exec` route: the bash child of a separated compound
/// stays the MIXED request record, and `stage` is one of its linked stages.
pub(super) fn request_record(f: &Fixture, stage: &Value) -> Value {
    let request = rows(f)
        .into_iter()
        .rev()
        .find(|row| row.class == "MIXED")
        .expect("request record");
    let stages = linked_stages(f, &request.id);
    assert_eq!(stages.len(), 2, "{stages:?}");
    assert_eq!(stage["job_id"], stages[1].as_str(), "{stage}");
    status(f, &request.id)
}

// AT-TRF-12 item 2, DET-016 item 1.
#[test]
fn mixed_bun_stages_via_exec() {
    admission::stage_by_stage(Route::Exec, BUN);
}

// DET-016 item 2: the literal Makefile recipe `cargo test`.
#[test]
fn mixed_make_stages_via_exec() {
    admission::stage_by_stage(Route::Exec, MAKE);
}

// AT-TRF-12 item 3, DET-016 item 3: the shell keeps `&&`. A failing Test stage
// prevents the gh stage on every route, and the stage's own arguments, working
// directory, environment, stdin, streams and exit status are the native ones.
#[test]
fn mixed_failing_test_stage_skips_gh_stage() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = harness::sandbox(&f);
    let mut daemon = f.spawn_isolated("test");
    let routes = [
        Route::Hook("/bin/sh"),
        Route::Hook("/bin/bash"),
        Route::Hook("/bin/zsh"),
        Route::Exec,
    ];
    for (source, reason, code) in [(BUN, "BUN_TEST", 17), (MAKE, "CARGO_TEST", 2)] {
        for route in routes {
            let label = format!("{route:?}: {source}");
            let before = rows(&f).len();
            let (expected, _) = observe(&f, native(route, &s, &f, source), &label);
            assert_eq!(rows(&f).len(), before, "{label}: native run registered");
            let (command, hook_request) = managed(route, &s, &f, source);
            let (actual, pid) = observe(&f, command, &label);
            assert_eq!(actual, expected, "{label}");
            assert_eq!(actual.args, json!([b"test".to_vec()]), "{label}");
            assert_eq!(actual.stdin, json!(b"input\n".to_vec()), "{label}");
            assert_eq!(actual.code, Some(code), "{label}");
            assert_eq!(actual.starts, 1, "{label}: the gh stub started");

            // One Test stage record and no gh record; the permit is released.
            let exec = route == Route::Exec;
            let all = rows(&f);
            assert_eq!(
                all.len(),
                before + 1 + usize::from(exec),
                "{label}: {all:?}"
            );
            assert!(all[before..].iter().all(|row| row.class != "REMOTE_CI"));
            let stage = all.last().unwrap();
            assert_eq!(
                (stage.class.as_str(), stage.locality.as_str()),
                ("TEST", "HOST_LOCAL"),
                "{label}"
            );
            assert_eq!(stage.permit.as_deref(), Some("RELEASED"), "{label}");
            let recorded = status(&f, &stage.id);
            assert_eq!(recorded["classification"]["reason"], reason, "{label}");
            assert_eq!(recorded["outcome"]["code"], code, "{label}");
            if source == BUN {
                assert_eq!(recorded["child"]["pid"].as_u64(), pid, "{label}");
            }
            let request = hook_request.unwrap_or_else(|| all[before].id.clone());
            assert_eq!(
                linked_stages(&f, &request),
                std::slice::from_ref(&stage.id),
                "{label}"
            );
            if exec {
                let shell = status(&f, &request);
                assert_eq!(shell["classification"]["classification"], "MIXED");
                assert_eq!(shell["outcome"]["code"], code, "{label}");
            }
        }
    }
    daemon.stop(libc::SIGTERM);
    f.report();
}

// DET-016 item 4: a compound that cannot be separated safely keeps today's
// single MIXED or UNKNOWN/OBSERVE record, with no stage, queue entry or
// reservation, and the shell that received it runs it unchanged.
#[test]
fn mixed_unseparable_compounds_pass_through() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = harness::sandbox(&f);
    for shell in ["sh", "zsh"] {
        symlink(format!("/bin/{shell}"), s.bin.join(shell)).unwrap();
    }
    let startup = s.home.join("startup.sh");
    let started = f.root.join("startup-runs");
    write(&startup, "printf 'startup\\n' >> \"$STARTUP_RUNS\"\n");
    let (startup, runs) = (startup.to_str().unwrap(), started.to_str().unwrap());
    let mut daemon = f.spawn_isolated("test");

    // The installed hook leaves every other spelling to the vendor's shell.
    for source in [
        "bun test $(printf x) && gh workflow run ci.yml",
        "bun test && gh workflow run ci.yml > dispatch.log",
        "bun test && gh workflow run \"$WORKFLOW\"",
        "bash -lc 'bun test && gh workflow run ci.yml'",
        "bash -c 'make test && gh workflow run ci.yml'",
        "make test && gh workflow run ci.yml ",
    ] {
        assert_eq!(installed_hook_rewrite(&s, &f, source), None, "{source}");
    }

    // Name, shell, source and the extra environment of one pass-through case.
    type Case<'a> = (&'a str, &'a str, &'a str, Vec<(&'a str, &'a str)>);
    let cases: [Case; 9] = [
        (
            "substitution",
            "bash",
            "bun test $(printf x) && gh workflow run ci.yml",
            vec![],
        ),
        (
            "redirection",
            "bash",
            "bun test && gh workflow run ci.yml > dispatch.log",
            vec![],
        ),
        (
            "dynamic target",
            "bash",
            "bun test && gh workflow run \"$WORKFLOW\"",
            vec![("WORKFLOW", "ci.yml")],
        ),
        (
            "nested shell",
            "bash",
            "bash -c 'make test && gh workflow run ci.yml'",
            vec![],
        ),
        (
            "startup file",
            "bash",
            BUN,
            vec![("BASH_ENV", startup), ("STARTUP_RUNS", runs)],
        ),
        (
            "exported options",
            "bash",
            MAKE,
            vec![("SHELLOPTS", "xtrace")],
        ),
        (
            "exported function",
            "bash",
            BUN,
            vec![("BASH_FUNC_bun%%", "() { command bun \"$@\"; }")],
        ),
        ("sh", "sh", BUN, vec![]),
        ("zsh", "zsh", MAKE, vec![]),
    ];
    for (name, shell, source, environment) in cases {
        let configure = |command: &mut Command| {
            command.env("ARC_FIXTURE_MODE", "success");
            for (key, value) in &environment {
                command.env(key, value);
            }
        };
        let mut reference = Command::new(s.bin.join(shell));
        reference
            .env_clear()
            .args(["-c", source])
            .current_dir(&s.repo);
        for (key, value) in s.command(&f, &[]).get_envs() {
            if let Some(value) = value {
                reference.env(key, value);
            }
        }
        configure(&mut reference);
        let reference = output_with_input(&mut reference, b"", name);
        assert!(reference.status.success(), "{name}: {reference:?}");

        let (before, calls, admission) = (rows(&f), s.calls(), admission_counts(&f));
        let startups = fs::read_to_string(&started).unwrap_or_default().len();
        let mut command = s.command(&f, &[]);
        command.args(["exec", "--", shell, "-c", source]);
        configure(&mut command);
        let output = output_with_input(&mut command, b"", name);
        assert_eq!(output.status.code(), Some(0), "{name}: {output:?}");
        assert_eq!(output.stdout, reference.stdout, "{name}");
        assert_eq!(
            output.stderr, reference.stderr,
            "{name}: ARC wrote to stderr"
        );
        assert_eq!(s.calls(), calls + 2, "{name}");
        if name == "startup file" {
            let after = fs::read_to_string(&started).unwrap().len();
            assert_eq!(after - startups, "startup\n".len(), "{name}");
        }

        let all = rows(&f);
        assert_eq!(all.len(), before.len() + 1, "{name}: a stage was recorded");
        let record = row(&f, &all.last().unwrap().id);
        assert_eq!((&record.entry, &record.permit), (&None, &None), "{name}");
        let job = status(&f, &record.id);
        match record.class.as_str() {
            "MIXED" => assert_eq!(job["classification"]["locality"], "MIXED", "{name}"),
            "UNKNOWN" => assert_eq!(job["classification"]["uncertainty"], "OBSERVE", "{name}"),
            other => panic!("{name} was recorded {other}: {job}"),
        }
        assert_eq!(job["outcome"]["code"], 0, "{name}");
        assert_eq!(
            linked_stages(&f, &record.id),
            Vec::<String>::new(),
            "{name}"
        );
        assert_eq!(admission_counts(&f), admission, "{name} was admitted");
    }
    daemon.stop(libc::SIGTERM);
    f.report();
}

// AT-TRF-12 item 4, DET-016 item 5: classification and the hook's rewrite
// analyse the command graph and start nothing, even while a stage would wait.
#[test]
fn mixed_classify_executes_nothing() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = harness::sandbox(&f);
    symlink("/bin/zsh", s.bin.join("zsh")).unwrap();
    let mut daemon = f.spawn_isolated("test");
    let held = harness::hold(&s, &f, "test");
    let (before, calls) = (rows(&f), s.calls());
    for source in [BUN, MAKE] {
        for inspection in inspect_shell(&s, &f, source, &[]) {
            assert_eq!(
                inspection,
                json!({"classification":"MIXED","locality":"MIXED",
                    "reason":"COMPOUND_MIXED","uncertainty":"NONE"}),
                "{source}"
            );
        }
        let nested = s.classify(&f, &["bash", "-c", source], &[]);
        assert_eq!(nested["classification"], "MIXED", "{source}: {nested}");
        let rewritten = installed_hook_rewrite(&s, &f, source).unwrap();
        assert_eq!(rewritten.matches(" hook-exec --arc-request ").count(), 2);
    }
    assert_eq!(s.calls(), calls, "classification started a stub");
    assert_eq!(rows(&f), before, "classification recorded or queued a job");
    held.release(&f);
    daemon.stop(libc::SIGTERM);
    f.report();
}

// Contract seam: the request option links each stage record to one request and
// never reaches the child, and an unseparable compound keeps its pass-through.
#[test]
fn stage_seam_links_hook_exec_records_and_leaves_unseparable_compound_alone() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = shell_sandbox(&f);
    write(
        &s.repo.join(".git/config"),
        "[remote \"origin\"]\n url = https://github.com/example/app.git\n",
    );
    let mut daemon = f.spawn_isolated("test");
    let request = arc_core::process_identity::random_job_id().unwrap();
    assert_eq!(linked_stages(&f, &request), Vec::<String>::new());

    let mut linked = Vec::new();
    let literals: [(&[&str], &str); 2] = [
        (&["cargo", "build"], "BUILD"),
        (&["gh", "workflow", "run", "build.yml"], "REMOTE_CI"),
    ];
    for (argv, class) in literals {
        let before = s.calls();
        let mut command = s.command(&f, &[]);
        command
            .env("ARC_FIXTURE_MODE", "success")
            .args(["hook-exec", "--arc-request", &request, "--"])
            .args(argv);
        let output = child_watchdog::output(&mut command, "linked stage").unwrap();
        assert!(output.status.success(), "{argv:?}: {output:?}");
        assert_eq!(s.calls(), before + 1, "{argv:?}");
        let child: Value = serde_json::from_slice(&output.stdout).unwrap();
        let arguments: Vec<Vec<u8>> = argv[1..]
            .iter()
            .map(|arg| arg.as_bytes().to_vec())
            .collect();
        assert_eq!(child["args"], json!(arguments), "{argv:?}");
        let id = cli::recent(&f);
        let response = protocol::call(
            &f,
            OPERATION_JOB_STATUS_LOCALITY,
            json!(StatusRequest::ById { job_id: id.clone() }),
        );
        let job = response.result.unwrap()["job"].clone();
        assert_eq!(job["classification"]["classification"], class, "{job}");
        assert_eq!(job["outcome"]["code"], 0, "{job}");
        linked.push(id.as_str().to_owned());
        assert_eq!(linked_stages(&f, &request), linked, "{argv:?}");
    }

    // Only the wrapper that registered a job may link it. This process is
    // neither that observer's peer nor the observer the job row names.
    let other = arc_core::process_identity::random_job_id().unwrap();
    let first: JobId = serde_json::from_value(json!(linked[0])).unwrap();
    let stored = protocol::call(
        &f,
        OPERATION_JOB_STATUS_LOCALITY,
        json!(StatusRequest::ById { job_id: first }),
    );
    let wrapper = stored.result.unwrap()["job"]["observer"].clone();
    for observer in [wrapper, json!(protocol::observer())] {
        let forged = protocol::call(
            &f,
            arc_ipc::request::OPERATION,
            json!({"action":"LINK","request_id":other,"job_id":linked[0],"observer":observer}),
        );
        assert_eq!(forged.status, "error", "{forged:?}");
        assert_eq!(forged.error.unwrap().code, "INVALID_REQUEST");
    }
    assert_eq!(linked_stages(&f, &other), Vec::<String>::new());
    assert_eq!(linked_stages(&f, &request), linked);

    // A value the hook did not write starts nothing and links nothing.
    let before = s.calls();
    let mut command = s.command(&f, &[]);
    command.args([
        "hook-exec",
        "--arc-request",
        "not-a-request",
        "--",
        "cargo",
        "build",
    ]);
    let output = child_watchdog::output(&mut command, "malformed request").unwrap();
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert_eq!(s.calls(), before);

    let source = "bun test $(printf x) && gh workflow run ci.yml";
    for inspection in inspect_shell(&s, &f, source, &[]) {
        assert!(
            matches!(
                inspection["classification"].as_str(),
                Some("MIXED" | "UNKNOWN")
            ),
            "{inspection}"
        );
    }
    let admission = admission_counts(&f);
    let (job, _) = execute_dialect(&s, &f, "bash", source, &[], 2);
    match job["classification"]["classification"].as_str().unwrap() {
        "UNKNOWN" => assert_eq!(job["classification"]["uncertainty"], "OBSERVE", "{job}"),
        "MIXED" => assert_eq!(job["classification"]["locality"], "MIXED", "{job}"),
        other => panic!("unseparable compound was recorded {other}: {job}"),
    }
    let shell = job["job_id"].as_str().unwrap();
    assert_eq!(linked_stages(&f, shell), Vec::<String>::new());
    assert_eq!(admission_counts(&f), admission, "pass-through was admitted");
    assert_eq!(linked_stages(&f, &request), linked);
    daemon.stop(libc::SIGTERM);
    f.report();
}

// Fail-open: with no daemon each stage of a separated compound still runs
// exactly once on both routes, and stdout and the exit status are native.
#[test]
fn mixed_stages_run_once_without_a_daemon() {
    let _serial = support::serial();
    let f = Fixture::new();
    let s = harness::sandbox(&f);
    assert!(!f.socket.exists());
    for source in [BUN, MAKE] {
        for route in [Route::Hook("/bin/sh"), Route::Exec] {
            let label = format!("{route:?}: {source}");
            let mut reference = native(route, &s, &f, source);
            reference.env("ARC_FIXTURE_MODE", "success");
            let reference = output_with_input(&mut reference, b"", &label);
            assert!(reference.status.success(), "{label}: {reference:?}");
            let calls = s.calls();
            let (mut command, _) = managed(route, &s, &f, source);
            command.env("ARC_FIXTURE_MODE", "success");
            let output = output_with_input(&mut command, b"", &label);
            assert_eq!(output.status.code(), Some(0), "{label}: {output:?}");
            assert_eq!(output.stdout, reference.stdout, "{label}");
            assert_eq!(s.calls(), calls + 2, "{label}: {output:?}");
            let stderr = String::from_utf8(output.stderr).unwrap();
            assert!(stderr.contains("TRACKING_UNAVAILABLE"), "{label}: {stderr}");
            assert_eq!(stderr.matches("fixture-stderr\n").count(), 2, "{label}");
        }
    }
}

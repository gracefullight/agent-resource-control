//! Direct and managed invocations share sources, argv and private fixture isolation.
use super::*;
use arc_core::process_identity::{probe_live, IdentityStatus, ProcessIdentity};
use serde_json::json;
use std::{
    fs::{self, File},
    process::Stdio,
};

pub(super) struct Baseline {
    pub code: i32,
    pub stdout: Vec<String>,
    pub stderr: Vec<String>,
    pub facts: Value,
}

pub(super) fn direct(prepared: &Prepared, tools: &Tools) -> Baseline {
    let mut command = Command::new(&prepared.argv[0]);
    command
        .args(&prepared.argv[1..])
        .current_dir(&prepared.project)
        .env(
            "PATH",
            format!(
                "{}:{}",
                tools.shims.display(),
                std::env::var("PATH").unwrap()
            ),
        )
        .stdin(Stdio::null())
        .stdout(File::create(prepared.project.join("direct-stdout")).unwrap())
        .stderr(File::create(prepared.project.join("direct-stderr")).unwrap());
    (prepared.configure)(&prepared.project, &mut command);
    let wrapper = crate::native::OwnedChild::new(command.spawn().unwrap());
    let identity = ProcessIdentity::capture(wrapper.child.id()).unwrap();
    // Reuse the established native ownership/drop guard for this direct route.
    let mut run = Run {
        wrapper,
        identity: identity.clone(),
        project: prepared.project.clone(),
        fifo: None,
    };
    eventually(|| {
        assert!(
            run.wrapper.child.try_wait().unwrap().is_none(),
            "direct native tool exited before barrier: {}\n{}",
            log(&prepared.project.join("direct-stdout")),
            log(&prepared.project.join("direct-stderr"))
        );
        (prepared.ready)(&mut run)
    });
    let image = executable(run.wrapper.child.id());
    assert_eq!(
        image,
        prepared.executable.canonicalize().unwrap(),
        "observer must exec the real tool"
    );
    assert_eq!(log(&prepared.project.join("launches")).lines().count(), 1);
    let payload = ProcessIdentity::capture(project::payload_pid(&prepared.project)).unwrap();
    run.release();
    let status = run.wait();
    let stdout = fs::read(prepared.project.join("direct-stdout")).unwrap();
    let stderr = fs::read(prepared.project.join("direct-stderr")).unwrap();
    let code = status
        .code()
        .expect("direct invocation should exit, not signal");
    assert_eq!(probe_live(&identity), IdentityStatus::Missing);
    assert_eq!(probe_live(&payload), IdentityStatus::Missing);
    let out = project::records(prepared.form, &stdout);
    let err = project::records(prepared.form, &stderr);
    assert_native(prepared, &out, &err);
    Baseline {
        code,
        stdout: out.clone(),
        stderr: err.clone(),
        facts: json!({
        "argv":prepared.argv,"cwd":prepared.project,"executable":image,"sha256":sha(&image),
        "exitCode":code,"nativeStdout":out,"nativeStderr":err,"starts":1,"bodyExecutions":1,
        "explicitBarrier":true,"payload":payload,"ownedChildrenCleaned":true,"offlinePrepared":true}),
    }
}

fn assert_native(prepared: &Prepared, stdout: &[String], stderr: &[String]) {
    let stdout = stdout.concat();
    let stderr = stderr.concat();
    let phase = if prepared.form == Form::CmakeBuild {
        "build"
    } else {
        "test"
    };
    for expected in [
        "ordinary stdout before".to_owned(),
        format!("NATIVE_PHASE={phase}"),
        "NATIVE_VALUE=literal $HOME ' 日本語".to_owned(),
        format!("NATIVE_CWD={}", prepared.project.display()),
        "NATIVE_ARGUMENT=literal $HOME ' 日本語".to_owned(),
        "ordinary stdout after".to_owned(),
    ] {
        assert_eq!(
            stdout.matches(&expected).count(),
            1,
            "native stdout {expected}: {stdout:?}"
        );
        assert!(
            !stderr.contains(&expected),
            "stdout payload moved to stderr"
        );
    }
    // CTest intentionally merges its child pipes onto CTest's own stdout.
    let (native_stderr, other) = if prepared.form == Form::Ctest {
        (&stdout, &stderr)
    } else {
        (&stderr, &stdout)
    };
    for expected in [
        "ordinary stderr before".to_owned(),
        format!("NATIVE_STDERR={phase}"),
        "ordinary stderr after".to_owned(),
    ] {
        assert_eq!(
            native_stderr.matches(&expected).count(),
            1,
            "{native_stderr:?}"
        );
        assert!(!other.contains(&expected), "stderr payload moved streams");
    }
}

pub(super) fn semantics(read: Read) {
    let case = case("cmake-execution-semantics", read);
    // Direct baselines have independent cloned trees and do not use the
    // per-user daemon. Managed jobs remain sequential on the single daemon.
    let baselines = std::thread::scope(|scope| {
        let mut workers = Vec::new();
        for form in [Form::CmakeBuild, Form::Ctest] {
            for fail in [false, true] {
                let case = &case;
                workers.push(scope.spawn(move || {
                    let name = format!(
                        "{}-{} project",
                        form.id(),
                        if fail { "failure" } else { "success" }
                    );
                    let prepared =
                        project::prepare(&case.artifacts.join(name), form, &case.tools, true, fail);
                    let baseline = direct(&prepared, &case.tools);
                    assert!(
                        !baseline
                            .stdout
                            .iter()
                            .any(|line| line.starts_with("-- Configuring done")),
                        "relocating a prepared tree must not trigger CMake reconfiguration"
                    );
                    (form, fail, prepared, baseline)
                }));
            }
        }
        workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect::<Vec<_>>()
    });
    let mut observations = Vec::new();
    for (form, fail, prepared, baseline) in baselines {
        let expected = if !fail {
            0
        } else if form == Form::CmakeBuild {
            2
        } else {
            8
        };
        assert_eq!(baseline.code, expected, "intended native build/test result");
        project::clear_records(&prepared.project);
        fs::remove_file(prepared.project.join("release")).unwrap();
        let mut run = prepared.start(&case, None);
        let entry = entered(&case, &mut run, &prepared);
        let payload = ProcessIdentity::capture(project::payload_pid(&prepared.project)).unwrap();
        run.finish(expected);
        let terminal = terminal(&case, &run, form, expected);
        let stdout = project::records(form, &fs::read(prepared.project.join("stdout")).unwrap());
        let stderr = project::records(form, &fs::read(prepared.project.join("stderr")).unwrap());
        assert_eq!(
            stdout, baseline.stdout,
            "native stdout survives managed execution"
        );
        assert_eq!(
            stderr, baseline.stderr,
            "native stderr survives managed execution"
        );
        assert_native(&prepared, &stdout, &stderr);
        assert_eq!(probe_live(&entry.child), IdentityStatus::Missing);
        assert_eq!(probe_live(&payload), IdentityStatus::Missing);
        observations.push(
            json!({"form":form.id(),"failure":fail,"baseline":baseline.facts,
                "entry":entry,"terminal":terminal,"payload":payload,"starts":1,"bodyExecutions":1,
                "originalExecutionSemantics":true,"ownedChildrenCleaned":true}),
        );
    }
    let permits = [
        permit_history(&case, "BUILD", 4, 1),
        permit_history(&case, "TEST", 4, 1),
    ];
    case.save(
        "cmake-success-failure",
        json!({"forms":["cmake --build","ctest"],
        "observations":observations,"permits":permits,"offlinePrepared":true}),
    );
    case.done();
}

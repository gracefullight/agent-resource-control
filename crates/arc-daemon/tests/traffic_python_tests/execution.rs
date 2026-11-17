//! Direct/native parity and production-daemon Meter/lineage checks.
use super::*;

static MANAGED_PARITY: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub fn semantics(read: Read) {
    let case = case("semantics", read, false);
    let mut observations = Vec::new();
    let scenarios: Vec<(&str, Kind, Vec<&str>)> = vec![
        ("pytest-console-discovery", Kind::Pytest, vec!["pytest"]),
        (
            "pytest-module-discovery",
            Kind::Pytest,
            vec!["python", "-m", "pytest"],
        ),
        (
            "pytest-file-node",
            Kind::Pytest,
            vec![
                "python3",
                "-m",
                "pytest",
                "test_fixture.py::TestFixture::test_selected",
            ],
        ),
        (
            "pytest-core-selectors",
            Kind::Pytest,
            vec![
                "pytest",
                "-q",
                "-k",
                "selected and not absent",
                "-m",
                "not external",
                "--maxfail=1",
                "--tb=no",
                "--capture=no",
                "--color=no",
            ],
        ),
        (
            "unittest-default",
            Kind::Unittest,
            vec!["python", "-m", "unittest"],
        ),
        (
            "unittest-discover",
            Kind::Unittest,
            vec!["python", "-m", "unittest", "discover"],
        ),
        (
            "unittest-discover-selectors",
            Kind::Unittest,
            vec![
                "python",
                "-m",
                "unittest",
                "discover",
                "-s",
                ".",
                "--pattern=test_*.py",
                "--top-level-directory=.",
                "-k",
                "selected",
                "-q",
            ],
        ),
        (
            "unittest-dotted-selector",
            Kind::Unittest,
            vec![
                "python",
                "-m",
                "unittest",
                "-v",
                "test_fixture.TestFixture.test_selected",
            ],
        ),
        (
            "unittest-file-selector",
            Kind::Unittest,
            vec!["python", "-m", "unittest", "test_fixture.py"],
        ),
    ];
    let mut independent: Vec<(String, Kind, Vec<&str>, i32, bool)> = scenarios
        .into_iter()
        .map(|(name, kind, argv)| (name.to_owned(), kind, argv, 0, false))
        .collect();
    for kind in [Kind::Pytest, Kind::Unittest] {
        independent.push((format!("{kind:?}-failure"), kind, kind.argv(), 1, false));
        independent.push((format!("{kind:?}-stdin"), kind, kind.argv(), 0, true));
    }
    for kind in [Kind::Pytest, Kind::Unittest] {
        independent.push((format!("{kind:?}-exit17"), kind, kind.argv(), 17, false));
    }
    // Projects and native children are private to each case. Keep bounded
    // batches so the direct baselines can overlap while the production daemon
    // still enforces the single TEST permit for managed executions.
    for batch in independent.chunks(4) {
        observations.extend(thread::scope(|scope| {
            let jobs: Vec<_> = batch
                .iter()
                .map(|(name, kind, argv, code, stdin)| {
                    let case = &case;
                    scope.spawn(move || parity(case, name, *kind, argv, *code, *stdin, None))
                })
                .collect();
            jobs.into_iter()
                .map(|job| job.join().unwrap())
                .collect::<Vec<_>>()
        }));
    }
    // An actual venv interpreter (no fake pytest script, pip or installation).
    // Both lexical relative and absolute venv executable paths are exercised.
    let venv_root = case.artifacts.join("venv runtime 日本語");
    let venv = case.tools.python.venv(&venv_root.join(".venv"));
    // Keep the genuine installed console script and its original shebang.
    // A lexical venv alias must not make ARC select a different interpreter.
    std::os::unix::fs::symlink(
        &case.tools.python.console,
        venv_root.join(".venv/bin/pytest"),
    )
    .unwrap();
    for (name, kind, argv) in [
        ("venv-console-alias", Kind::Pytest, vec![".venv/bin/pytest"]),
        (
            "venv-pytest",
            Kind::Pytest,
            vec![
                venv.to_str().unwrap(),
                "-m",
                "pytest",
                "test_fixture.py::TestFixture::test_selected",
            ],
        ),
        (
            "venv-unittest",
            Kind::Unittest,
            vec![".venv/bin/python", "-m", "unittest", "discover"],
        ),
    ] {
        observations.push(parity(&case, name, kind, &argv, 0, false, Some(&venv_root)));
    }
    let unresolved = unresolved(&case);
    let history = permit_history(&case, observations.len(), 1);
    case.save("execution-semantics", json!({"realPythonAndPytest":case.tools.receipt(),"directWrapped":observations,"unresolvedBypass":unresolved,"permitHistory":history,"fixtureOnlyIsolation":true}));
    case.done();
}

fn parity(
    case: &Case,
    name: &str,
    kind: Kind,
    argv: &[&str],
    code: i32,
    stdin: bool,
    venv_root: Option<&Path>,
) -> Value {
    let p = projects::project(case, name, kind, false);
    if let Some(venv) = venv_root {
        std::os::unix::fs::symlink(venv.join(".venv"), p.join(".venv")).unwrap();
    }
    let input = "stdin\0bytes 日本語\n".as_bytes();
    let configure = |cmd: &mut Command| {
        projects::configure(case, &p, cmd);
        cmd.env("TBS_FAIL", if code == 1 { "1" } else { "0" })
            .env("TBS_EXIT", if code == 17 { "17" } else { "" })
            .env("TBS_STDIN", if stdin { "1" } else { "" })
            .stdin(if stdin { Stdio::piped() } else { Stdio::null() });
    };
    let mut cmd = Command::new(argv[0]);
    cmd.args(&argv[1..])
        .current_dir(&p)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure(&mut cmd);
    let mut direct = native::OwnedChild::new(cmd.spawn().unwrap());
    if stdin {
        direct.child.stdin.take().unwrap().write_all(input).unwrap();
    }
    let expected = direct.wait_with_output().unwrap();
    assert_eq!(expected.status.code(), Some(code), "{name}: {expected:?}");
    assert_eq!(log(&p.join("bodies")).lines().count(), 1);
    let expected_native: Value =
        serde_json::from_slice(&fs::read(p.join("native.json")).unwrap()).unwrap();
    // A queued managed run has legitimate ARC notices on stderr. Keep managed
    // parity invocations mutually exclusive so the exact native-stream
    // assertion remains meaningful while independent native baselines overlap.
    let _managed = MANAGED_PARITY.lock().unwrap();
    write(&p.join("bodies"), "");
    write(&p.join("launches"), "");
    let mut managed = start_with(case, &p, argv, configure);
    if stdin {
        managed
            .wrapper
            .child
            .stdin
            .take()
            .unwrap()
            .write_all(input)
            .unwrap();
    }
    assert_eq!(
        managed.wait().code(),
        expected.status.code(),
        "{name}: {}",
        log(&p.join("stderr"))
    );
    assert_eq!(
        fs::read(p.join("stdout")).unwrap(),
        expected.stdout,
        "{name}: exact stdout"
    );
    assert_eq!(
        fs::read(p.join("stderr")).unwrap(),
        expected.stderr,
        "{name}: exact stderr"
    );
    let native: Value = serde_json::from_slice(&fs::read(p.join("native.json")).unwrap()).unwrap();
    assert_eq!(
        native, expected_native,
        "{name}: cwd/env/executable/argv/import path must match native invocation"
    );
    assert_eq!(native["dependency"], "loaded from caller PYTHONPATH");
    assert_eq!(native["env"], "literal $HOME ' value 日本語");
    if stdin {
        assert_eq!(native["input"], json!(input.to_vec()));
    }
    if venv_root.is_some() && argv[0].ends_with("python") {
        assert_ne!(native["prefix"], native["base_prefix"]);
    }
    assert_eq!(log(&p.join("bodies")).lines().count(), 1);
    if venv_root.is_none() {
        assert_eq!(log(&p.join("launches")).lines().count(), 1);
    }
    let result = case.terminal(&managed, kind.class(), kind.reason(), code);
    json!({"case":name,"originalArgv":argv,"native":native,"exactStdout":String::from_utf8(expected.stdout).unwrap(),"exactStderr":String::from_utf8(expected.stderr).unwrap(),"exit":code,"bodyCount":1,"result":result})
}

fn unresolved(case: &Case) -> Value {
    let p = projects::project(case, "unresolved-native-option", Kind::Pytest, false);
    // pytest supports this option; the frozen admission grammar deliberately
    // leaves it OBSERVE. The original process must still execute successfully.
    let argv = ["python", "-m", "pytest", "--strict-markers"];
    let mut command = Command::new(argv[0]);
    command
        .args(&argv[1..])
        .current_dir(&p)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    projects::configure(case, &p, &mut command);
    let expected = native::OwnedChild::new(command.spawn().unwrap())
        .wait_with_output()
        .unwrap();
    assert_eq!(expected.status.code(), Some(0));
    let native_expected = fs::read(p.join("native.json")).unwrap();
    write(&p.join("bodies"), "");
    write(&p.join("launches"), "");
    let mut run = start(case, &p, &argv);
    assert_eq!(run.wait().code(), Some(0));
    assert_eq!(fs::read(p.join("stdout")).unwrap(), expected.stdout);
    assert_eq!(fs::read(p.join("stderr")).unwrap(), expected.stderr);
    assert_eq!(fs::read(p.join("native.json")).unwrap(), native_expected);
    let id = case.job(run.wrapper.child.id());
    eventually(|| case.row(&id)["outcome_kind"] == "EXITED");
    let result = case.reconcile(&id, "UNKNOWN", "UNRESOLVED_COMMAND");
    assert!(result["diagnostic"]["admission"].is_null());
    assert_eq!(result["sqlite"]["exit_code"], 0);
    assert_eq!(log(&p.join("bodies")).lines().count(), 1);
    assert_eq!(log(&p.join("launches")).lines().count(), 1);
    let cli = case.cli(&["meter", "summary", "--job", &id, "--json"]);
    assert!(cli.status.success());
    let meter: Value = serde_json::from_slice(&cli.stdout).unwrap();
    assert_eq!(
        meter,
        (case.read)(&case.fixture.socket, "meter_summary", json!({"job_id":id}))
    );
    assert_eq!(
        meter["accounting"]["report"]["outcome"],
        json!({"kind":"EXITED","code":0})
    );
    json!({"originalArgv":argv,"result":result,"meter":meter,"starts":1,"bodyCount":1,"noAdmissionOrReservation":true,"exactNativeStreamsAndImports":true})
}

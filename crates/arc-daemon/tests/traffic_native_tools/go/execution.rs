//! Delivered for the integration owner to run after shared product negotiation.
use super::{super::*, direct as compare, projects::*};
use serde_json::{json, Value};
use std::{fs, io::Write, path::Path, process::Stdio};

fn case(name: &str, read: Read) -> Case<Tools> {
    let tools = Tools::installed();
    let receipt = tools.receipt();
    let observers = tools.observers();
    Case::with_tools(name, read, false, tools, receipt, observers)
}

pub fn semantics(read: Read) {
    let case = case("go-semantics", read);
    let mut observations = Vec::new();
    for form in FORMS {
        for fail in [false, true] {
            let path = project(
                &case.artifacts,
                &case.tools,
                &format!("{}-{fail}", form.id()),
                form,
                false,
                fail,
            );
            observations.push(parity(&case, &path, form, fail));
        }
    }
    let mut bypasses = Vec::new();
    for (name, args) in [
        (
            "compile-only",
            vec!["go", "test", "-c", "-o", "test output"],
        ),
        (
            "execution-wrapper",
            vec!["go", "test", "-count=1", "-exec", "/usr/bin/env", "."],
        ),
        ("dry-run", vec!["go", "build", "-n", "."]),
    ] {
        let path = project(
            &case.artifacts,
            &case.tools,
            name,
            Form::GoTest,
            false,
            false,
        );
        let argv: Vec<_> = args.iter().map(|s| (*s).to_owned()).collect();
        let expected = direct(&path, &case.tools.go, &argv, b"");
        assert_eq!(expected.status.code(), Some(0), "{expected:?}");
        write(&path.join("test-bodies"), "");
        let mut run = Run::start_with(&case, &path, &args, |cmd| {
            configure(&path, cmd);
        });
        assert_eq!(run.wait().code(), Some(0), "{}", log(&path.join("stderr")));
        for (stream, expected) in [("stdout", &expected.stdout), ("stderr", &expected.stderr)] {
            let actual = fs::read(path.join(stream)).unwrap();
            if name == "dry-run" {
                assert_eq!(
                    compare::dry_streams(expected),
                    compare::dry_streams(&actual)
                );
            } else {
                assert_eq!(
                    compare::streams(Form::GoTest, expected),
                    compare::streams(Form::GoTest, &actual)
                );
            }
        }
        let id = case.job(run.wrapper.child.id());
        eventually(|| case.row(&id)["outcome_kind"] == "EXITED");
        let facts = case.reconcile(&id, "UNKNOWN", "UNRESOLVED_COMMAND");
        assert!(facts["diagnostic"]["admission"].is_null());
        assert_eq!(facts["sqlite"]["exit_code"], 0);
        assert!(facts["sqlite"]["permit_job"].is_null());
        assert_eq!(run.count("go"), 1);
        assert_eq!(
            log(&path.join("test-bodies")).lines().count(),
            usize::from(name == "execution-wrapper")
        );
        bypasses.push(
            json!({"mode":name,"originalArgv":args,"facts":facts,"starts":1,"noPermit":true}),
        );
    }
    let permits = ["BUILD", "TEST", "LINT"].map(|class| permit_history(&case, class, 6, 1));
    case.save("go-execution-semantics", json!({"tools":case.tools.receipt(),"successFailure":observations,
        "unsupportedModesStillExecute":bypasses,"permitHistory":permits,"fixtureOnlyIsolation":true}));
    case.done();
}

fn parity(case: &Case<Tools>, path: &Path, form: Form, fail: bool) -> Value {
    let arguments = argv(form);
    let input = "stdin\0bytes 日本語\n".as_bytes();
    let expected = direct(path, &case.tools.go, &arguments, input);
    compare::assert_result(form, fail, &expected);
    let expected_facts = (form == Form::GoTest).then(|| compare::facts(path));
    let archive = (form == Form::GoBuild && !fail).then(|| sha(&path.join("native output.a")));
    write(&path.join("test-bodies"), "");
    let args: Vec<_> = arguments.iter().map(String::as_str).collect();
    let mut run = Run::start_with(case, path, &args, |cmd| {
        configure(path, cmd);
        cmd.stdin(Stdio::piped());
    });
    run.wrapper
        .child
        .stdin
        .take()
        .unwrap()
        .write_all(input)
        .unwrap();
    assert_eq!(
        run.wait().code(),
        expected.status.code(),
        "{}",
        log(&path.join("stderr"))
    );
    assert_eq!(
        compare::streams(form, &fs::read(path.join("stdout")).unwrap()),
        compare::streams(form, &expected.stdout)
    );
    assert_eq!(
        compare::streams(form, &fs::read(path.join("stderr")).unwrap()),
        compare::streams(form, &expected.stderr)
    );
    if let Some(expected) = &expected_facts {
        assert_eq!(
            compare::facts(path),
            *expected,
            "native cwd, argv, environment and input must match"
        );
        assert_eq!(expected["value"], "literal $HOME ' value 日本語");
        assert_eq!(log(&path.join("test-bodies")).lines().count(), 1);
    }
    if let Some(expected) = archive {
        assert_eq!(
            sha(&path.join("native output.a")),
            expected,
            "same native build output"
        );
    }
    let result = terminal(case, &run, form, i32::from(fail));
    assert_eq!(case.active()["permits"], 0);
    json!({"form":form.id(),"originalArgv":arguments,"failure":fail,"exit":expected.status.code(),
        "nativeFacts":expected_facts,"sameStreamsExceptNativeElapsedSeconds":true,
        "exactlyOneNativeExecution":true,"result":result})
}

pub fn pairs(read: Read) {
    for descriptor in ORDERED_PAIRS
        .into_iter()
        .filter(|p| FORMS.contains(&p.first) || FORMS.contains(&p.second))
    {
        let case = case(descriptor.id, read);
        let prepare = |name: &str, form| {
            if FORMS.contains(&form) {
                prepared(
                    project(&case.artifacts, &case.tools, name, form, true, false),
                    &case.tools,
                    form,
                )
            } else {
                cargo_peer(&case, name, form, &case.tools.cargo)
            }
        };
        let first = prepare("first", descriptor.first);
        let second = prepare("second", descriptor.second);
        pair(case, descriptor, first, second);
    }
}

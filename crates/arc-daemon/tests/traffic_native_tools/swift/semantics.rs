//! Success/failure baselines and managed comparison; shared recovery remains shared.
use super::{projects::*, tools::Tools, *};
use crate::suite::{entered, permit_history, terminal, Run};
use serde_json::json;
use std::{
    io::Write,
    path::Path,
    process::{Output, Stdio},
};

pub fn offline() {
    let tools = Tools::installed();
    let mut facts = Vec::new();
    for form in [Form::SwiftBuild, Form::SwiftTest] {
        for fail in [false, true] {
            let prepared = prepare(
                &tools.directory,
                &tools,
                &format!("{}-{fail}", form.id()),
                form,
                false,
                fail,
            );
            let output = direct(&tools, &prepared);
            assert_result(&output, form, fail);
            super::streams::validate_form(form, &output.stdout, &output.stderr);
            let record = record(&prepared.project);
            facts.push(
                json!({"form":form.id(),"fail":fail,"exitCode":output.status.code(),
                "record":record,"argv":prepared.argv,"offline":true,"executions":1,
                "nativeChild":super::evidence::observation(&prepared.project),
                "stdinSupplied":INPUT,"privateCacheWritesVerified":true}),
            );
        }
    }
    println!(
        "SWIFT_OFFLINE_READINESS {}",
        json!({"tools":tools.receipt(),"cases":facts})
    );
}

pub fn managed(read: Read) {
    let tools = Tools::installed();
    let receipt = tools.receipt();
    let observers = tools.observers.clone();
    let case = Case::with_tools("swift-semantics", read, false, tools, receipt, observers);
    for form in [Form::SwiftBuild, Form::SwiftTest] {
        for fail in [false, true] {
            let baseline = prepare(
                &case.artifacts,
                &case.tools,
                &format!("direct-{}-{fail}", form.id()),
                form,
                false,
                fail,
            );
            let expected = direct(&case.tools, &baseline);
            assert_result(&expected, form, fail);
            let prepared = prepare(
                &case.artifacts,
                &case.tools,
                &format!("managed-{}-{fail}", form.id()),
                form,
                true,
                fail,
            );
            assert_eq!(baseline.argv[..5], prepared.argv[..5]);
            let argv: Vec<_> = prepared.argv.iter().map(String::as_str).collect();
            let mut run = Run::start_with(&case, &prepared.project, &argv, |command| {
                configure(&prepared.project, command);
                command.stdin(Stdio::piped());
            });
            run.wrapper
                .child
                .stdin
                .take()
                .unwrap()
                .write_all(INPUT)
                .unwrap();
            let entry = entered(&case, &mut run, &prepared);
            let code = i32::from(fail);
            run.finish(code);
            let actual = record(&prepared.project);
            assert_eq!(actual, record(&baseline.project));
            assert_streams(form, &expected, &prepared.project);
            assert_eq!(super::evidence::body_count(&prepared.project), 1);
            let terminal = terminal(&case, &run, form, code);
            case.save(
                &format!("{}-{}", form.id(), if fail { "failure" } else { "success" }),
                json!({"originalArgv":prepared.argv,"baseline":actual,"outcome":terminal,
                    "entry":entry,"nativeChild":super::evidence::observation(&prepared.project),
                    "offline":true,"eachExecutedOnce":true,"originalExecutionSemantics":true}),
            );
        }
    }
    let permits = permit_history(&case, "BUILD", 4, 1);
    case.save("swift-semantics-permits", permits);
    case.done();
}

fn assert_streams(form: Form, expected: &Output, project: &Path) {
    super::streams::compare(
        form,
        [&expected.stdout, &expected.stderr],
        [
            &std::fs::read(project.join("stdout")).unwrap(),
            &std::fs::read(project.join("stderr")).unwrap(),
        ],
    );
}

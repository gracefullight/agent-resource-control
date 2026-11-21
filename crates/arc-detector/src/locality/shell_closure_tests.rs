use super::*;
use crate::{JobClass, Locality, Uncertainty};

#[test]
fn literal_shell_forms_are_contextual_and_do_not_change_protocol_one() {
    let context = EndpointContext {
        captured_at_ms: 1,
        ..EndpointContext::default()
    };
    for (command, expected) in [
        ("bun test", JobClass::Test),
        ("MODE=test cargo test", JobClass::Test),
        ("env cargo test", JobClass::Test),
        ("nice cargo test", JobClass::Test),
        ("timeout 10 cargo build", JobClass::Build),
        ("./검증/cargo build", JobClass::Build),
        ("cd fixtures && cargo build", JobClass::Build),
        ("cargo test | cargo test", JobClass::Test),
    ] {
        for dialect in ["bash", "zsh"] {
            let request = ClassifyRequest {
                command: command.into(),
                shell_kind: dialect.into(),
            };
            let result = classify_with_context(&request, &context, 1);
            assert_eq!(result.classification, expected, "{dialect}: {command}");
            assert_eq!(result.locality, Locality::HostLocal);
            let native = ["sh", "-c", command].map(OsString::from);
            assert_eq!(
                classify_argv_with_context(&native, &context, 1).classification,
                expected,
                "native sh: {command}"
            );
        }
    }
    let request = ClassifyRequest {
        command: "env cargo test".into(),
        shell_kind: "sh".into(),
    };
    assert_eq!(crate::classify(&request).classification, JobClass::Unknown);
}

#[test]
fn assignments_directory_changes_and_wrappers_cannot_borrow_remote_facts() {
    let context = EndpointContext {
        captured_at_ms: 1,
        facts: vec![EndpointFact {
            kind: EndpointKind::CiHost,
            selector: "gh".into(),
            endpoints: vec!["https://github.com".into()],
        }],
        ..EndpointContext::default()
    };
    for command in [
        "GH_HOST=localhost gh workflow run build.yml",
        "env gh workflow run build.yml",
        "nice gh workflow run build.yml",
        "cd fixtures && gh workflow run build.yml",
        "cd \"$TARGET\" && cargo build",
        "cargo test $(printf x)",
        "sudo cargo build",
        "printf x | xargs cargo test",
        "'MODE=test' cargo test",
    ] {
        let request = ClassifyRequest {
            command: command.into(),
            shell_kind: "bash".into(),
        };
        let result = classify_with_context(&request, &context, 1);
        assert_eq!(
            (result.classification, result.locality, result.uncertainty),
            (JobClass::Unknown, Locality::Unknown, Uncertainty::Observe),
            "{command}: {result:?}"
        );
    }
    assert!(capture_shell_argv(&ClassifyRequest {
        command: "cd fixtures && gh workflow run build.yml".into(),
        shell_kind: "bash".into(),
    })
    .is_none());
}

#[test]
fn literal_makefile_test_recipe_is_classified_only_with_matching_caller_evidence() {
    // The caller supplies the complete selected Makefile as transient data;
    // the detector itself never opens a file or executes Make.
    let context: EndpointContext = serde_json::from_value(serde_json::json!({
        "captured_at_ms": 1,
        "facts": [
            {"kind": "MAKEFILE", "selector": "test", "endpoints": ["test:\n\tcargo test\n"]},
            {"kind": "CI_HOST", "selector": "gh", "endpoints": ["https://github.com"]}
        ]
    }))
    .unwrap();
    let direct = ["make", "test"].map(OsString::from);
    assert_eq!(
        classify_argv_with_context(&direct, &context, 1).classification,
        JobClass::Test
    );
    for (source, expected) in [
        ("make test", JobClass::Test),
        ("make test && gh workflow run ci.yml", JobClass::Mixed),
    ] {
        for dialect in ["bash", "zsh"] {
            let request = ClassifyRequest {
                command: source.into(),
                shell_kind: dialect.into(),
            };
            let shell = classify_with_context(&request, &context, 1);
            let native = ["sh", "-c", source].map(OsString::from);
            let execution = classify_argv_with_context(&native, &context, 1);
            for actual in [shell, execution] {
                assert_eq!(actual.classification, expected, "{dialect}: {source}");
                assert_eq!(actual.uncertainty, Uncertainty::None);
                assert!(actual.is_consistent());
            }
        }
    }

    for contents in [
        "test:\n\t$(TEST_RUNNER) test\n",
        "test: dependency\n\tcargo test\n",
        "test:\n\tcargo test\n\tgh workflow run ci.yml\n",
        "include other.mk\ntest:\n\tcargo test\n",
    ] {
        let mut dynamic = context.clone();
        dynamic.facts[0].endpoints[0] = contents.into();
        assert_eq!(
            classify_argv_with_context(&direct, &dynamic, 1).classification,
            JobClass::Unknown,
            "{contents:?}"
        );
        for source in ["make test", "make test && gh workflow run ci.yml"] {
            let request = ClassifyRequest {
                command: source.into(),
                shell_kind: "bash".into(),
            };
            let native = ["sh", "-c", source].map(OsString::from);
            for result in [
                classify_with_context(&request, &dynamic, 1),
                classify_argv_with_context(&native, &dynamic, 1),
            ] {
                assert_eq!(
                    (result.classification, result.locality, result.uncertainty),
                    (JobClass::Unknown, Locality::Unknown, Uncertainty::Observe),
                    "{contents:?}: {source}"
                );
            }
        }
    }
    let mut absent = context.clone();
    absent.facts.remove(0);
    let mut wrong_target = context.clone();
    wrong_target.facts[0].selector = "build".into();
    let mut multiple_files = context.clone();
    multiple_files.facts[0]
        .endpoints
        .push("test:\n\tother\n".into());
    for invalid in [&absent, &wrong_target, &multiple_files] {
        let result = classify_argv_with_context(&direct, invalid, 1);
        assert_eq!(
            (result.classification, result.locality, result.uncertainty),
            (JobClass::Unknown, Locality::Unknown, Uncertainty::Observe)
        );
    }
    assert_eq!(
        classify_argv_with_context(&direct, &context, MAX_CONTEXT_AGE_MS + 2).classification,
        JobClass::Unknown
    );
    for source in [
        "make test",
        "make -f other.mk test",
        "make other",
        "make test OTHER=value",
        "MODE=test make test",
        "cd elsewhere && make test",
        "cd elsewhere && sh -c 'make test'",
        "sh -c 'cd elsewhere && make test'",
    ] {
        let request = ClassifyRequest {
            command: source.into(),
            shell_kind: "bash".into(),
        };
        let proof = if source == "make test" {
            &absent
        } else {
            &context
        };
        let result = classify_with_context(&request, proof, 1);
        assert_eq!(
            (result.classification, result.locality, result.uncertainty),
            (JobClass::Unknown, Locality::Unknown, Uncertainty::Observe),
            "{source}"
        );
    }
}

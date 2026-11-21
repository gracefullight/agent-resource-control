use super::*;
use crate::{JobClass, Locality, Uncertainty};

fn context() -> EndpointContext {
    EndpointContext {
        captured_at_ms: 10_000,
        ..EndpointContext::default()
    }
}

fn shell(command: &str) -> ClassifyRequest {
    ClassifyRequest {
        command: command.into(),
        shell_kind: "sh".into(),
    }
}

fn argv(words: &[&str]) -> Vec<OsString> {
    words.iter().map(OsString::from).collect()
}

#[test]
fn nested_bash_startup_cannot_borrow_caller_endpoint_facts() {
    let mut c = context();
    c.facts.push(EndpointFact {
        kind: EndpointKind::CiHost,
        selector: "gh".into(),
        endpoints: vec!["https://github.com".into()],
    });
    for option in ["-c", "-lc"] {
        let command = format!("bash {option} 'gh workflow run build.yml'");
        for result in [
            classify_with_context(&shell(&command), &c, 10_000),
            classify_argv_with_context(
                &argv(&["bash", option, "gh workflow run build.yml"]),
                &c,
                10_000,
            ),
        ] {
            assert_eq!(
                (result.classification, result.locality, result.uncertainty),
                (JobClass::Unknown, Locality::Unknown, Uncertainty::Observe),
                "{command}"
            );
        }
        assert_eq!(
            capture_shell_argv(&shell(&command)),
            Some(vec![argv(&["bash", option, "gh workflow run build.yml"])])
        );
    }
    // Protocol-1 is a frozen syntax-only contract. The CLI does not trust its
    // confident result without agreement from the guarded contextual route.
    assert_eq!(
        crate::classify(&shell("bash -lc 'cargo test'")).classification,
        JobClass::Test
    );
    for command in [
        "sh -c \"bash -c 'gh workflow run build.yml'\"",
        "cargo build && bash -lc 'gh workflow run build.yml'",
    ] {
        let result = classify_with_context(&shell(command), &c, 10_000);
        assert_eq!(
            (result.classification, result.locality, result.uncertainty),
            (JobClass::Unknown, Locality::Unknown, Uncertainty::Observe),
            "{command}"
        );
    }
    assert_eq!(
        classify_with_context(&shell("sh -c 'gh workflow run build.yml'"), &c, 10_000)
            .classification,
        JobClass::RemoteCi
    );
    assert_eq!(
        classify_with_context(&shell("cargo build"), &c, 10_000).classification,
        JobClass::Build
    );
}

#[test]
fn both_routes_use_unowned_fallback_and_terminal_unknown_for_ambiguous_owned_commands() {
    let c = context();
    assert_eq!(
        classify_with_context(&shell("cargo build"), &c, 10_000).reason,
        Reason::CargoBuild
    );
    assert_eq!(
        classify_argv_with_context(&argv(&["cargo", "build"]), &c, 10_000).reason,
        Reason::CargoBuild
    );
    assert_eq!(
        classify_argv_with_context(&argv(&["go", "build"]), &c, 10_000).reason,
        Reason::GoBuild
    );
    assert_eq!(
        classify_with_context(&shell("sh -c 'cargo test'"), &c, 10_000).reason,
        Reason::CargoTest
    );

    // The legacy syntax-only rule considers this host remote, but an alias has
    // no endpoint evidence. Contextual UNKNOWN must not fall through to it.
    let old = crate::classify(&shell("ssh build01 make test"));
    assert_eq!(old.classification, JobClass::RemoteExec);
    for actual in [
        classify_with_context(&shell("ssh build01 make test"), &c, 10_000),
        classify_argv_with_context(&argv(&["ssh", "build01", "make", "test"]), &c, 10_000),
    ] {
        assert_eq!(
            (actual.classification, actual.locality, actual.uncertainty),
            (JobClass::Unknown, Locality::Unknown, Uncertainty::Observe)
        );
        assert!(actual.is_consistent());
    }
    let compound = classify_with_context(&shell("cargo test && ssh build01 make test"), &c, 10_000);
    assert_eq!(compound.classification, JobClass::Unknown);
    let mut known_ci = c.clone();
    known_ci
        .environment
        .insert("GH_HOST".into(), "github.com".into());
    known_ci.facts.push(EndpointFact {
        kind: EndpointKind::CiHost,
        selector: "gh".into(),
        endpoints: vec!["https://github.com".into()],
    });
    // The same verified repository host is sufficient without GH_HOST.
    let mut repository_ci = known_ci.clone();
    repository_ci.environment.remove("GH_HOST");
    for context in [&known_ci, &repository_ci] {
        for result in [
            classify_with_context(&shell("gh workflow run build.yml"), context, 10_000),
            classify_argv_with_context(
                &argv(&["gh", "workflow", "run", "build.yml"]),
                context,
                10_000,
            ),
        ] {
            assert_eq!(result.classification, JobClass::RemoteCi);
            assert_eq!(result.reason, Reason::GhWorkflowRun);
        }
    }
    assert_eq!(
        classify_with_context(
            &shell("cargo test && gh workflow run build.yml"),
            &known_ci,
            10_000,
        )
        .classification,
        JobClass::Mixed
    );
    // A verified host also supports an explicit owner/repository selection.
    for result in [
        classify_with_context(
            &shell("gh workflow run build.yml --repo example/repo"),
            &known_ci,
            10_000,
        ),
        classify_argv_with_context(
            &argv(&[
                "gh",
                "workflow",
                "run",
                "build.yml",
                "--repo",
                "example/repo",
            ]),
            &known_ci,
            10_000,
        ),
    ] {
        assert_eq!(
            (
                result.classification,
                result.locality,
                result.reason,
                result.uncertainty
            ),
            (
                JobClass::RemoteCi,
                Locality::Remote,
                Reason::GhWorkflowRun,
                Uncertainty::None
            )
        );
    }
    let mut host_only = c.clone();
    host_only
        .environment
        .insert("GH_HOST".into(), "github.com".into());
    let mut conflicting_fact = known_ci.clone();
    conflicting_fact.facts[0].endpoints = vec!["https://localhost".into()];
    let mut conflicting_environment = known_ci.clone();
    conflicting_environment
        .environment
        .insert("GH_HOST".into(), "localhost".into());
    let mut multiple_hosts = known_ci.clone();
    multiple_hosts.facts[0]
        .endpoints
        .push("https://localhost".into());
    let mut repo_override = known_ci.clone();
    repo_override
        .environment
        .insert("GH_REPO".into(), "localhost/org/repo".into());
    // A repo option is not enough when its effective host remains ambiguous.
    for context in [
        &host_only,
        &conflicting_fact,
        &conflicting_environment,
        &multiple_hosts,
    ] {
        for result in [
            classify_with_context(
                &shell("gh workflow run build.yml --repo example/repo"),
                context,
                10_000,
            ),
            classify_argv_with_context(
                &argv(&[
                    "gh",
                    "workflow",
                    "run",
                    "build.yml",
                    "--repo",
                    "example/repo",
                ]),
                context,
                10_000,
            ),
        ] {
            assert_eq!(
                (result.classification, result.locality, result.uncertainty),
                (JobClass::Unknown, Locality::Unknown, Uncertainty::Observe)
            );
        }
    }
    assert_eq!(
        crate::classify(&shell("gh workflow run build.yml")).classification,
        JobClass::RemoteCi
    );
    for context in [
        host_only,
        conflicting_fact,
        conflicting_environment,
        multiple_hosts,
        repo_override,
    ] {
        for result in [
            classify_with_context(&shell("gh workflow run build.yml"), &context, 10_000),
            classify_argv_with_context(
                &argv(&["gh", "workflow", "run", "build.yml"]),
                &context,
                10_000,
            ),
        ] {
            assert_eq!(
                (result.classification, result.uncertainty),
                (JobClass::Unknown, Uncertainty::Observe)
            );
        }
    }
    assert_eq!(
        classify_argv_with_context(&argv(&["custom-program"]), &c, 10_000).classification,
        JobClass::Unknown
    );
}

#[test]
fn context_validation_checks_age_limits_keys_and_duplicate_evidence_before_classification() {
    let base = context();
    assert!(base.is_valid_at(15_000));
    assert!(!base.is_valid_at(15_001));
    assert!(!base.is_valid_at(9_999));
    let mut bad_key = base.clone();
    bad_key.environment.insert("TOKEN".into(), "secret".into());
    let mut too_big = base.clone();
    too_big.environment.insert(
        "DOCKER_HOST".into(),
        "x".repeat(MAX_CONTEXT_VALUE_BYTES + 1),
    );
    let mut duplicate = base.clone();
    duplicate.facts = vec![
        EndpointFact {
            kind: EndpointKind::DockerContext,
            selector: "prod".into(),
            endpoints: vec!["tcp://192.0.2.1:2376".into()],
        },
        EndpointFact {
            kind: EndpointKind::DockerContext,
            selector: "prod".into(),
            endpoints: vec!["unix:///var/run/docker.sock".into()],
        },
    ];
    let mut excessive = base.clone();
    excessive.facts.push(EndpointFact {
        kind: EndpointKind::DockerBuilder,
        selector: "builder".into(),
        endpoints: vec!["x".into(); MAX_CONTEXT_ENTRIES],
    });
    for c in [bad_key, too_big, duplicate, excessive] {
        assert!(!c.is_valid_at(10_000));
        let result = classify_argv_with_context(&argv(&["cargo", "build"]), &c, 10_000);
        assert_eq!(result.classification, JobClass::Unknown);
        assert_eq!(result.uncertainty, Uncertainty::Observe);
    }
    assert_eq!(
        classify_with_context(&shell("cargo build"), &base, 15_001).classification,
        JobClass::Unknown
    );
}

#[test]
fn new_vocabulary_is_consistent_and_conservatively_projects_to_protocol_one() {
    for (class, reason) in [
        (JobClass::RemoteCi, Reason::RemoteCiDispatch),
        (JobClass::RemoteExec, Reason::RemoteEndpointExec),
        (JobClass::Build, Reason::LocalEndpointBuild),
        (JobClass::Unknown, Reason::EndpointAmbiguous),
    ] {
        let result = Classification::new(class, reason);
        assert!(result.is_consistent());
        assert!(result.is_endpoint_locality());
        assert_eq!(
            result.legacy(),
            Classification::unknown(Reason::UnresolvedCommand)
        );
        let forged = Classification {
            classification: JobClass::Test,
            ..result
        };
        assert!(!forged.is_consistent());
    }
}

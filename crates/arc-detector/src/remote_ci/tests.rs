use super::*;
use crate::{ClassifyRequest, EndpointFact, Locality, Uncertainty};
use std::ffi::OsString;

const NOW: u64 = 10_000;
const HTTPS: &str = "https://github.com/org/repo.git";
const SSH: &str = "ssh://git@github.com/org/repo.git";

fn context(kind: EndpointKind, selector: &str, endpoint: &str) -> EndpointContext {
    EndpointContext {
        captured_at_ms: NOW,
        facts: vec![EndpointFact {
            kind,
            selector: selector.into(),
            endpoints: vec![endpoint.into()],
        }],
        ..EndpointContext::default()
    }
}

fn routes(command: &str, context: &EndpointContext, now: u64) -> [Classification; 2] {
    let request = ClassifyRequest {
        command: command.into(),
        shell_kind: "sh".into(),
    };
    let argv: Vec<OsString> = command.split_whitespace().map(Into::into).collect();
    [
        crate::classify_with_context(&request, context, now),
        crate::classify_argv_with_context(&argv, context, now),
    ]
}

#[test]
fn named_bare_push_remote_local_and_ambiguous_controls_on_both_routes() {
    let remote = context(EndpointKind::GitPushDefault, "@bare-push", HTTPS);
    let local = context(
        EndpointKind::GitPushDefault,
        "@bare-push",
        "file:///fixture/repo",
    );
    let mut ambiguous = remote.clone();
    ambiguous.facts[0]
        .endpoints
        .push("file:///fixture/repo".into());
    for (name, facts, expected) in [
        ("bare_remote", &remote, JobClass::RemoteCi),
        ("bare_local", &local, JobClass::Unknown),
        (
            "bare_ambiguous_multiple_destinations",
            &ambiguous,
            JobClass::Unknown,
        ),
    ] {
        for result in routes("git push", facts, NOW) {
            assert_eq!(result.classification, expected, "{name}");
            assert_ne!(result.locality, Locality::HostLocal, "{name}");
            if expected == JobClass::Unknown {
                assert_eq!(result.uncertainty, Uncertainty::Observe, "{name}");
            }
        }
    }
    for result in routes("git -c remote.pushDefault=origin push", &remote, NOW) {
        assert_eq!(result.classification, JobClass::RemoteCi);
    }
    for result in routes("git push", &remote, NOW + 5_001) {
        assert_eq!(
            (result.classification, result.uncertainty),
            (JobClass::Unknown, Uncertainty::Observe)
        );
    }
}

#[test]
fn options_endpoints_and_ambiguous_or_mixed_controls_are_conservative() {
    let gh = context(EndpointKind::CiHost, "gh", "https://github.com");
    let mut override_repo = gh.clone();
    override_repo
        .environment
        .insert("GH_HOST".into(), "localhost".into());
    override_repo
        .environment
        .insert("GH_REPO".into(), "localhost/a/b".into());
    for actual in routes(
        "gh workflow run build.yml --repo github.com/org/repo",
        &override_repo,
        NOW,
    ) {
        assert_eq!(actual.classification, JobClass::RemoteCi);
    }
    let mut override_env_repo = gh.clone();
    override_env_repo
        .environment
        .insert("GH_REPO".into(), "localhost/a/b".into());
    for actual in routes(
        "gh workflow run build.yml --repo org/repo",
        &override_env_repo,
        NOW,
    ) {
        assert_eq!(actual.classification, JobClass::RemoteCi);
    }
    let mut host_only = EndpointContext {
        captured_at_ms: NOW,
        ..EndpointContext::default()
    };
    host_only
        .environment
        .insert("GH_HOST".into(), "github.com".into());
    let local_git = context(EndpointKind::GitRemote, "origin", "file:///tmp/repo");
    let mut local_ssh = context(EndpointKind::GitRemote, SSH, SSH);
    local_ssh.facts.push(EndpointFact {
        kind: EndpointKind::SshDestination,
        selector: "github.com".into(),
        endpoints: vec!["ssh://127.0.0.1:22".into()],
    });
    let multi_git = EndpointContext {
        facts: vec![EndpointFact {
            kind: EndpointKind::GitRemote,
            selector: "origin".into(),
            endpoints: vec![HTTPS.into(), "file:///tmp/repo".into()],
        }],
        ..gh.clone()
    };
    for (command, context) in [
        ("gh workflow run build.yml", &host_only),
        ("gh workflow run build.yml", &override_repo),
        ("gh workflow run build.yml --repo localhost/org/repo", &gh),
        (
            "gh workflow run build.yml --repo org/repo --repo org/other",
            &gh,
        ),
        ("gh workflow run build.yml --mystery value", &gh),
        ("gh workflow list --workflow build.yml", &gh),
        ("/tmp/gh workflow run build.yml", &gh),
        ("git push origin main", &local_git),
        ("git push origin main", &multi_git),
        ("git push https://github.com/org/repo.git main", &gh),
        (
            "git push ssh://git@github.com/org/repo.git main",
            &local_ssh,
        ),
        ("git push origin main --receive-pack custom", &gh),
        ("gitlab pipeline run --branch main", &gh),
    ] {
        for result in routes(command, context, NOW) {
            assert_eq!(
                (result.classification, result.locality, result.uncertainty),
                (JobClass::Unknown, Locality::Unknown, Uncertainty::Observe),
                "{command}: {result:?}"
            );
        }
    }
    for result in routes("gh workflow run build.yml", &gh, NOW + 5_001) {
        assert_eq!(result.classification, JobClass::Unknown);
        assert_eq!(result.uncertainty, Uncertainty::Observe);
    }
    for result in routes("cargo build", &gh, NOW) {
        assert_eq!(
            (result.classification, result.locality),
            (JobClass::Build, Locality::HostLocal)
        );
    }
    let mixed = crate::classify_with_context(
        &ClassifyRequest {
            command: "cargo test && gh workflow run build.yml".into(),
            shell_kind: "sh".into(),
        },
        &gh,
        NOW,
    );
    assert_eq!(
        (mixed.classification, mixed.locality),
        (JobClass::Mixed, Locality::Mixed)
    );
    let mixed_unknown = crate::classify_with_context(
        &ClassifyRequest {
            command: "cargo test && gh workflow run build.yml".into(),
            shell_kind: "sh".into(),
        },
        &host_only,
        NOW,
    );
    assert_eq!(
        (mixed_unknown.classification, mixed_unknown.uncertainty),
        (JobClass::Unknown, Uncertainty::Observe)
    );
}

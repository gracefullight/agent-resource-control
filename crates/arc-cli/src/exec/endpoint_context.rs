//! Fresh, bounded endpoint evidence from the caller's environment.
use arc_detector::{EndpointContext, CONTEXT_ENV_KEYS, MAX_CONTEXT_BYTES, MAX_CONTEXT_VALUE_BYTES};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    path::Path,
    sync::mpsc,
    time::{Duration, Instant},
};

mod docker;
mod files;
mod interfaces;
mod kube;
mod ssh;

pub(super) fn capture(argv: &[OsString]) -> Option<EndpointContext> {
    match within_input_budget(argv, || Some(capture_valid_input(argv))) {
        Some(context) => context,
        None => Some(EndpointContext {
            captured_at_ms: arc_ipc::locality::now_ms()?,
            ..EndpointContext::default()
        }),
    }
}

// Skip discovery for oversized inputs before collecting the environment,
// allocating a copy of argv, or opening any endpoint configuration. The empty
// context still lets the detector report its INPUT_LIMIT or TOKEN_LIMIT.
fn within_input_budget<T>(argv: &[OsString], work: impl FnOnce() -> Option<T>) -> Option<T> {
    if argv.len() > arc_detector::MAX_TOKENS
        || argv.iter().try_fold(0usize, |bytes, arg| {
            bytes.checked_add(arg.as_encoded_bytes().len())
        })? > arc_detector::MAX_INPUT_BYTES
    {
        return None;
    }
    work()
}

fn capture_valid_input(argv: &[OsString]) -> Option<EndpointContext> {
    let started = Instant::now();
    let mut context = capture_with(argv, arc_ipc::locality::now_ms()?, |key| {
        std::env::var_os(key)
    })?;
    let words: Vec<String> = argv
        .iter()
        .map(|arg| arg.to_str().map(str::to_owned))
        .collect::<Option<_>>()?;
    if matches!(
        words.first().map(String::as_str),
        Some("ssh" | "docker" | "kubectl" | "act")
    ) {
        // A regular-file read can still block on a slow filesystem despite
        // O_NONBLOCK. One-shot discovery is bounded by the freshness window.
        let remaining = Duration::from_millis(arc_detector::MAX_CONTEXT_AGE_MS)
            .checked_sub(started.elapsed())?;
        context = bounded(remaining, move || {
            let mut reads = files::Reads::new();
            let result = match words[0].as_str() {
                "ssh" => ssh::capture(&words[1..], &mut context, &mut reads),
                "docker" => docker::capture(&words[1..], &mut context, &mut reads),
                "kubectl" => kube::capture(&words[1..], &mut context, &mut reads),
                "act" => capture_act(&words[1..], &mut context, &mut reads),
                _ => unreachable!(),
            };
            // Missing or unsupported data leave no usable facts.
            if result.is_none() {
                context.facts.clear();
            }
            context
        })?;
    }
    context
        .is_valid_at(arc_ipc::locality::now_ms()?)
        .then_some(context)
}

pub(super) fn bounded<T: Send + 'static>(
    limit: Duration,
    work: impl FnOnce() -> T + Send + 'static,
) -> Option<T> {
    let (send, receive) = mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("arc-endpoint-capture".into())
        .spawn(move || {
            let _ = send.send(work());
        })
        .ok()?;
    receive.recv_timeout(limit).ok()
}

/// Bind selected Git-over-SSH hosts only after the Git lane resolves its URLs.
pub(super) fn finish(_: &[OsString], context: &mut EndpointContext) -> Option<()> {
    if context.environment.contains_key("GIT_SSH")
        || context.environment.contains_key("GIT_SSH_COMMAND")
    {
        return Some(());
    }
    let hosts = git_ssh_hosts(context);
    if hosts.is_empty() {
        return Some(());
    }
    let age = arc_ipc::locality::now_ms()?.checked_sub(context.captured_at_ms)?;
    let remaining = arc_detector::MAX_CONTEXT_AGE_MS.checked_sub(age)?;
    let mut snapshot = context.clone();
    let resolved = bounded(Duration::from_millis(remaining), move || {
        let mut reads = files::Reads::new();
        for host in hosts {
            ssh::bind_git(&host, &mut snapshot, &mut reads)?;
        }
        Some(snapshot)
    })?;
    if let Some(resolved) = resolved {
        *context = resolved;
    }
    Some(())
}

fn git_ssh_hosts(context: &EndpointContext) -> std::collections::BTreeSet<String> {
    use arc_detector::EndpointKind;
    let mut hosts = std::collections::BTreeSet::new();
    for item in &context.facts {
        if !matches!(
            item.kind,
            EndpointKind::GitRemote | EndpointKind::GitPushDefault
        ) {
            continue;
        }
        for endpoint in &item.endpoints {
            let Some(rest) = endpoint.strip_prefix("ssh://git@") else {
                continue;
            };
            let Some((host, _)) = rest.split_once('/') else {
                continue;
            };
            if matches!(host, "github.com" | "gitlab.com")
                && !context
                    .facts
                    .iter()
                    .any(|fact| fact.kind == EndpointKind::SshDestination && fact.selector == host)
            {
                hosts.insert(host.to_owned());
            }
        }
    }
    hosts
}

fn capture_with(
    argv: &[OsString],
    captured_at_ms: u64,
    mut get: impl FnMut(&str) -> Option<OsString>,
) -> Option<EndpointContext> {
    let mut environment = BTreeMap::new();
    let mut bytes = 0usize;
    let directly_local =
        arc_detector::classify_argv(argv).locality == arc_detector::Locality::HostLocal;
    for &key in CONTEXT_ENV_KEYS {
        if directly_local || !relevant(argv, key) {
            continue;
        }
        if let Some(value) = get(key) {
            if value.as_encoded_bytes().len() > MAX_CONTEXT_VALUE_BYTES {
                return None;
            }
            let value = value.into_string().ok()?;
            bytes = bytes.checked_add(key.len() + value.len())?;
            if bytes > MAX_CONTEXT_BYTES {
                return None;
            }
            environment.insert(key.to_owned(), value);
        }
    }
    let context = EndpointContext {
        captured_at_ms,
        environment,
        facts: Vec::new(),
    };
    context
        .is_valid_at(context.captured_at_ms)
        .then_some(context)
}

fn relevant(argv: &[OsString], key: &str) -> bool {
    let program = argv
        .first()
        .and_then(|arg| Path::new(arg).file_name())
        .and_then(|name| name.to_str());
    match program {
        // Package scripts can contain any of the endpoint-dependent commands.
        Some("bun" | "pnpm") => true,
        Some("gh" | "git" | "gitlab" | "glab") => {
            key.starts_with("GH_")
                || key.starts_with("GIT_")
                || key.starts_with("GITLAB_")
                || key.starts_with("GLAB_")
        }
        Some("docker" | "docker-compose" | "act") => {
            key.starts_with("DOCKER_") || key.starts_with("BUILDX_")
        }
        Some("kubectl") => key == "KUBECONFIG",
        _ => false,
    }
}

fn capture_act(
    args: &[String],
    context: &mut EndpointContext,
    reads: &mut files::Reads,
) -> Option<()> {
    if args != ["-j", "build"]
        || context.environment.contains_key("DOCKER_CONTEXT")
        || std::env::vars_os().any(|(key, _)| key.to_string_lossy().starts_with("ACT_"))
    {
        return None;
    }
    let host = context.environment.get("DOCKER_HOST")?.clone();
    if !host.starts_with("unix:///") {
        return None;
    }
    let home = files::path_env("HOME")??;
    let cwd = std::env::current_dir().ok()?;
    let xdg = files::path_env("XDG_CONFIG_HOME")?.unwrap_or_else(|| home.join(".config"));
    for path in [
        cwd.join(".actrc"),
        home.join(".actrc"),
        xdg.join("act/actrc"),
    ] {
        if reads.read(&path)?.is_some() {
            return None;
        }
    }
    docker::capture(&["build".into(), ".".into()], context, reads)?;
    if !context.facts.iter().any(|fact| {
        fact.kind == arc_detector::EndpointKind::DockerContext
            && fact.selector.starts_with("@local-proof/")
            && fact.endpoints.as_slice() == [host.as_str()]
    }) {
        return None;
    }
    files::fact(context, arc_detector::EndpointKind::ActLocal, "act", host)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::os::unix::ffi::OsStringExt;

    #[test]
    fn input_limits_precede_all_discovery() {
        let calls = Cell::new(0);
        let run = |argv: &[OsString]| {
            within_input_budget(argv, || {
                calls.set(calls.get() + 1);
                Some(())
            })
        };
        let bytes_at_limit = [OsString::from("x".repeat(arc_detector::MAX_INPUT_BYTES))];
        assert!(run(&bytes_at_limit).is_some());
        let bytes_over_limit = [
            OsString::from("docker"),
            OsString::from("x".repeat(arc_detector::MAX_INPUT_BYTES)),
        ];
        assert!(run(&bytes_over_limit).is_none());
        let tokens_at_limit = vec![OsString::from("x"); arc_detector::MAX_TOKENS];
        assert!(run(&tokens_at_limit).is_some());
        let mut tokens_over_limit = vec![OsString::from("x"); arc_detector::MAX_TOKENS + 1];
        tokens_over_limit[0] = OsString::from("docker");
        assert!(run(&tokens_over_limit).is_none());
        assert_eq!(
            calls.get(),
            2,
            "over-budget inputs must not reach discovery"
        );
        for (argv, reason) in [
            (
                bytes_over_limit.as_slice(),
                arc_detector::Reason::InputLimit,
            ),
            (
                tokens_over_limit.as_slice(),
                arc_detector::Reason::TokenLimit,
            ),
        ] {
            for program in ["docker", "ssh", "kubectl", "git", "gh", "bun", "fixture"] {
                let mut invocation = argv.to_vec();
                invocation[0] = program.into();
                let context =
                    capture(&invocation).expect("over-budget input retains an empty context");
                assert!(context.environment.is_empty(), "{program}");
                assert!(context.facts.is_empty(), "{program}");
                let result = super::super::metadata::classify(&invocation);
                assert_eq!(
                    result.classification,
                    arc_detector::JobClass::Unknown,
                    "{program}"
                );
                assert_eq!(
                    result.locality,
                    arc_detector::Locality::Unknown,
                    "{program}"
                );
                assert_eq!(
                    result.uncertainty,
                    arc_detector::Uncertainty::Observe,
                    "{program}"
                );
                assert_eq!(result.reason, reason, "{program}");
            }
        }
    }

    #[test]
    fn unrelated_invalid_values_do_not_affect_local_commands() {
        for value in [
            OsString::from("x".repeat(MAX_CONTEXT_VALUE_BYTES + 1)),
            OsString::from_vec(vec![0xff]),
        ] {
            let cargo = ["cargo".into(), "build".into()];
            let context = capture_with(&cargo, 10_000, |key| {
                (key == "KUBECONFIG").then(|| value.clone())
            })
            .unwrap();
            let result = arc_detector::classify_argv_with_context(&cargo, &context, 10_000);
            assert_eq!(result.reason, arc_detector::Reason::CargoBuild);

            let kubectl = ["kubectl".into(), "exec".into(), "pod".into()];
            assert!(capture_with(&kubectl, 10_000, |key| {
                (key == "KUBECONFIG").then(|| value.clone())
            })
            .is_none());
        }
    }

    #[test]
    fn a_stalled_capture_does_not_supply_facts() {
        let (release, wait) = std::sync::mpsc::channel::<()>();
        assert!(bounded(Duration::from_millis(10), move || wait.recv()).is_none());
        release.send(()).unwrap();
    }

    #[test]
    fn git_post_capture_selects_public_ssh_hosts_and_retains_existing_facts() {
        use arc_detector::{EndpointFact, EndpointKind};
        let mut context = EndpointContext {
            captured_at_ms: 10_000,
            facts: vec![
                EndpointFact {
                    kind: EndpointKind::GitRemote,
                    selector: "origin".into(),
                    endpoints: vec!["ssh://git@github.com/org/repo.git".into()],
                },
                EndpointFact {
                    kind: EndpointKind::GitPushDefault,
                    selector: "@bare-push".into(),
                    endpoints: vec!["ssh://git@gitlab.com/org/repo.git".into()],
                },
            ],
            ..Default::default()
        };
        assert_eq!(
            git_ssh_hosts(&context).into_iter().collect::<Vec<_>>(),
            ["github.com", "gitlab.com"]
        );
        context.facts.push(EndpointFact {
            kind: EndpointKind::SshDestination,
            selector: "github.com".into(),
            endpoints: vec!["ssh://192.0.2.24:22".into()],
        });
        assert_eq!(
            git_ssh_hosts(&context).into_iter().collect::<Vec<_>>(),
            ["gitlab.com"]
        );
        context
            .environment
            .insert("GIT_SSH_COMMAND".into(), "custom".into());
        assert!(finish(&[], &mut context).is_some());
        assert_eq!(context.facts.len(), 3);
    }
}

#[cfg(test)]
mod route_tests;

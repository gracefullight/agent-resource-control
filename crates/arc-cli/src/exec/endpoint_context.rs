//! Fresh caller environment. Task-3 owns the bounded endpoint evidence port.
use arc_detector::{EndpointContext, CONTEXT_ENV_KEYS, MAX_CONTEXT_BYTES, MAX_CONTEXT_VALUE_BYTES};
use std::{collections::BTreeMap, ffi::OsString, path::Path};

pub(super) fn capture(argv: &[OsString]) -> Option<EndpointContext> {
    capture_with(argv, arc_ipc::locality::now_ms()?, |key| {
        std::env::var_os(key)
    })
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
        Some("bun") => true,
        Some("gh" | "git" | "gitlab" | "glab") => {
            key.starts_with("GH_")
                || key.starts_with("GIT_")
                || key.starts_with("GITLAB_")
                || key.starts_with("GLAB_")
        }
        Some("docker" | "docker-compose") => {
            key.starts_with("DOCKER_") || key.starts_with("BUILDX_")
        }
        Some("kubectl") => key == "KUBECONFIG",
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::ffi::OsStringExt;

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
}

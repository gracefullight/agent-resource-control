//! Capture kubeconfig selection without probing the cluster or inferring placement.
use super::files::{fact, path_env, Reads};
use arc_detector::{EndpointContext, EndpointKind};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

#[derive(Deserialize, Default)]
struct Config {
    #[serde(rename = "current-context")]
    current: Option<String>,
    #[serde(default)]
    contexts: Vec<NamedContext>,
    #[serde(default)]
    clusters: Vec<NamedCluster>,
}
#[derive(Deserialize)]
struct NamedContext {
    name: String,
    context: Context,
}
#[derive(Deserialize)]
struct Context {
    cluster: String,
    #[serde(default)]
    namespace: Option<String>,
}
#[derive(Deserialize)]
struct NamedCluster {
    name: String,
    cluster: Cluster,
}
#[derive(Deserialize)]
struct Cluster {
    server: String,
}

pub(super) fn capture(
    args: &[String],
    context: &mut EndpointContext,
    reads: &mut Reads,
) -> Option<()> {
    let (mut selected, mut config, mut namespace, mut i) = (None, None, None, 0);
    while let Some(arg) = args.get(i) {
        if arg == "--" {
            break;
        }
        // These options override the cluster or its server, so a fact for
        // the context's configured server would describe the wrong target.
        // Until those overrides are supported, do not read or emit a fact.
        if arg == "--cluster"
            || arg.starts_with("--cluster=")
            || arg == "--server"
            || arg.starts_with("--server=")
            || arg == "-s"
            || (arg.starts_with("-s") && !arg.starts_with("--"))
            || arg == "-A"
            || arg == "--all-namespaces"
            || arg.starts_with("--all-namespaces=")
        {
            return None;
        }
        let mut found = None;
        for name in ["--context", "--kubeconfig", "--namespace", "-n"] {
            if arg == name {
                i += 1;
                found = Some((name, args.get(i)?.as_str()));
                break;
            }
            if let Some(v) = arg.strip_prefix(name).and_then(|s| s.strip_prefix('=')) {
                found = Some((name, v));
                break;
            }
            if name == "-n" && arg.starts_with("-n") && arg.len() > 2 {
                found = Some((name, &arg[2..]));
                break;
            }
        }
        if let Some((name, value)) = found {
            if value.is_empty() {
                return None;
            }
            let slot = match name {
                "--context" => &mut selected,
                "--kubeconfig" => &mut config,
                _ => &mut namespace,
            };
            if slot.replace(value).is_some() {
                return None;
            }
        }
        i += 1;
    }
    let paths: Vec<PathBuf> = if let Some(file) = config {
        vec![file.into()]
    } else if let Some(value) = std::env::var_os("KUBECONFIG").filter(|v| !v.is_empty()) {
        if value.as_encoded_bytes().len() > arc_detector::MAX_CONTEXT_VALUE_BYTES {
            return None;
        }
        std::env::split_paths(&value)
            .filter(|p| !p.as_os_str().is_empty())
            .collect()
    } else {
        vec![path_env("HOME")??.join(".kube/config")]
    };
    if paths.is_empty() || paths.len() > 8 {
        return None;
    }
    resolve(paths, selected, namespace, context, reads)
}

fn resolve(
    paths: Vec<PathBuf>,
    selected: Option<&str>,
    namespace_override: Option<&str>,
    context: &mut EndpointContext,
    reads: &mut Reads,
) -> Option<()> {
    let mut current = None;
    let mut contexts = BTreeMap::new();
    let mut clusters = BTreeMap::new();
    for path in paths {
        let Some(bytes) = reads.read(&path)? else {
            continue;
        };
        let input = std::str::from_utf8(&bytes).ok()?;
        // Aliases, tags, merges and multi-document streams require expansion
        // semantics outside this bounded capture contract.
        if input.contains(['&', '*', '!'])
            || input.contains("<<:")
            || input.lines().filter(|l| l.trim() == "---").count() > 1
            || input.lines().any(|l| l.trim() == "...")
        {
            return None;
        }
        let config: Config = serde_norway::from_slice(&bytes).ok()?;
        if current.is_none() {
            current = config.current.filter(|s| !s.is_empty());
        }
        let mut local = BTreeSet::new();
        for item in config.contexts {
            if item.name.is_empty() || !local.insert(item.name.clone()) {
                return None;
            }
            contexts.entry(item.name).or_insert(item.context);
        }
        local.clear();
        for item in config.clusters {
            if item.name.is_empty() || !local.insert(item.name.clone()) {
                return None;
            }
            clusters.entry(item.name).or_insert(item.cluster.server);
        }
    }
    let name = selected.or(current.as_deref())?;
    let selection = contexts.get(name)?;
    let server = clusters.get(&selection.cluster)?;
    let namespace = namespace_override
        .or(selection.namespace.as_deref())
        .filter(|value| !value.is_empty())
        .unwrap_or("default");
    if !valid_name(namespace) || selection.cluster.is_empty() {
        return None;
    }
    // Each metadata selector is tied to the same concrete API endpoint as the
    // ordinary context fact. None asserts where a pod executes.
    let metadata = [
        format!("@selected-context/{name}"),
        format!("@selected-cluster/{}", selection.cluster),
        format!("@selected-namespace/{namespace}"),
    ];
    if metadata
        .iter()
        .any(|item| item == selected.unwrap_or("@current"))
    {
        return None;
    }
    fact(
        context,
        EndpointKind::KubernetesContext,
        selected.unwrap_or("@current"),
        server.clone(),
    )?;
    for selector in metadata {
        fact(
            context,
            EndpointKind::KubernetesContext,
            &selector,
            server.clone(),
        )?;
    }
    Some(())
}

fn valid_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 253
        && !value.starts_with('-')
        && value
            .bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, b'.' | b'-'))
}

#[cfg(test)]
mod tests;

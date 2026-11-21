//! Docker context and Buildx store discovery without calling either tool.
use super::{
    files::{fact, path_env, Reads},
    interfaces,
};
use arc_detector::{EndpointContext, EndpointKind};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

mod socket;

#[derive(Deserialize, Default)]
struct Config {
    #[serde(rename = "currentContext")]
    current: Option<String>,
}
#[derive(Deserialize)]
struct Meta {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Endpoints")]
    endpoints: Endpoints,
}
#[derive(Deserialize)]
struct Endpoints {
    docker: DockerEndpoint,
}
#[derive(Deserialize)]
struct DockerEndpoint {
    #[serde(rename = "Host")]
    host: String,
}
#[derive(Deserialize)]
struct Current {
    #[serde(rename = "Key", default)]
    key: String,
    #[serde(rename = "Name", default)]
    name: String,
    #[serde(rename = "Global", default)]
    global: bool,
}
#[derive(Deserialize)]
struct Group {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Driver")]
    driver: String,
    #[serde(rename = "Dynamic", default)]
    dynamic: bool,
    #[serde(rename = "Nodes")]
    nodes: Vec<Node>,
}
#[derive(Deserialize)]
struct Node {
    #[serde(rename = "Endpoint")]
    endpoint: String,
}

fn opt<'a>(args: &'a [String], i: &mut usize, names: &[&str]) -> Option<Option<&'a str>> {
    let arg = args.get(*i)?;
    for name in names {
        if arg == name {
            *i += 1;
            return Some(Some(args.get(*i)?.as_str()));
        }
        if let Some(v) = arg.strip_prefix(name).and_then(|v| v.strip_prefix('=')) {
            return Some(Some(v));
        }
    }
    Some(None)
}
fn set<'a>(slot: &mut Option<&'a str>, value: &'a str) -> Option<()> {
    if value.is_empty() || slot.replace(value).is_some() {
        None
    } else {
        Some(())
    }
}
fn hash(s: &str) -> String {
    Sha256::digest(s.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn name(s: &str) -> bool {
    !s.is_empty()
        && s != "."
        && s != ".."
        && s.bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-' | b'.'))
}

pub(super) fn capture(
    args: &[String],
    context: &mut EndpointContext,
    reads: &mut Reads,
) -> Option<()> {
    let (mut i, mut cli_context, mut cli_host, mut config_dir) = (0, None, None, None);
    while let Some(arg) = args.get(i) {
        if let Some(v) = opt(args, &mut i, &["--context", "-c"])? {
            set(&mut cli_context, v)?;
        } else if let Some(v) = opt(args, &mut i, &["--host", "-H"])? {
            set(&mut cli_host, v)?;
        } else if let Some(v) = opt(args, &mut i, &["--config"])? {
            set(&mut config_dir, v)?;
        } else if arg.starts_with('-') {
            return None;
        } else {
            break;
        }
        i += 1;
    }
    if cli_context.is_some() && cli_host.is_some() {
        return None;
    }
    let is_buildx = args.get(i)? == "buildx";
    if is_buildx {
        i += 1;
    }
    let mut cli_builder = None;
    if is_buildx && args.get(i).is_some_and(|s| s.starts_with("--builder")) {
        let v = opt(args, &mut i, &["--builder"])??;
        set(&mut cli_builder, v)?;
        i += 1;
    }
    if args.get(i)? != "build" {
        return None;
    }
    i += 1;
    while i < args.len() {
        if args[i] == "--" {
            break;
        }
        if let Some(v) = opt(args, &mut i, &["--builder"])? {
            set(&mut cli_builder, v)?;
        }
        i += 1;
    }
    let dir: PathBuf = if let Some(path) = config_dir {
        path.into()
    } else if let Some(path) = path_env("DOCKER_CONFIG")? {
        path
    } else {
        path_env("HOME")??.join(".docker")
    };
    let config: Config = match reads.read(&dir.join("config.json"))? {
        Some(bytes) => serde_json::from_slice(&bytes).ok()?,
        None => Config::default(),
    };
    let env_context_owned = context
        .environment
        .get("DOCKER_CONTEXT")
        .filter(|v| !v.is_empty())
        .cloned();
    let env_host_owned = context
        .environment
        .get("DOCKER_HOST")
        .filter(|v| !v.is_empty())
        .cloned();
    let env_context = env_context_owned.as_deref();
    let env_host = env_host_owned.as_deref();
    let context_name = if let Some(name) = cli_context {
        name
    } else if cli_host.is_some() || env_host.is_some() {
        "default"
    } else {
        env_context
            .or(config.current.as_deref().filter(|v| !v.is_empty()))
            .unwrap_or("default")
    };
    let host = if context_name == "default" {
        cli_host
            .or(env_host)
            .unwrap_or("unix:///var/run/docker.sock")
            .to_owned()
    } else {
        context_host(&dir, context_name, reads)?
    };
    // Match the detector's effective selector, which is also a fact key.
    let selector = cli_context
        .or(cli_host)
        .or(env_host)
        .or(env_context)
        .unwrap_or("@current");
    fact(context, EndpointKind::DockerContext, selector, host.clone())?;
    let env_builder_owned = context
        .environment
        .get("BUILDX_BUILDER")
        .filter(|v| !v.is_empty())
        .cloned();
    let builder = cli_builder.or(env_builder_owned.as_deref());
    let builder_dir = path_env("BUILDX_CONFIG")?.unwrap_or_else(|| dir.join("buildx"));
    let key = if context_name == "default" {
        host.as_str()
    } else {
        context_name
    };
    let effective = if let Some(builder) = builder {
        Some(builder.to_owned())
    } else {
        selected_builder(&builder_dir, key, reads)?
    };
    let endpoint = match effective.as_deref() {
        Some(selected) => builder_endpoint(
            &dir,
            &builder_dir,
            selected,
            context_name,
            &host,
            builder.is_some(),
            reads,
        )?,
        None => host.clone(),
    };
    // Docker's ssh transport invokes OpenSSH and can be redirected by its
    // independent configuration. A URI alone cannot prove its compute host.
    if host.starts_with("ssh://") || endpoint.starts_with("ssh://") {
        return None;
    }
    let builder_selector = if is_buildx && builder.is_none() {
        format!("@selected/{selector}")
    } else {
        builder.unwrap_or(selector).to_owned()
    };
    fact(
        context,
        EndpointKind::DockerBuilder,
        &builder_selector,
        endpoint.clone(),
    )?;
    // Inspect the listener through OS process/descriptor tables; never
    // connect to the endpoint. An unproved or ambiguous listener stays unknown.
    for (index, value) in [host.as_str(), endpoint.as_str()].into_iter().enumerate() {
        if index == 1 && endpoint == host {
            continue;
        }
        if socket::is_local_engine(value) {
            fact(
                context,
                EndpointKind::DockerContext,
                &format!("@local-proof/{index}"),
                value.to_owned(),
            )?;
        }
    }
    if [endpoint.as_str(), host.as_str()]
        .iter()
        .any(|v| v.starts_with("tcp://") || v.starts_with("ssh://"))
    {
        interfaces::capture(context)?;
    }
    Some(())
}

fn context_host(dir: &Path, name: &str, reads: &mut Reads) -> Option<String> {
    if !self::name(name) {
        return None;
    }
    let bytes = reads.read(&dir.join("contexts/meta").join(hash(name)).join("meta.json"))??;
    let meta: Meta = serde_json::from_slice(&bytes).ok()?;
    (meta.name == name && !meta.endpoints.docker.host.is_empty())
        .then_some(meta.endpoints.docker.host)
}
fn selected_builder(dir: &Path, key: &str, reads: &mut Reads) -> Option<Option<String>> {
    if let Some(bytes) = reads.read(&dir.join("current"))? {
        let current: Current = serde_json::from_slice(&bytes).ok()?;
        if !current.name.is_empty() && (current.global || current.key == key) {
            return Some(Some(current.name));
        }
    }
    let digest = hash(key);
    let Some(bytes) = reads.read(&dir.join("defaults").join(&digest[..20]))? else {
        return Some(None);
    };
    let value = std::str::from_utf8(&bytes).ok()?;
    if !name(value) {
        return None;
    }
    Some(Some(value.to_owned()))
}
fn builder_endpoint(
    dir: &Path,
    builder_dir: &Path,
    builder: &str,
    active_context: &str,
    host: &str,
    explicit: bool,
    reads: &mut Reads,
) -> Option<String> {
    if !name(builder) {
        return None;
    }
    let Some(bytes) = reads.read(&builder_dir.join("instances").join(builder))? else {
        if !explicit {
            return None;
        }
        return if builder == active_context {
            Some(host.to_owned())
        } else {
            context_host(dir, builder, reads)
        };
    };
    let group: Group = serde_json::from_slice(&bytes).ok()?;
    if group.name != builder || group.dynamic || group.nodes.len() != 1 {
        return None;
    }
    let endpoint = &group.nodes[0].endpoint;
    match group.driver.as_str() {
        "remote" => Some(endpoint.clone()),
        "docker" | "docker-container" => {
            if endpoint == active_context {
                Some(host.to_owned())
            } else if endpoint.contains("://") {
                Some(endpoint.clone())
            } else if endpoint == "default" {
                None
            } else {
                context_host(dir, endpoint, reads)
            }
        }
        _ => None,
    }
}

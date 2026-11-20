//! Endpoint facts describe the effective, revalidated compute endpoint, not a
//! context name or a mere API server. No discovery or command execution occurs here.
use crate::{
    budget::{Budget, Checked},
    Classification, EndpointContext, EndpointFact, EndpointKind, JobClass, Reason,
};
use std::net::{IpAddr, SocketAddr};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Place {
    Remote,
}

pub(crate) fn recognize(
    words: &[String],
    context: &EndpointContext,
    budget: &mut Budget,
) -> Checked<Option<Classification>> {
    let Some(head) = words.first() else {
        return Ok(None);
    };
    budget.charge(1 + head.len())?;
    let name = head.rsplit('/').next().unwrap_or("");
    if !budget.one_of(name, &["ssh", "docker", "docker-compose", "kubectl"])? {
        return Ok(None);
    }
    // An arbitrary path can name a custom program, even if its basename is docker.
    if head != name {
        return Ok(Some(ambiguous()));
    }
    // Each bounded input may be inspected by several parsers/fact lookups below.
    for word in words {
        budget.charge(4 + 4 * word.len())?;
    }
    for fact in &context.facts {
        budget.charge(4 + 4 * fact.selector.len())?;
        for endpoint in &fact.endpoints {
            budget.charge(4 + 4 * endpoint.len())?;
        }
    }
    for (key, value) in &context.environment {
        budget.charge(4 + 4 * (key.len() + value.len()))?;
    }
    let result = match name {
        "ssh" => ssh(&words[1..], context),
        "docker" => docker(&words[1..], context),
        "kubectl" => kubectl(&words[1..], context),
        _ => None,
    };
    Ok(Some(result.unwrap_or_else(ambiguous)))
}

fn ambiguous() -> Classification {
    Classification::unknown(Reason::EndpointAmbiguous)
}

fn remote(reason: Reason) -> Classification {
    Classification::new(JobClass::RemoteExec, reason)
}

fn fact<'a>(
    context: &'a EndpointContext,
    kind: EndpointKind,
    selector: &str,
) -> Option<&'a EndpointFact> {
    context
        .facts
        .iter()
        .find(|f| f.kind == kind && f.selector == selector)
}

// @local-addresses is reserved for the caller's complete, fresh host interface
// address set. A numeric non-loopback address alone can be this same host.
// Capture must also resolve proxies and forwarding before emitting compute facts.
fn remote_ip(ip: IpAddr, context: &EndpointContext) -> bool {
    let ip = match ip {
        IpAddr::V6(ip) => ip.to_ipv4_mapped().map_or(IpAddr::V6(ip), IpAddr::V4),
        other => other,
    };
    let Some(local) = fact(context, EndpointKind::SshDestination, "@local-addresses") else {
        return false;
    };
    for value in &local.endpoints {
        let Ok(address) = value.parse::<IpAddr>() else {
            return false;
        };
        let address = match address {
            IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or(IpAddr::V6(v6), IpAddr::V4),
            other => other,
        };
        if address.is_unspecified() || address == ip {
            return false;
        }
    }
    match ip {
        IpAddr::V4(ip) => {
            ip.octets()[0] != 0
                && ip.octets()[0] < 224
                && !ip.is_loopback()
                && !ip.is_unspecified()
                && !ip.is_multicast()
                && !ip.is_broadcast()
                && !ip.is_link_local()
        }
        IpAddr::V6(ip) => {
            ip.to_ipv4().is_none()
                && !ip.is_loopback()
                && !ip.is_unspecified()
                && !ip.is_multicast()
                && !ip.is_unicast_link_local()
        }
    }
}

fn socket_endpoint(value: &str, scheme: &str) -> Option<SocketAddr> {
    value.strip_prefix(scheme)?.parse().ok()
}

fn place(endpoint: &str, context: &EndpointContext) -> Option<Place> {
    // A Unix socket may be forwarded, and SSH configuration can redirect its
    // transport. Neither URI alone establishes the builder's compute host.
    if let Some(socket) = socket_endpoint(endpoint, "tcp://") {
        return (socket.port() != 0 && remote_ip(socket.ip(), context)).then_some(Place::Remote);
    }
    None
}

fn single_place(fact: &EndpointFact, context: &EndpointContext) -> Option<Place> {
    match fact.endpoints.as_slice() {
        [endpoint] => place(endpoint, context),
        _ => None,
    }
}

fn ssh(args: &[String], context: &EndpointContext) -> Option<Classification> {
    let mut at = 0;
    let mut port = None;
    while let Some(arg) = args.get(at) {
        if arg == "--" {
            at += 1;
            break;
        }
        if !arg.starts_with('-') {
            break;
        }
        // Forwarding, config overrides, proxy options and unknown flags are
        // intentionally unresolved; none may fall back to syntax-only SSH.
        if arg == "-F" {
            at += 1;
            if args.get(at).is_none_or(String::is_empty) {
                return None;
            }
            at += 1;
            continue;
        }
        if arg.starts_with("-F") && arg.len() > 2 {
            at += 1;
            continue;
        }
        let value = if arg == "-p" {
            at += 1;
            args.get(at)?.as_str()
        } else {
            arg.strip_prefix("-p")?
        };
        if port.is_some() {
            return None;
        }
        port = Some(value.parse::<u16>().ok().filter(|p| *p != 0)?);
        at += 1;
    }
    let destination = args.get(at)?;
    if destination.is_empty() || destination.starts_with('-') {
        return None;
    }
    at += 1;
    if args.get(at).is_some_and(|word| word == "--") {
        at += 1;
    }
    if args
        .get(at)
        .is_none_or(|word| word.is_empty() || word.starts_with('-'))
    {
        return None;
    }
    let selected = fact(context, EndpointKind::SshDestination, destination)?;
    let [endpoint] = selected.endpoints.as_slice() else {
        return None;
    };
    let socket = socket_endpoint(endpoint, "ssh://")?;
    if socket.port() == 0
        || !remote_ip(socket.ip(), context)
        || port.is_some_and(|port| port != socket.port())
    {
        return None;
    }
    Some(remote(Reason::SshRemoteCommand))
}

// Return a value for a separate or --option=value argument. Repeated selectors
// are rejected because their exact effective configuration has not been bound.
fn option<'a>(args: &'a [String], at: &mut usize, names: &[&str]) -> Option<Option<&'a str>> {
    let arg = args.get(*at)?;
    for name in names {
        if arg == name {
            *at += 1;
            return Some(Some(args.get(*at)?.as_str()));
        }
        if let Some(value) = arg.strip_prefix(name).and_then(|s| s.strip_prefix('=')) {
            return Some(Some(value));
        }
    }
    Some(None)
}

fn selected<'a>(slot: &mut Option<&'a str>, value: &'a str) -> Option<()> {
    if value.is_empty() || slot.replace(value).is_some() {
        return None;
    }
    Some(())
}

fn docker(args: &[String], context: &EndpointContext) -> Option<Classification> {
    let (mut at, mut cli_context, mut cli_host, mut config) = (0, None, None, None);
    while let Some(arg) = args.get(at) {
        if let Some(value) = option(args, &mut at, &["--context", "-c"])? {
            selected(&mut cli_context, value)?;
        } else if let Some(value) = option(args, &mut at, &["--host", "-H"])? {
            selected(&mut cli_host, value)?;
        } else if let Some(value) = option(args, &mut at, &["--config"])? {
            selected(&mut config, value)?;
        } else if arg.starts_with('-') {
            return None;
        } else {
            break;
        }
        at += 1;
    }
    if cli_context.is_some() && cli_host.is_some() {
        return None;
    }
    let buildx = args.get(at)?.as_str() == "buildx";
    // Only plain docker build can opt out of BuildKit and ignore the builder.
    if !buildx
        && context
            .environment
            .get("DOCKER_BUILDKIT")
            .is_some_and(|value| !value.is_empty() && value != "1")
    {
        return None;
    }
    if buildx {
        at += 1;
    }
    let mut builder = None;
    // buildx accepts --builder before the subcommand as well as on build.
    if buildx && args.get(at).is_some_and(|w| w.starts_with("--builder")) {
        let value = option(args, &mut at, &["--builder"])??;
        selected(&mut builder, value)?;
        at += 1;
    }
    if args.get(at)?.as_str() != "build" {
        return None;
    }
    at += 1;
    let mut positional = 0;
    let mut after_separator = false;
    while let Some(arg) = args.get(at) {
        if !after_separator && arg == "--" {
            after_separator = true;
        } else if !after_separator && (arg == "--no-cache" || arg == "--push") {
            // Flags with no value.
        } else if !after_separator && arg.starts_with('-') {
            if let Some(value) = option(args, &mut at, &["--builder"])? {
                selected(&mut builder, value)?;
            } else if let Some(value) = option(
                args,
                &mut at,
                &["-t", "--tag", "-f", "--file", "--platform"],
            )? {
                if value.is_empty() {
                    return None;
                }
            } else {
                return None;
            }
        } else {
            if arg.is_empty() {
                return None;
            }
            positional += 1;
        }
        at += 1;
    }
    if positional != 1 {
        return None;
    }

    let env = &context.environment;
    let env_context = env
        .get("DOCKER_CONTEXT")
        .filter(|v| !v.is_empty())
        .map(String::as_str);
    let env_host = env
        .get("DOCKER_HOST")
        .filter(|v| !v.is_empty())
        .map(String::as_str);
    let host = cli_host.or_else(|| {
        if cli_context.is_none() || cli_context == Some("default") {
            env_host
        } else {
            None
        }
    });
    let selector = cli_context
        .or(cli_host)
        .or(env_host)
        .or(env_context)
        .unwrap_or("@current");
    let engine = fact(context, EndpointKind::DockerContext, selector);
    if let (Some(host), Some(engine)) = (host, engine) {
        if engine.endpoints.as_slice() != [host] {
            return None;
        }
    }
    let explicit_builder = builder.or_else(|| {
        env.get("BUILDX_BUILDER")
            .filter(|v| !v.is_empty())
            .map(String::as_str)
    });
    // For buildx without an explicit builder, capture binds @selected/<context>
    // to the configured selected builder. For docker build the default builder
    // is bound to the effective engine selector.
    let selected_builder = if buildx && explicit_builder.is_none() {
        format!("@selected/{selector}")
    } else {
        explicit_builder.unwrap_or(selector).to_owned()
    };
    let builder_place = single_place(
        fact(context, EndpointKind::DockerBuilder, &selected_builder)?,
        context,
    )?;
    let engine_place = engine.and_then(|engine| single_place(engine, context));
    let unix_transport = engine.is_some_and(|fact| {
        matches!(fact.endpoints.as_slice(), [endpoint]
            if endpoint.starts_with("unix:///") && endpoint.len() > "unix:///".len())
    });
    if engine.is_some() && engine_place.is_none() && !(buildx && unix_transport) {
        return None;
    }
    // Plain docker build can use the engine even with a selected Buildx
    // builder (legacy fallback or older clients). Context facts do not attest
    // which client implementation will run, including with DOCKER_BUILDKIT=1.
    // Explicit buildx instead has its own attested compute placement.
    if !buildx && engine_place != Some(builder_place) {
        return None;
    }
    Some(remote(Reason::RemoteEndpointExec))
}

fn kubectl(args: &[String], context: &EndpointContext) -> Option<Classification> {
    let (mut at, mut selected_context, mut kubeconfig, mut namespace, mut pod) =
        (0, None, None, None, None);
    let mut exec = false;
    while let Some(arg) = args.get(at) {
        if arg == "--" {
            at += 1;
            break;
        }
        if let Some(value) = option(args, &mut at, &["--context"])? {
            selected(&mut selected_context, value)?;
        } else if let Some(value) = option(args, &mut at, &["--kubeconfig"])? {
            selected(&mut kubeconfig, value)?;
        } else if let Some(value) = option(args, &mut at, &["--namespace", "-n"])? {
            selected(&mut namespace, value)?;
        } else if exec && matches!(arg.as_str(), "-i" | "-t" | "-it" | "-ti") {
            // Interactive flags do not select a different pod.
        } else if exec && option(args, &mut at, &["--container", "-c"])?.is_some() {
            // Container selection still executes on the attested pod node.
        } else if arg.starts_with('-') {
            return None;
        } else if !exec && arg == "exec" {
            exec = true;
        } else if exec && pod.replace(arg.as_str()).is_none() {
            // Resource syntax is restricted to a concrete pod name.
        } else {
            return None;
        }
        at += 1;
    }
    if !exec || args.get(at).is_none_or(|word| word.is_empty()) {
        return None;
    }
    let pod = pod.filter(|value| !value.is_empty() && !value.contains('/'))?;
    let selector = selected_context.unwrap_or("@current");
    // The context fact must contain one valid API endpoint, but the API
    // server's locality never establishes where the pod actually runs.
    let [server] = fact(context, EndpointKind::KubernetesContext, selector)?
        .endpoints
        .as_slice()
    else {
        return None;
    };
    let server = server
        .strip_prefix("https://")
        .or_else(|| server.strip_prefix("http://"))?;
    let server: SocketAddr = server.parse().ok()?;
    if server.port() == 0 {
        return None;
    }
    let prefix = format!("{selector}/");
    let suffix = format!("/{pod}");
    let mut matches = context.facts.iter().filter(|f| {
        if f.kind != EndpointKind::KubernetesPod {
            return false;
        }
        let Some(rest) = f.selector.strip_prefix(&prefix) else {
            return false;
        };
        let Some(ns) = rest.strip_suffix(&suffix) else {
            return false;
        };
        !ns.is_empty() && !ns.contains('/') && namespace.is_none_or(|chosen| chosen == ns)
    });
    // When no namespace flag exists the collector must bind the fact to the
    // effective namespace; multiple candidates are not enough to select one.
    let placement = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    let [endpoint] = placement.endpoints.as_slice() else {
        return None;
    };
    let ip: IpAddr = endpoint.parse().ok()?;
    remote_ip(ip, context).then(|| remote(Reason::RemoteEndpointExec))
}

#[cfg(test)]
mod tests;

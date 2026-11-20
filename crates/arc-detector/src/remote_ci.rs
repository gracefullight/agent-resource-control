//! Bounded CI recognition over caller-supplied effective endpoint facts.
use crate::{
    budget::{Budget, Checked},
    Classification, EndpointContext, EndpointKind, JobClass, Reason,
};

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
    if !budget.one_of(name, &["gh", "git", "gitlab"])? {
        return Ok(None);
    }
    // A path can name a different executable, even if its basename matches.
    if head != name {
        return Ok(Some(Classification::unknown(Reason::EndpointAmbiguous)));
    }
    let reason = match name {
        "gh" => github(words, context, budget)?,
        "git" => git_push(words, context, budget)?,
        "gitlab" => gitlab(words, context, budget)?,
        _ => unreachable!(),
    };
    Ok(Some(match reason {
        Some(reason) => Classification::new(JobClass::RemoteCi, reason),
        None => Classification::unknown(Reason::EndpointAmbiguous),
    }))
}

fn fact_values<'a>(
    context: &'a EndpointContext,
    kind: EndpointKind,
    selector: &str,
    b: &mut Budget,
) -> Checked<Option<&'a [String]>> {
    for item in &context.facts {
        b.charge(1)?;
        if item.kind == kind && b.eq(&item.selector, selector)? {
            return Ok(Some(&item.endpoints));
        }
    }
    Ok(None)
}

fn fact<'a>(
    context: &'a EndpointContext,
    kind: EndpointKind,
    selector: &str,
    b: &mut Budget,
) -> Checked<Option<&'a str>> {
    Ok(match fact_values(context, kind, selector, b)? {
        Some([endpoint]) => Some(endpoint),
        _ => None,
    })
}

// A repo is [HOST/]OWNER/REPO, with no URL syntax, empty parts or aliases.
fn repo_host(repo: &str) -> Option<Option<&str>> {
    let parts: Vec<_> = repo.split('/').collect();
    if !(parts.len() == 2 || parts.len() == 3)
        || parts.iter().any(|part| {
            part.is_empty()
                || *part == "."
                || *part == ".."
                || !part
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'-' | b'_'))
        })
    {
        return None;
    }
    Some((parts.len() == 3).then_some(parts[0]))
}

fn github(words: &[String], context: &EndpointContext, b: &mut Budget) -> Checked<Option<Reason>> {
    if words.len() < 3 {
        return Ok(None);
    }
    b.charge(2 + words[1].len() + words[2].len())?;
    let (reason, min_positional, max_positional) = match (words[1].as_str(), words[2].as_str()) {
        ("workflow", "run") => (Reason::GhWorkflowRun, 1, 1),
        ("run", "watch") => (Reason::GhRunWatch, 0, 1),
        ("run", "view") => (Reason::GhRunView, 0, 1),
        ("run", "list") | ("workflow", "list") => (Reason::RemoteCiDispatch, 0, 0),
        ("workflow", "view") => (Reason::RemoteCiDispatch, 1, 1),
        _ => return Ok(None),
    };
    let (mut repo, mut positional, mut index) = (None, 0, 3);
    while let Some(word) = words.get(index) {
        b.charge(1 + 4 * word.len())?;
        let (option, assigned) = word
            .split_once('=')
            .map_or((word.as_str(), None), |(key, value)| (key, Some(value)));
        if option.starts_with('-') {
            let is_repo = matches!(option, "-R" | "--repo");
            let takes_value = is_repo
                || matches!(
                    (reason, option),
                    (
                        Reason::GhWorkflowRun,
                        "--ref" | "-f" | "--raw-field" | "-F" | "--field"
                    ) | (Reason::GhRunView, "--json")
                )
                || (words[1] == "run" && words[2] == "list" && option == "--workflow");
            if reason == Reason::GhRunWatch && option == "--exit-status" && assigned.is_none() {
                index += 1;
                continue;
            }
            if !takes_value
                || (assigned.is_some() && option.starts_with('-') && !option.starts_with("--"))
            {
                return Ok(None);
            }
            let value = if let Some(value) = assigned {
                value
            } else {
                index += 1;
                let Some(value) = words.get(index) else {
                    return Ok(None);
                };
                b.charge(1 + 4 * value.len())?;
                value.as_str()
            };
            if value.is_empty() || value.starts_with('-') {
                return Ok(None);
            }
            if is_repo && repo.replace(value).is_some() {
                return Ok(None);
            }
        } else {
            if word.is_empty() || assigned.is_some() {
                return Ok(None);
            }
            positional += 1;
        }
        index += 1;
    }
    if !(min_positional..=max_positional).contains(&positional) {
        return Ok(None);
    }
    // CiHost attests the host selected after repository and config discovery.
    if !matches!(fact(context, EndpointKind::CiHost, "gh", b)?, Some(host) if b.eq(host, "https://github.com")?)
    {
        return Ok(None);
    }
    let selected = repo.or_else(|| context.environment.get("GH_REPO").map(String::as_str));
    let explicit_host = if let Some(selected) = selected {
        b.charge(1 + 4 * selected.len())?;
        let Some(host) = repo_host(selected) else {
            return Ok(None);
        };
        if host.is_some_and(|host| host != "github.com") {
            return Ok(None);
        }
        host.is_some()
    } else {
        false
    };
    // An explicit host in the selected repo takes precedence over GH_HOST.
    if !explicit_host {
        if let Some(host) = context.environment.get("GH_HOST") {
            if !b.eq(host, "github.com")? {
                return Ok(None);
            }
        }
    }
    Ok(Some(reason))
}

// Only well-formed public service URLs are accepted. The GitRemote fact must
// attest the selected URL after pushurl and insteadOf configuration is applied.
fn public_git_url(url: &str) -> Option<(&str, bool)> {
    let (authority_path, ssh) = if let Some(rest) = url.strip_prefix("https://") {
        (rest, false)
    } else if let Some(rest) = url.strip_prefix("ssh://git@") {
        (rest, true)
    } else {
        return None;
    };
    let (host, path) = authority_path.split_once('/')?;
    if !matches!(host, "github.com" | "gitlab.com") || path.contains(['?', '#', ':']) {
        return None;
    }
    match repo_host(path) {
        Some(None) => Some((host, ssh)),
        _ => None,
    }
}

fn git_push(
    words: &[String],
    context: &EndpointContext,
    b: &mut Budget,
) -> Checked<Option<Reason>> {
    let mut index = 1;
    while let Some(word) = words.get(index) {
        b.charge(1 + word.len())?;
        match word.as_str() {
            "push" => {
                index += 1;
                break;
            }
            "-C" | "-c" => {
                index += 1;
                let Some(value) = words.get(index) else {
                    return Ok(None);
                };
                b.charge(1 + value.len())?;
                if value.is_empty() || (word == "-c" && !value.contains('=')) {
                    return Ok(None);
                }
            }
            _ => return Ok(None),
        }
        index += 1;
    }
    if words.get(index.saturating_sub(1)).map(String::as_str) != Some("push") {
        return Ok(None);
    }
    let mut destination = None;
    for word in &words[index..] {
        b.charge(1 + 4 * word.len())?;
        if word.starts_with('-') {
            if !matches!(
                word.as_str(),
                "--set-upstream" | "-u" | "--force-with-lease" | "--tags"
            ) {
                return Ok(None);
            }
        } else if word.is_empty() {
            return Ok(None);
        } else if destination.is_none() {
            destination = Some(word.as_str());
        }
    }
    let (kind, selector) = match destination {
        Some(destination) => (EndpointKind::GitRemote, destination),
        None => (EndpointKind::GitPushDefault, "@bare-push"),
    };
    let Some(endpoint) = fact(context, kind, selector, b)? else {
        return Ok(None);
    };
    b.charge(1 + 4 * endpoint.len())?;
    let Some((host, ssh)) = public_git_url(endpoint) else {
        return Ok(None);
    };
    if ssh {
        if context.environment.contains_key("GIT_SSH")
            || context.environment.contains_key("GIT_SSH_COMMAND")
        {
            return Ok(None);
        }
        // The independently captured SSH fact attests that selection did not
        // resolve through a local, forwarding, or ambiguous destination.
        let Some(target) = fact(context, EndpointKind::SshDestination, host, b)? else {
            return Ok(None);
        };
        b.charge(1 + 4 * target.len())?;
        // A hostname is not proof of its resolved destination. The selected
        // address must be numeric and must not be an obvious local endpoint.
        let address = target
            .strip_prefix("ssh://")
            .and_then(|value| value.strip_suffix(":22"))
            .and_then(|value| value.parse::<std::net::Ipv4Addr>().ok());
        let Some(ip) = address else {
            return Ok(None);
        };
        if ip.is_loopback()
            || ip.is_private()
            || ip.is_link_local()
            || ip.is_unspecified()
            || ip.is_multicast()
            || ip.is_broadcast()
        {
            return Ok(None);
        }
        // This fact attests the complete fresh host interface inventory.
        let Some(local) =
            fact_values(context, EndpointKind::SshDestination, "@local-addresses", b)?
        else {
            return Ok(None);
        };
        if local.is_empty() {
            return Ok(None);
        }
        for value in local {
            b.charge(1 + 4 * value.len())?;
            let Ok(address) = value.parse::<std::net::IpAddr>() else {
                return Ok(None);
            };
            let address = match address {
                std::net::IpAddr::V6(v6) => {
                    v6.to_ipv4_mapped().map_or(address, std::net::IpAddr::V4)
                }
                _ => address,
            };
            if address.is_unspecified() || address == std::net::IpAddr::V4(ip) {
                return Ok(None);
            }
        }
    }
    Ok(Some(Reason::RemoteCiDispatch))
}

fn gitlab(words: &[String], context: &EndpointContext, b: &mut Budget) -> Checked<Option<Reason>> {
    b.charge(2 + words.get(1).map_or(0, String::len) + words.get(2).map_or(0, String::len))?;
    if words.len() < 3 || words[1] != "pipeline" || words[2] != "run" {
        return Ok(None);
    }
    let mut index = 3;
    let mut branch = false;
    while let Some(word) = words.get(index) {
        b.charge(1 + 4 * word.len())?;
        if word != "--branch" || branch {
            return Ok(None);
        }
        index += 1;
        let Some(value) = words.get(index) else {
            return Ok(None);
        };
        b.charge(1 + 4 * value.len())?;
        if value.is_empty() || value.starts_with('-') {
            return Ok(None);
        }
        branch = true;
        index += 1;
    }
    if !matches!(fact(context, EndpointKind::CiHost, "gitlab", b)?, Some(host) if b.eq(host, "https://gitlab.com")?)
    {
        return Ok(None);
    }
    for (key, expected) in [
        ("GITLAB_HOST", "gitlab.com"),
        ("GLAB_HOST", "gitlab.com"),
        ("GITLAB_URL", "https://gitlab.com"),
    ] {
        if let Some(value) = context.environment.get(key) {
            if !b.eq(value, expected)? {
                return Ok(None);
            }
        }
    }
    Ok(Some(Reason::RemoteCiDispatch))
}

#[cfg(test)]
mod tests;

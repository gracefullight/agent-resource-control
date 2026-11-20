//! Resolve only a provable GitHub CLI host, without credentials or subprocesses.
use super::git::{git_config_for_gh, Reader};
use std::path::PathBuf;

pub(super) fn public_host(words: &[&str]) -> bool {
    resolve(words).unwrap_or(false)
}

fn resolve(words: &[&str]) -> Option<bool> {
    // Even an explicit repository/host still uses gh's effective transport
    // configuration, which can redirect requests away from github.com.
    let config = GhConfig::read()?;
    if !config.public_transport()? {
        return None;
    }
    let mut repo = None;
    let mut i = 3;
    while let Some(&word) = words.get(i) {
        if word == "--repo" || word == "-R" {
            i += 1;
            if repo.replace(*words.get(i)?).is_some() {
                return None;
            }
        } else if let Some(value) = word.strip_prefix("--repo=") {
            if repo.replace(value).is_some() {
                return None;
            }
        }
        i += 1;
    }
    let repo_env = std::env::var("GH_REPO").ok();
    if let Some(value) = repo.or(repo_env.as_deref()) {
        let parts: Vec<_> = value.split('/').collect();
        if !(parts.len() == 2 || parts.len() == 3)
            || parts.iter().any(|p| {
                p.is_empty()
                    || !p
                        .bytes()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'-' | b'_'))
            })
        {
            return None;
        }
        return Some(if parts.len() == 3 {
            parts[0] == "github.com"
        } else {
            config.default_host()?
        });
    }
    // Any non-GitHub remote, SSH translation or rewrite ambiguity could select
    // another authenticated host, so only all-public HTTPS remotes prove it.
    if !config.default_host()? {
        return Some(false);
    }
    Some(git_config_for_gh()?.github_fetches())
}

struct GhConfig {
    general: Option<serde_norway::Value>,
    hosts: Option<serde_norway::Value>,
}

impl GhConfig {
    fn read() -> Option<Self> {
        let directory = if let Some(path) = std::env::var_os("GH_CONFIG_DIR") {
            PathBuf::from(path)
        } else if let Some(path) = std::env::var_os("XDG_CONFIG_HOME") {
            PathBuf::from(path).join("gh")
        } else {
            PathBuf::from(std::env::var_os("HOME")?).join(".config/gh")
        };
        if !directory.is_absolute()
            || directory.as_os_str().as_encoded_bytes().len()
                > arc_detector::MAX_CONTEXT_VALUE_BYTES
        {
            return None;
        }
        let mut reader = Reader::new();
        let mut read = |name| match reader.read(&directory.join(name))? {
            Some(bytes) => Some(Some(parse_yaml(&bytes)?)),
            None => Some(None),
        };
        Some(Self {
            general: read("config.yml")?,
            hosts: read("hosts.yml")?,
        })
    }

    fn hosts(&self) -> Option<Option<&serde_norway::Mapping>> {
        let general_hosts = match self.general.as_ref() {
            Some(value) if value.is_null() => None,
            Some(value) => value
                .as_mapping()?
                .get(serde_norway::Value::String("hosts".into())),
            None => None,
        };
        // Different files can both carry host records. Reject conflicts rather
        // than assuming how a particular gh version merges duplicate keys.
        let hosts = match (general_hosts, self.hosts.as_ref()) {
            (Some(_), Some(value)) if !value.is_null() && !value.as_mapping()?.is_empty() => {
                return None
            }
            (_, Some(value)) if !value.is_null() && !value.as_mapping()?.is_empty() => Some(value),
            (general, _) => general,
        };
        let Some(value) = hosts else {
            return Some(None);
        };
        if value.is_null() {
            return Some(None);
        }
        let map = value.as_mapping()?;
        for (key, value) in map {
            let host = key.as_str()?;
            if host.is_empty() || !value.is_mapping() {
                return None;
            }
        }
        Some(Some(map))
    }

    fn public_transport(&self) -> Option<bool> {
        let general = match self.general.as_ref() {
            Some(value) if !value.is_null() => Some(value.as_mapping()?),
            _ => None,
        };
        let hosts = self.hosts()?;
        let github =
            match hosts.and_then(|m| m.get(serde_norway::Value::String("github.com".into()))) {
                Some(value) => Some(value.as_mapping()?),
                None => None,
            };
        for map in [general, github].into_iter().flatten() {
            for key in ["http_unix_socket", "api_host"] {
                if let Some(value) = map.get(serde_norway::Value::String(key.into())) {
                    if !value.is_null() && !value.as_str().is_some_and(str::is_empty) {
                        return Some(false);
                    }
                }
            }
        }
        Some(true)
    }

    fn default_host(&self) -> Option<bool> {
        if let Some(host) = std::env::var_os("GH_HOST") {
            let host = host.into_string().ok()?;
            if !host.is_empty() {
                return Some(host == "github.com");
            }
        }
        let Some(hosts) = self.hosts()? else {
            return Some(true);
        };
        Some(hosts.len() != 1 || hosts.keys().next()?.as_str()? == "github.com")
    }
}

fn parse_yaml(bytes: &[u8]) -> Option<serde_norway::Value> {
    let text = std::str::from_utf8(bytes).ok()?;
    // Reject YAML features that can make a bounded host list misleading.
    if text.contains(['&', '*', '!'])
        || text
            .lines()
            .any(|line| line.trim_start().starts_with("<<:"))
        || text
            .lines()
            .any(|line| line.trim() == "---" || line.trim() == "...")
    {
        return None;
    }
    serde_norway::from_str(text).ok()
}

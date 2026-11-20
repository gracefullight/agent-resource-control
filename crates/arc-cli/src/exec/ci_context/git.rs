//! A bounded Git configuration snapshot; Git itself is never invoked.
use std::{
    collections::BTreeSet,
    fs, io,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

mod push;
mod reader;
pub(super) use push::push;
pub(super) use reader::Reader;
const MAX_ANCESTORS: usize = 64;
const MAX_CONFIG_VALUES: usize = 512;

#[derive(Default)]
pub(super) struct Config {
    entries: Vec<Entry>,
}
struct Entry {
    section: String,
    subsection: Option<String>,
    key: String,
    value: String,
}

impl Config {
    fn insert(
        &mut self,
        section: &str,
        subsection: Option<&str>,
        key: &str,
        value: &str,
    ) -> Option<()> {
        if self.entries.len() >= MAX_CONFIG_VALUES || value.len() > 1024 || value.contains('\0') {
            return None;
        }
        self.entries.push(Entry {
            section: section.to_ascii_lowercase(),
            subsection: subsection.map(str::to_owned),
            key: key.to_ascii_lowercase(),
            value: value.to_owned(),
        });
        Some(())
    }
    fn parse(&mut self, data: &[u8]) -> Option<()> {
        let config = gix_config::File::from_bytes_no_includes(
            data,
            gix_config::file::Metadata::default(),
            Default::default(),
        )
        .ok()?;
        for section in config.sections() {
            let header = section.header();
            if header.is_legacy() {
                return None;
            }
            let name = std::str::from_utf8(header.name().as_ref()).ok()?;
            if name.eq_ignore_ascii_case("include") || name.eq_ignore_ascii_case("includeif") {
                return None;
            }
            let subsection = header
                .subsection_name()
                .map(|v| std::str::from_utf8(v.as_ref()))
                .transpose()
                .ok()?;
            let all_names: Vec<_> = section
                .value_names()
                .map(|s| s.to_ascii_lowercase())
                .collect();
            let names: BTreeSet<_> = all_names.iter().cloned().collect();
            for key in names {
                let values = section.values(&key);
                if matches!(
                    (name.to_ascii_lowercase().as_str(), key.as_str()),
                    ("branch", "remote" | "pushremote")
                        | (
                            "remote",
                            "pushdefault" | "url" | "pushurl" | "vcs" | "receivepack"
                        )
                        | ("url", "insteadof" | "pushinsteadof")
                        | ("core", "sshcommand")
                        | ("extensions", "worktreeconfig" | "refstorage")
                ) && values.len() != all_names.iter().filter(|name| *name == &key).count()
                {
                    return None;
                }
                if values.is_empty() {
                    continue;
                }
                for value in values {
                    let value = std::str::from_utf8(value.as_ref()).ok()?;
                    self.insert(name, subsection, &key, value)?;
                }
            }
        }
        Some(())
    }
    fn command_value(&mut self, key: &str, value: &str) -> Option<()> {
        let (section, rest) = key.split_once('.')?;
        let (subsection, name) = match rest.rsplit_once('.') {
            Some((sub, name)) => (Some(sub), name),
            None => (None, rest),
        };
        if section.is_empty()
            || name.is_empty()
            || section.eq_ignore_ascii_case("include")
            || section.eq_ignore_ascii_case("includeif")
        {
            return None;
        }
        self.insert(section, subsection, name, value)
    }
    fn values<'a>(
        &'a self,
        section: &'a str,
        subsection: Option<&'a str>,
        key: &'a str,
    ) -> impl Iterator<Item = &'a str> {
        self.entries
            .iter()
            .filter(move |e| {
                e.section == section && e.subsection.as_deref() == subsection && e.key == key
            })
            .map(|e| e.value.as_str())
    }
    fn last<'a>(
        &'a self,
        section: &'a str,
        subsection: Option<&'a str>,
        key: &'a str,
    ) -> Option<&'a str> {
        self.values(section, subsection, key).last()
    }
    fn names(&self, section: &str) -> Vec<String> {
        self.entries
            .iter()
            .filter(|e| e.section == section)
            .filter_map(|e| e.subsection.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }
    fn urls(&self, remote: &str, key: &str) -> Vec<String> {
        let mut result = Vec::new();
        for value in self.values("remote", Some(remote), key) {
            if value.is_empty() {
                result.clear();
            } else {
                result.push(value.to_owned());
            }
        }
        result
    }
    fn rewrite(&self, url: &str, key: &str) -> Option<String> {
        Some(
            self.rewrite_match(url, key)?
                .unwrap_or_else(|| url.to_owned()),
        )
    }
    fn rewrite_match(&self, url: &str, key: &str) -> Option<Option<String>> {
        let mut best: Option<(&str, &str)> = None;
        let mut conflicting_best = false;
        for entry in &self.entries {
            if entry.section != "url" || entry.key != key {
                continue;
            }
            let base = entry.subsection.as_deref()?;
            if url.starts_with(&entry.value) {
                if let Some((prefix, old_base)) = best {
                    if prefix.len() == entry.value.len() {
                        conflicting_best |= old_base != base;
                        continue;
                    }
                    if prefix.len() > entry.value.len() {
                        continue;
                    }
                }
                best = Some((&entry.value, base));
                conflicting_best = false;
            }
        }
        if conflicting_best {
            return None;
        }
        Some(best.map(|(prefix, base)| format!("{}{}", base, &url[prefix.len()..])))
    }
    fn endpoints(&self, selected: &str) -> Option<Vec<String>> {
        if selected.is_empty() || selected == "." {
            return None;
        }
        if self.last("core", None, "sshcommand").is_some()
            || self.last("remote", Some(selected), "vcs").is_some()
            || self.last("remote", Some(selected), "receivepack").is_some()
        {
            return None;
        }
        let push = self.urls(selected, "pushurl");
        let urls = self.urls(selected, "url");
        let configured = self.names("remote").iter().any(|name| name == selected);
        // A URL-less named remote can fall back to Git's legacy remotes/ or
        // branches/ files, which this bounded reader does not interpret.
        if configured && urls.is_empty() {
            return None;
        }
        let raw = if !push.is_empty() {
            push.clone()
        } else if !urls.is_empty() {
            urls
        } else if configured {
            return None;
        } else {
            vec![selected.to_owned()]
        };
        // Git first collects push aliases for the entire fetch URL list. If
        // any match, only those aliases are push destinations; unmatched
        // fetch URLs do not fall back individually to insteadOf.
        let aliases = if push.is_empty() {
            raw.iter()
                .map(|url| self.rewrite_match(url, "pushinsteadof"))
                .collect::<Option<Vec<_>>>()?
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let effective = if aliases.is_empty() {
            raw.iter()
                .map(|url| self.rewrite(url, "insteadof"))
                .collect::<Option<Vec<_>>>()?
        } else {
            aliases
        };
        let result: Vec<_> = effective.iter().map(|url| normalize_url(url)).collect();
        if result.is_empty() || result.len() > 16 {
            return None;
        }
        Some(result)
    }
    fn default_remote(&self, branch: Option<&str>) -> Option<String> {
        let selected = branch
            .and_then(|b| self.last("branch", Some(b), "pushremote"))
            .or_else(|| self.last("remote", None, "pushdefault"))
            .or_else(|| branch.and_then(|b| self.last("branch", Some(b), "remote")));
        if let Some(value) = selected {
            return (!value.is_empty()).then(|| value.to_owned());
        }
        let names = self.names("remote");
        Some(if names.len() == 1 {
            names[0].clone()
        } else {
            "origin".to_owned()
        })
    }
    pub(super) fn github_fetches(&self) -> bool {
        let names = self.names("remote");
        !names.is_empty()
            && names.iter().all(|name| {
                let urls = self.urls(name, "url");
                urls.len() == 1
                    && self
                        .rewrite(&urls[0], "insteadof")
                        .is_some_and(|u| valid_github_https(&u))
            })
    }
}

fn valid_github_https(url: &str) -> bool {
    let Some(path) = url.strip_prefix("https://github.com/") else {
        return false;
    };
    let parts: Vec<_> = path.split('/').collect();
    parts.len() == 2
        && parts.iter().all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-'))
        })
}
fn normalize_url(url: &str) -> String {
    if let Some(path) = url.strip_prefix("git@github.com:") {
        format!("ssh://git@github.com/{path}")
    } else if let Some(path) = url.strip_prefix("git@gitlab.com:") {
        format!("ssh://git@gitlab.com/{path}")
    } else {
        url.to_owned()
    }
}
fn env_string(key: &str) -> Option<Option<String>> {
    match std::env::var_os(key) {
        Some(value) if value.as_encoded_bytes().len() <= arc_detector::MAX_CONTEXT_VALUE_BYTES => {
            Some(Some(value.into_string().ok()?))
        }
        Some(_) => None,
        None => Some(None),
    }
}
fn env_path(key: &str) -> Option<Option<PathBuf>> {
    match std::env::var_os(key) {
        Some(value) if value.as_encoded_bytes().len() <= arc_detector::MAX_CONTEXT_VALUE_BYTES => {
            Some(Some(PathBuf::from(value)))
        }
        Some(_) => None,
        None => Some(None),
    }
}
fn bool_true(value: &str) -> Option<bool> {
    match value.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" | "" => Some(false),
        _ => None,
    }
}
fn read_config(reader: &mut Reader, config: &mut Config, path: &Path) -> Option<()> {
    if let Some(bytes) = reader.read(path)? {
        config.parse(&bytes)?;
    }
    Some(())
}
fn repository(cwd: &Path, reader: &mut Reader) -> Option<Option<PathBuf>> {
    if let Some(dir) = env_path("GIT_DIR")? {
        return Some(Some(if dir.is_absolute() {
            dir
        } else {
            cwd.join(dir)
        }));
    }
    if std::env::var_os("GIT_CEILING_DIRECTORIES").is_some()
        || std::env::var_os("GIT_DISCOVERY_ACROSS_FILESYSTEM").is_some()
    {
        return None;
    }
    let mut device = None;
    for dir in cwd.ancestors().take(MAX_ANCESTORS) {
        let current_device = fs::metadata(dir).ok()?.dev();
        if device.is_some_and(|prior| prior != current_device) {
            return None;
        }
        device = Some(current_device);
        let dot = dir.join(".git");
        match fs::symlink_metadata(&dot) {
            Ok(meta) if meta.is_dir() => return Some(Some(dot)),
            Ok(meta) if meta.is_file() => {
                let content = reader.read(&dot)??;
                let text = std::str::from_utf8(&content).ok()?.trim();
                let path = Path::new(text.strip_prefix("gitdir: ")?);
                return Some(Some(if path.is_absolute() {
                    path.to_owned()
                } else {
                    dir.join(path)
                }));
            }
            Ok(_) => return None,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(_) => return None,
        }
        // Git can also discover a bare repository at the working directory.
        // Only recognize the normal directory layout; unusual forms fail closed.
        if dir.join("HEAD").is_file() && dir.join("objects").is_dir() && dir.join("refs").is_dir() {
            return Some(Some(dir.to_owned()));
        }
    }
    if cwd.ancestors().count() > MAX_ANCESTORS {
        return None;
    }
    Some(None)
}

pub(super) fn load(
    cwd: &Path,
    command_values: &[(&str, &str)],
) -> Option<(Config, Option<String>)> {
    let no_system = bool_true(&env_string("GIT_CONFIG_NOSYSTEM")?.unwrap_or_default())?;
    if std::env::var_os("GIT_CONFIG_PARAMETERS").is_some()
        || std::env::var_os("GIT_COMMON_DIR").is_some()
    {
        return None;
    }
    let mut reader = Reader::new();
    let mut cfg = Config::default();
    reader.system_config(&mut cfg, no_system)?;
    if let Some(path) = env_path("GIT_CONFIG_GLOBAL")? {
        if !path.is_absolute() {
            return None;
        }
        read_config(&mut reader, &mut cfg, &path)?;
    } else {
        let home = env_path("HOME")??;
        if !home.is_absolute() {
            return None;
        }
        let xdg = env_path("XDG_CONFIG_HOME")?.unwrap_or_else(|| home.join(".config"));
        if !xdg.is_absolute() {
            return None;
        }
        read_config(&mut reader, &mut cfg, &xdg.join("git/config"))?;
        read_config(&mut reader, &mut cfg, &home.join(".gitconfig"))?;
    }
    let cwd = cwd.canonicalize().ok()?;
    let repo = repository(&cwd, &mut reader)??;
    let mut branch = None;
    {
        let dir = repo;
        let common = match reader.read(&dir.join("commondir"))? {
            Some(bytes) => {
                let path = Path::new(std::str::from_utf8(&bytes).ok()?.trim());
                if path.as_os_str().is_empty() {
                    return None;
                }
                if path.is_absolute() {
                    path.to_owned()
                } else {
                    dir.join(path)
                }
            }
            None => dir.clone(),
        };
        read_config(&mut reader, &mut cfg, &common.join("config"))?;
        {
            let head = reader.read(&dir.join("HEAD"))??;
            let text = std::str::from_utf8(&head).ok()?.trim();
            if let Some(name) = text.strip_prefix("ref: refs/heads/") {
                if !valid_branch(name) {
                    return None;
                }
                if let Some(value) = reader.read(&common.join("refs/heads").join(name))? {
                    let target = std::str::from_utf8(&value).ok()?.trim();
                    if target.starts_with("ref:")
                        || !(target.len() == 40 || target.len() == 64)
                        || !target.bytes().all(|c| c.is_ascii_hexdigit())
                    {
                        return None;
                    }
                }
                branch = Some(name.to_owned());
            } else if !text.bytes().all(|c| c.is_ascii_hexdigit())
                || !(text.len() == 40 || text.len() == 64)
            {
                return None;
            }
        }
    }
    let count = env_string("GIT_CONFIG_COUNT")?.unwrap_or_default();
    let count = if count.is_empty() {
        0
    } else {
        count.parse::<usize>().ok()?
    };
    if count > 32 {
        return None;
    }
    for i in 0..count {
        let key = env_string(&format!("GIT_CONFIG_KEY_{i}"))??;
        let value = env_string(&format!("GIT_CONFIG_VALUE_{i}"))??;
        cfg.command_value(&key, &value)?;
    }
    for &(key, value) in command_values {
        cfg.command_value(key, value)?;
    }
    if cfg
        .last("extensions", None, "worktreeconfig")
        .is_some_and(|v| bool_true(v) != Some(false))
        || cfg
            .last("extensions", None, "refstorage")
            .is_some_and(|v| !v.eq_ignore_ascii_case("files"))
    {
        return None;
    }
    Some((cfg, branch))
}

fn valid_branch(name: &str) -> bool {
    !name.is_empty()
        && !name.contains("..")
        && !name.contains("@{")
        && !name.ends_with('.')
        && name
            .split('/')
            .all(|part| !part.is_empty() && !part.starts_with('.') && !part.ends_with(".lock"))
        && !name.bytes().any(|c| {
            c.is_ascii_control()
                || c == b' '
                || matches!(c, b'~' | b'^' | b':' | b'?' | b'*' | b'[' | b'\\')
        })
}

pub(super) fn git_config_for_gh() -> Option<Config> {
    load(&std::env::current_dir().ok()?, &[]).map(|(c, _)| c)
}

#[cfg(test)]
mod tests;

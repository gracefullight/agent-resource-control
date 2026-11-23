//! npm's effective `include-workspace-root` for a workspace fan-out (lane C).
//!
//! npm runs the root's script beside the selected workspaces only when
//! `include-workspace-root` is true; its default is false. The setting is
//! taken from the available configuration in npm's precedence order: the
//! command line, `npm_config_*` environment variables, the project `.npmrc`
//! at the workspace root, the user config and the global config.
//!
//! The configuration files are located as npm locates them. The user config
//! is the `userconfig` of the environment or the project file, else
//! `~/.npmrc`. The global config is the `globalconfig` of the environment,
//! the project or the user file, else `etc/npmrc` under the effective
//! `prefix` (environment, project or user file, then `PREFIX`). A `~/`
//! location is expanded with `HOME`; any other relative location, or a
//! location given on the command line, is ambiguous. Locating npm's own
//! install, and so its builtin config or a default global prefix, would need
//! a tool lookup, which classification never performs; those files are not
//! consulted.
//!
//! Environment keys are normalized as npm normalizes them (`npm_config_`
//! prefix in any case, later underscores as hyphens, lower case), and their
//! values are not `ini`-decoded. Path-field values are trimmed using
//! JavaScript whitespace after decoding, as npm's `parseField` does. File
//! keys are not normalized: npm keeps an
//! `.npmrc` key as written, so `include_workspace_root` there is an unknown
//! key that cannot override `include-workspace-root`. File keys and values
//! are decoded as npm's `ini` parser decodes them (`npm_ini.rs`): quoted
//! tokens, `;` and `#` comments, including one after a value, and `\`
//! escapes, so `globalconfig=/x/npmrc # note` names `/x/npmrc`.
//!
//! Configuration that changes which packages run or which hooks run
//! (`workspace`, `workspaces`, `if-present`, `ignore-scripts`,
//! `script-shell`), an unparsable value, a token whose decoding is not
//! reproduced exactly, an `ini` section, a dotted key that `ini` could nest
//! onto another key, a `${...}` expansion in a key or a relevant value, or an
//! unreadable file is
//! ambiguous, and the fan-out is UNKNOWN/OBSERVE. A resolved setting becomes
//! the equivalent explicit flag for the analysis; execution keeps the
//! original argv. The digest of every consulted location and file is compared
//! again at the decision boundary.
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    io::{self, Read},
    os::unix::fs::OpenOptionsExt,
    path::{Component, Path, PathBuf},
};

#[path = "npm_ini.rs"]
mod ini;

/// Largest `.npmrc` read.
const MAX_NPMRC_BYTES: u64 = 65_536;
const FLAG: &str = "--include-workspace-root";
const INCLUDE: &str = "include-workspace-root";
/// Keys whose value changes the selected packages or the scripts they run.
const SELECTION_KEYS: &[&str] = &[
    "workspace",
    "workspaces",
    "if-present",
    "ignore-scripts",
    "script-shell",
];
/// Keys that locate another configuration file.
const LOCATION_KEYS: &[&str] = &["userconfig", "globalconfig", "prefix"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Npm {
    /// Not an npm workspace fan-out.
    NotApplicable,
    /// The effective setting and the digest of the evidence it came from.
    Resolved {
        include_root: bool,
        digest: [u8; 32],
    },
    Ambiguous,
}

impl Npm {
    /// The analysis words: the original argv with the effective setting as
    /// an explicit flag right after `run`.
    pub(crate) fn rewrite(&self, words: &[String]) -> Vec<String> {
        let mut words = words.to_vec();
        if let Self::Resolved { include_root, .. } = self {
            if arc_detector::scripts::cache::fan_out_syntax(&words)
                .is_some_and(|syntax| syntax.include_root.is_none())
            {
                words.insert(2, format!("{FLAG}={include_root}"));
            }
        }
        words
    }
}

/// npm's environment key: `npm_config_` stripped, every underscore but a
/// leading one a hyphen, lower case.
fn environment_key(name: &str) -> Option<String> {
    let rest = name
        .get(..11)?
        .eq_ignore_ascii_case("npm_config_")
        .then(|| &name[11..])?;
    let mut key = String::with_capacity(rest.len());
    for (at, c) in rest.chars().enumerate() {
        key.push(if c == '_' && at > 0 { '-' } else { c });
    }
    Some(key.to_ascii_lowercase())
}

/// The relevant settings of one configuration layer.
#[derive(Default)]
struct Layer {
    include: Option<bool>,
    locations: BTreeMap<&'static str, String>,
}

fn relevant(key: &str) -> bool {
    key == INCLUDE || SELECTION_KEYS.contains(&key) || LOCATION_KEYS.contains(&key)
}

/// One setting as npm's `parseField` receives it.
enum Setting {
    Bool(bool),
    Text(String),
    /// A number, `null`, an object or an undecided token.
    Other,
}
impl Setting {
    /// An environment value: never `ini`-decoded; exact `true` or `false`.
    fn environment(value: &str) -> Self {
        match value {
            "true" => Self::Bool(true),
            "false" => Self::Bool(false),
            _ => Self::Text(value.to_owned()),
        }
    }
    fn file(value: ini::Decoded) -> Self {
        match value {
            ini::Decoded::Text(text) => Self::Text(text),
            ini::Decoded::Json(serde_json::Value::Bool(value)) => Self::Bool(value),
            _ => Self::Other,
        }
    }
}

/// Apply one assignment of an exactly spelled key; `Err` when ambiguous.
fn assign(layer: &mut Layer, key: &str, value: Setting, array: bool) -> Result<(), ()> {
    if !relevant(key) {
        return Ok(());
    }
    if array || SELECTION_KEYS.contains(&key) {
        return Err(());
    }
    if key == INCLUDE {
        // The last assignment of a key wins, as in npm's ini parser.
        layer.include = Some(match value {
            Setting::Bool(value) => value,
            _ => return Err(()),
        });
        return Ok(());
    }
    let key = LOCATION_KEYS.iter().find(|k| **k == key).ok_or(())?;
    match value {
        Setting::Text(text) if !ini::js_trim(&text).is_empty() && !text.contains("${") => {
            layer.locations.insert(key, ini::js_trim(&text).to_owned());
            Ok(())
        }
        _ => Err(()),
    }
}

/// The relevant settings of one `.npmrc`'s text. Keys are compared as
/// written, after `ini` decodes them; see [`ini`].
fn parse(text: &str) -> Result<Layer, ()> {
    let mut layer = Layer::default();
    for line in ini::lines(text) {
        let (key, array, value) = match line {
            ini::Line::Skip => continue,
            ini::Line::Section => return Err(()),
            ini::Line::Assign { key, array, value } => (key, array, value),
        };
        if ini::nests(&key, &value) {
            return Err(());
        }
        // A non-string key is never one of npm's names.
        let ini::Decoded::Text(key) = key else {
            continue;
        };
        // npm expands `${...}` in keys too: any key may become a relevant one.
        if key.contains("${") {
            return Err(());
        }
        assign(&mut layer, &key, Setting::file(value), array)?;
    }
    Ok(layer)
}

/// Read a config file, following a symlink as npm does; `None` if absent.
fn read(path: &Path, hash: &mut Sha256) -> Result<Option<String>, ()> {
    hash.update((path.as_os_str().len() as u64).to_le_bytes());
    hash.update(path.as_os_str().as_encoded_bytes());
    // A FIFO named as a config file must not block classification.
    let file = match std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            hash.update(b"absent");
            return Ok(None);
        }
        Err(_) => return Err(()),
    };
    if !file.metadata().map_err(|_| ())?.is_file() {
        return Err(());
    }
    let mut bytes = Vec::new();
    file.take(MAX_NPMRC_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ())?;
    if bytes.len() as u64 > MAX_NPMRC_BYTES {
        return Err(());
    }
    hash.update(b"present");
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(&bytes);
    String::from_utf8(bytes).map(Some).map_err(|_| ())
}

/// Read and parse one configuration file; an absent file sets nothing.
fn file_layer(path: &Path, hash: &mut Sha256) -> Result<Layer, ()> {
    match read(path, hash)? {
        Some(text) => parse(&text),
        None => Ok(Layer::default()),
    }
}

/// An absolute location as npm's path fields resolve it: `~/` expanded with
/// `HOME`, then lexically normalized; anything else relative, including a
/// bare `~`, would follow npm's working directory and is ambiguous.
fn location(value: &str, home: Option<&Path>) -> Result<PathBuf, ()> {
    let path = match value.strip_prefix("~/") {
        Some(rest) => home.ok_or(())?.join(rest),
        None => PathBuf::from(value),
    };
    if !path.is_absolute() {
        return Err(());
    }
    let mut normal = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                normal.pop();
            }
            Component::CurDir => (),
            other => normal.push(other),
        }
    }
    Ok(normal)
}

/// The first layer, in precedence order, that sets `key`.
fn first<'a>(layers: &[&'a Layer], key: &str) -> Option<&'a str> {
    layers
        .iter()
        .find_map(|layer| layer.locations.get(key).map(String::as_str))
}

/// Resolve npm's root participation for `words` run at `root`.
pub(crate) fn resolve(words: &[String], root: &Path) -> Npm {
    resolve_with(words, root, std::env::vars_os())
}

pub(crate) fn resolve_with(
    words: &[String],
    root: &Path,
    environment: impl IntoIterator<Item = (OsString, OsString)>,
) -> Npm {
    let is_npm = words
        .first()
        .and_then(|p| Path::new(p).file_name())
        .is_some_and(|name| name == "npm");
    let Some(syntax) = is_npm
        .then(|| arc_detector::scripts::cache::fan_out_syntax(words))
        .flatten()
    else {
        return Npm::NotApplicable;
    };
    resolve_layers(syntax.include_root, words, root, environment).unwrap_or(Npm::Ambiguous)
}

fn resolve_layers(
    command_line: Option<bool>,
    words: &[String],
    root: &Path,
    environment: impl IntoIterator<Item = (OsString, OsString)>,
) -> Result<Npm, ()> {
    // A location on the command line would move a file; it is not decided.
    if words.iter().skip(1).any(|word| {
        LOCATION_KEYS.iter().any(|key| {
            word.strip_prefix("--")
                .and_then(|w| w.strip_prefix(key))
                .is_some_and(|rest| rest.is_empty() || rest.starts_with('='))
        })
    }) {
        return Err(());
    }
    let mut hash = Sha256::new();
    hash.update(b"arc.npm-root.v2");
    // Environment configuration, by normalized key; later duplicates of a
    // key in another spelling are ambiguous rather than order-dependent.
    let mut config = BTreeMap::new();
    let (mut home, mut prefix) = (None, None);
    for (name, value) in environment {
        let Some(name) = name.to_str() else { continue };
        match name {
            "HOME" => home = Some(PathBuf::from(&value)),
            "PREFIX" => prefix = Some(value.clone()),
            _ => (),
        }
        let Some(key) = environment_key(name) else {
            continue;
        };
        let value = value.to_str().ok_or(())?;
        if config.insert(key, value.to_owned()).is_some() {
            return Err(());
        }
    }
    let mut env = Layer::default();
    for (key, value) in config.iter().filter(|(key, _)| relevant(key)) {
        assign(&mut env, key, Setting::environment(value), false)?;
        hash.update((key.len() as u64).to_le_bytes());
        hash.update(key.as_bytes());
        hash.update((value.len() as u64).to_le_bytes());
        hash.update(value.as_bytes());
    }
    let home = home.as_deref();
    let project = file_layer(&root.join(".npmrc"), &mut hash)?;
    let user_path = match first(&[&env, &project], "userconfig") {
        Some(value) => location(value, home)?,
        None => home.ok_or(())?.join(".npmrc"),
    };
    let user = file_layer(&user_path, &mut hash)?;
    let layers = [&env, &project, &user];
    let global_path = match first(&layers, "globalconfig") {
        Some(value) => Some(location(value, home)?),
        None => match first(&layers, "prefix") {
            Some(value) => Some(location(value, home)?.join("etc/npmrc")),
            None => match prefix.as_ref() {
                Some(value) => Some(location(value.to_str().ok_or(())?, home)?.join("etc/npmrc")),
                None => None,
            },
        },
    };
    let global = match &global_path {
        Some(path) => file_layer(path, &mut hash)?,
        None => {
            hash.update(b"no-global");
            Layer::default()
        }
    };
    let include_root = command_line
        .or(env.include)
        .or(project.include)
        .or(user.include)
        .or(global.include)
        .unwrap_or(false);
    hash.update([u8::from(include_root)]);
    Ok(Npm::Resolved {
        include_root,
        digest: hash.finalize().into(),
    })
}

#[cfg(test)]
#[path = "npm_config_tests.rs"]
mod tests;

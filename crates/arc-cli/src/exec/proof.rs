//! Revalidate the two finite, file-backed workload proofs before using them.
use arc_detector::{
    scripts::{MetadataStats, PackageMetadata},
    EndpointContext, EndpointFact, EndpointKind,
};
use std::{
    fs::{self, OpenOptions},
    io::{ErrorKind, Read},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::Path,
    time::Duration,
};

const MAKE_BODY: &str = "test:\n\tcargo test\n";
const PNPM_BODY: &str = "cargo build && gh workflow run ci.yml";
const PNPM_CASE: &str = "cargo-build-gh-workflow-ci-v1";

pub(super) fn bounded_shell_startup(shell: &str) -> Option<EndpointFact> {
    let shell = shell.to_owned();
    super::endpoint_context::bounded(
        Duration::from_millis(arc_detector::MAX_CONTEXT_AGE_MS),
        move || shell_startup_fact(&shell),
    )?
}

/// Only the system shell with no executable startup source is attested.
pub(super) fn shell_startup_fact(shell: &str) -> Option<EndpointFact> {
    if !matches!(shell, "bash" | "zsh") {
        return None;
    }
    let expected = fs::canonicalize(format!("/bin/{shell}")).ok()?;
    let path = std::env::var_os("PATH")?;
    let selected = std::env::split_paths(&path).find_map(|directory| {
        let candidate = directory.join(shell);
        let metadata = fs::metadata(&candidate).ok()?;
        (metadata.is_file() && metadata.permissions().mode() & 0o111 != 0).then_some(candidate)
    })?;
    if fs::canonicalize(selected).ok()? != expected {
        return None;
    }
    if shell == "bash" {
        if std::env::var_os("BASH_ENV").is_some()
            || std::env::vars_os().any(|(key, _)| key.to_string_lossy().starts_with("BASH_FUNC_"))
        {
            return None;
        }
    } else {
        let directory = std::env::var_os("ZDOTDIR").or_else(|| std::env::var_os("HOME"))?;
        let directory = Path::new(&directory);
        if !directory.is_absolute()
            || !missing(&directory.join(".zshenv"))
            || !missing(Path::new("/etc/zshenv"))
            || !missing(Path::new("/etc/zsh/zshenv"))
        {
            return None;
        }
    }
    Some(EndpointFact {
        kind: EndpointKind::ShellStartup,
        selector: shell.into(),
        endpoints: vec!["safe-nonlogin-v1".into()],
    })
}

fn missing(path: &Path) -> bool {
    matches!(fs::symlink_metadata(path), Err(error) if error.kind() == ErrorKind::NotFound)
}

pub(super) fn makefile_fact() -> Option<EndpointFact> {
    if [
        "MAKEFLAGS",
        "GNUMAKEFLAGS",
        "MFLAGS",
        "MAKEFILES",
        "MAKEOVERRIDES",
    ]
    .iter()
    .any(|key| std::env::var_os(key).is_some())
    {
        return None;
    }
    let cwd = std::env::current_dir().ok()?;
    if !missing(&cwd.join("GNUmakefile")) {
        return None;
    }
    // GNU Make checks lowercase before uppercase. On case-insensitive file
    // systems both names may refer to the very same file.
    let selected = if !missing(&cwd.join("makefile")) {
        cwd.join("makefile")
    } else {
        cwd.join("Makefile")
    };
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW)
        .open(selected)
        .ok()?;
    if !file.metadata().ok()?.is_file() {
        return None;
    }
    let mut bytes = Vec::new();
    file.take((MAKE_BODY.len() + 1) as u64)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes != MAKE_BODY.as_bytes() {
        return None;
    }
    Some(EndpointFact {
        kind: EndpointKind::Makefile,
        selector: "test".into(),
        endpoints: vec![MAKE_BODY.into()],
    })
}

fn pnpm_config_is_default(cwd: &Path) -> bool {
    if std::env::vars_os().any(|(name, _)| {
        let name = name.to_string_lossy().to_ascii_lowercase();
        name.starts_with("npm_config_") || name.starts_with("pnpm_")
    }) {
        return false;
    }
    let Some(home) = std::env::var_os("HOME") else {
        return false;
    };
    if !missing(&Path::new(&home).join(".npmrc")) || !missing(Path::new("/etc/npmrc")) {
        return false;
    }
    cwd.ancestors().all(|directory| {
        missing(&directory.join(".npmrc")) && missing(&directory.join(".pnpmfile.cjs"))
    })
}

pub(super) fn pnpm_fact(name: &str) -> Option<EndpointFact> {
    let cwd = std::env::current_dir().ok()?.canonicalize().ok()?;
    if !pnpm_config_is_default(&cwd) {
        return None;
    }
    let mut source = super::metadata::FreshMetadata::default();
    let mut budget = MetadataStats::default();
    let package = source.package(&cwd, &mut budget).ok()?;
    if package.package_dir != cwd
        || package.scripts.get(name)?.as_str() != PNPM_BODY
        || package.scripts.contains_key(&format!("pre{name}"))
        || package.scripts.contains_key(&format!("post{name}"))
    {
        return None;
    }
    source
        .revalidate(std::slice::from_ref(&package), &mut budget)
        .ok()?;
    let digest = package
        .fingerprint
        .sha256
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    Some(EndpointFact {
        kind: EndpointKind::PnpmScript,
        selector: name.to_owned(),
        endpoints: vec![PNPM_CASE.into(), digest],
    })
}

/// Fail closed if a selected manifest changes, disappears or becomes ambiguous.
pub(crate) fn revalidate_context(context: &EndpointContext) -> bool {
    let relevant: Vec<_> = context
        .facts
        .iter()
        .filter(|fact| {
            matches!(
                fact.kind,
                EndpointKind::Makefile
                    | EndpointKind::PnpmScript
                    | EndpointKind::ShellStartup
                    | EndpointKind::ActLocal
            )
        })
        .cloned()
        .collect();
    if relevant.is_empty() {
        return true;
    }
    let Some(remaining) = arc_ipc::locality::now_ms()
        .and_then(|now| now.checked_sub(context.captured_at_ms))
        .and_then(|age| arc_detector::MAX_CONTEXT_AGE_MS.checked_sub(age))
    else {
        return false;
    };
    super::endpoint_context::bounded(Duration::from_millis(remaining), move || {
        relevant.iter().all(|fact| {
            let current = match fact.kind {
                EndpointKind::Makefile => makefile_fact(),
                EndpointKind::PnpmScript => pnpm_fact(&fact.selector),
                EndpointKind::ShellStartup => shell_startup_fact(&fact.selector),
                EndpointKind::ActLocal => {
                    let argv = ["act", "-j", "build"].map(std::ffi::OsString::from);
                    super::endpoint_context::capture(&argv).and_then(|fresh| {
                        let host = fact.endpoints.first()?;
                        if !fresh.facts.iter().any(|item| {
                            item.kind == EndpointKind::DockerContext
                                && item.selector.starts_with("@local-proof/")
                                && item.endpoints.as_slice() == [host.as_str()]
                        }) {
                            return None;
                        }
                        fresh
                            .facts
                            .into_iter()
                            .find(|item| item.kind == EndpointKind::ActLocal)
                    })
                }
                _ => unreachable!(),
            };
            current.is_some_and(|fresh| {
                fresh.kind == fact.kind
                    && fresh.selector == fact.selector
                    && fresh.endpoints == fact.endpoints
            })
        })
    })
    .unwrap_or(false)
}

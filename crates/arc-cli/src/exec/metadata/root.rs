//! The project root that names an invocation's analysis entry (lane C,
//! DET-014).
//!
//! From the canonical invocation directory, bounded metadata reads walk up
//! to the nearest `package.json`, the invocation's package. The walk then
//! continues to the nearest workspace declaration (`package.json`
//! `workspaces` or `pnpm-workspace.yaml`). When that workspace is the package
//! itself, or its declared globs name the package, the workspace root is the
//! project; otherwise the package is. A workspace root, a member directory
//! and a member subdirectory therefore share one entry, while a package the
//! globs do not name keeps its own.
//!
//! The root only names the entry. Correctness never depends on it: every
//! file an entry records is revalidated, results are keyed by the
//! invocation directory relative to the root, and a file outside the root
//! is never recorded. Discovery also records what selected the root: the
//! files it read, the paths it found absent and, when a workspace's globs
//! named the package, that membership. The entry revalidates those facts
//! inside the root like its own, so a sibling member added or removed, or an
//! edited workspace declaration that still names the package, is a miss even
//! for an entry that an ordinary member script populated first. Facts above
//! the root only choose which entry is named, and discovery observes them
//! afresh on every classification. When discovery cannot finish within its
//! budget, the nearest package, or else the invocation directory, is the
//! project and nothing is recorded.
use super::{fresh, members, validate};
use arc_detector::scripts::{cache::FileIdentity, MetadataStats};
use sha2::{Digest, Sha256};
use std::{
    io,
    path::{Path, PathBuf},
};

/// Ancestors examined above the invocation directory.
const MAX_DEPTH: usize = 24;

/// What discovery observed, by absolute path, so the entry it names can
/// revalidate the facts that selected it.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct Observed {
    /// Files read: their identity and lower-case hex content digest.
    pub(crate) files: Vec<(PathBuf, FileIdentity, String)>,
    pub(crate) absent: Vec<PathBuf>,
    /// The declaration whose globs named the invocation's package, those
    /// globs, and the candidate directories that hold a manifest.
    pub(crate) membership: Option<(PathBuf, Vec<String>, Vec<PathBuf>)>,
}

/// The project root and, when discovery finished within its budget, what
/// it observed. An unfinished discovery records nothing.
pub(crate) struct Discovery {
    pub(crate) root: PathBuf,
    pub(crate) observed: Option<Observed>,
}

/// A bounded read of `path`, recorded in `seen`.
fn observe(
    path: &Path,
    budget: &mut MetadataStats,
    seen: &mut Observed,
) -> Result<Option<Vec<u8>>, ()> {
    match fresh::read(path, budget).map_err(|_| ())? {
        Some((bytes, identity)) => {
            let digest = validate::hex(&Sha256::digest(&bytes));
            seen.files.push((path.to_owned(), identity, digest));
            Ok(Some(bytes))
        }
        None => {
            seen.absent.push(path.to_owned());
            Ok(None)
        }
    }
}

/// The declared globs at `dir` and the declaration file that names its
/// workspace, or `None` when it declares no workspace. `Err` when its
/// declaration cannot be read within the budget.
fn workspace_globs(
    dir: &Path,
    manifest: Option<&serde_json::Value>,
    budget: &mut MetadataStats,
    seen: &mut Observed,
) -> Result<Option<(Vec<String>, &'static str)>, ()> {
    let mut globs = Vec::new();
    let mut declaration = None;
    if let Some(found) = manifest.and_then(|m| members::package_json_globs(m).ok()) {
        globs.extend(found);
        declaration = Some("package.json");
    }
    if let Some(bytes) = observe(&dir.join("pnpm-workspace.yaml"), budget, seen)? {
        if let Ok(found) = members::pnpm_globs(&bytes) {
            globs.extend(found);
        }
        declaration.get_or_insert("pnpm-workspace.yaml");
    }
    Ok(declaration.map(|d| (globs, d)))
}

/// The candidates of `named` that hold a manifest, as the cache checks
/// them; `None` when one cannot be examined.
fn manifests(named: &[PathBuf]) -> Option<Vec<PathBuf>> {
    let mut budget = MetadataStats::default();
    let mut found = Vec::new();
    for dir in named {
        budget.lookup().ok()?;
        match std::fs::symlink_metadata(dir.join("package.json")) {
            Ok(meta) if meta.is_file() => found.push(dir.clone()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => (),
            _ => return None,
        }
    }
    Some(found)
}

/// The project root of the canonical directory `cwd`.
#[cfg(test)]
pub(crate) fn root(cwd: &Path) -> PathBuf {
    discover(cwd).root
}

/// The project root of the canonical directory `cwd` and what selected it.
pub(crate) fn discover(cwd: &Path) -> Discovery {
    let mut budget = MetadataStats::default();
    let mut seen = Observed::default();
    match discover_with(cwd, &mut budget, &mut seen) {
        Ok(root) => Discovery {
            root,
            observed: Some(seen),
        },
        Err(package) => Discovery {
            root: package.unwrap_or_else(|| cwd.to_owned()),
            observed: None,
        },
    }
}

/// `Err` carries the nearest package found before the budget ran out.
fn discover_with(
    cwd: &Path,
    budget: &mut MetadataStats,
    seen: &mut Observed,
) -> Result<PathBuf, Option<PathBuf>> {
    let mut package: Option<PathBuf> = None;
    for dir in cwd.ancestors().take(MAX_DEPTH) {
        let manifest = match observe(&dir.join("package.json"), budget, seen) {
            Ok(Some(bytes)) => {
                package.get_or_insert_with(|| dir.to_owned());
                serde_json::from_slice::<serde_json::Value>(&bytes).ok()
            }
            Ok(None) => None,
            Err(()) => return Err(package),
        };
        let Some((globs, declaration)) =
            workspace_globs(dir, manifest.as_ref(), budget, seen).map_err(|()| package.clone())?
        else {
            continue;
        };
        let Some(nearest) = package.as_ref() else {
            // A declaration without any package below it: the workspace.
            return Ok(dir.to_owned());
        };
        if nearest == dir {
            return Ok(dir.to_owned());
        }
        let named = members::expand(dir, &globs, budget).map_err(|_| package.clone())?;
        if !named.iter().any(|m| m == nearest) {
            return Ok(nearest.clone());
        }
        // The membership that names the package selects the workspace.
        let found = manifests(&named).ok_or_else(|| package.clone())?;
        seen.membership = Some((dir.join(declaration), globs, found));
        return Ok(dir.to_owned());
    }
    package.ok_or(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn members_and_their_subdirectories_share_the_workspace_root() {
        // Beside the test binary: neither /tmp nor TMPDIR.
        let dir = std::env::current_exe().unwrap().with_file_name(format!(
            "arc-project-root-{}-{}",
            std::process::id(),
            module_path!().replace("::", "-")
        ));
        let _ = fs::remove_dir_all(&dir);
        let write = |relative: &str, text: &str| {
            let path = dir.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        };
        write("ws/package.json", r#"{"workspaces":["packages/*"]}"#);
        write("ws/packages/a/package.json", r#"{"name":"a"}"#);
        write("ws/other/package.json", r#"{"name":"other"}"#);
        write("pn/pnpm-workspace.yaml", "packages:\n  - 'apps/*'\n");
        write("pn/apps/web/package.json", r#"{"name":"web"}"#);
        fs::create_dir_all(dir.join("ws/packages/a/src/deep")).unwrap();
        fs::create_dir_all(dir.join("ws/packages/empty")).unwrap();
        let dir = dir.canonicalize().unwrap();
        let ws = dir.join("ws");
        for cwd in [
            ws.clone(),
            ws.join("packages/a"),
            ws.join("packages/a/src/deep"),
            // No manifest of its own: the workspace root is its package.
            ws.join("packages/empty"),
        ] {
            assert_eq!(root(&cwd), ws, "{}", cwd.display());
        }
        // A package the globs do not name is its own project.
        assert_eq!(root(&ws.join("other")), ws.join("other"));
        // pnpm declares its members in pnpm-workspace.yaml.
        assert_eq!(root(&dir.join("pn/apps/web")), dir.join("pn"));
        // An exhausted budget falls back to the nearest package.
        let mut spent = MetadataStats {
            lookups: arc_detector::scripts::MAX_PACKAGE_LOOKUPS - 3,
            ..MetadataStats::default()
        };
        assert_eq!(
            discover_with(
                &ws.join("packages/a/src"),
                &mut spent,
                &mut Observed::default()
            ),
            Err(Some(ws.join("packages/a")))
        );
        fs::remove_dir_all(&dir).unwrap();
    }
}

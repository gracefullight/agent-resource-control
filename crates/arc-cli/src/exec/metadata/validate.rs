//! Validation, construction and bounding of project analysis entries.
//!
//! [`read`] reads every file of an entry under identity checks, compares its
//! content digest and re-evaluates the declared globs; it returns the
//! validated bytes for analysis. [`confirm`] is the decision boundary of a
//! recorded result: the globs are re-evaluated again and every file must
//! still have the identity its validated read observed. Both also require
//! every nearer manifest an upward lookup found absent to remain absent.
use super::{fresh, members, root::Observed};
use arc_detector::scripts::{
    cache::{
        CachedFile, Entry, FileIdentity, Membership, ENTRY_VERSION, MAX_ENTRY_ABSENT,
        MAX_ENTRY_BYTES, MAX_ENTRY_FILES,
    },
    MetadataStats, ScriptIssue, ScriptResolution,
};
use fresh::FreshMetadata;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};

/// Validated bytes and current identities by absolute path.
pub(crate) type Overlay = Arc<BTreeMap<PathBuf, (Vec<u8>, FileIdentity)>>;

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
pub(crate) fn relative(root: &Path, path: &Path) -> Option<String> {
    let rest = path.strip_prefix(root).ok()?.to_str()?;
    (!rest.is_empty()).then(|| rest.to_owned())
}

/// The validated bytes when every file and membership is unchanged; equal
/// bytes under a changed identity refresh that identity in `entry`.
pub(crate) fn read(entry: &mut Entry, root: &Path) -> Option<(Overlay, bool)> {
    let mut budget = MetadataStats::default();
    let mut overlay = BTreeMap::new();
    let mut refreshed = false;
    for file in &mut entry.files {
        let path = root.join(&file.path);
        // Identity is a race detector, not a substitute for content validation.
        let (bytes, identity) = fresh::read(&path, &mut budget).ok()??;
        if hex(&Sha256::digest(&bytes)) != file.sha256 {
            return None;
        }
        if file.identity != identity {
            file.identity = identity;
            refreshed = true;
        }
        overlay.insert(path, (bytes, identity));
    }
    check_memberships(entry, root)?;
    check_absent(entry, root, &mut budget)?;
    Some((Arc::new(overlay), refreshed))
}

/// The final check of a recorded result after [`read`].
pub(crate) fn confirm(entry: &Entry, root: &Path) -> bool {
    confirm_inner(entry, root).is_some()
}
fn confirm_inner(entry: &Entry, root: &Path) -> Option<()> {
    check_memberships(entry, root)?;
    // No file may change while the remaining files or membership are checked.
    let mut budget = MetadataStats::default();
    check_absent(entry, root, &mut budget)?;
    for file in &entry.files {
        budget.lookup().ok()?;
        let meta = std::fs::symlink_metadata(root.join(&file.path)).ok()?;
        if !meta.is_file() || fresh::identity(&meta) != file.identity {
            return None;
        }
    }
    Some(())
}

/// [`read`] then [`confirm`], with `between` run after the first membership
/// check: a race-test seam.
#[cfg(test)]
pub(crate) fn validate_with(
    entry: &mut Entry,
    root: &Path,
    between: impl FnOnce(),
) -> Option<bool> {
    let (_, refreshed) = read(entry, root)?;
    between();
    confirm(entry, root).then_some(refreshed)
}

/// Every recorded nearer manifest is still absent: nothing, not even a
/// symlink or directory, has been created at its path.
fn check_absent(entry: &Entry, root: &Path, budget: &mut MetadataStats) -> Option<()> {
    for path in &entry.absent {
        budget.lookup().ok()?;
        match std::fs::symlink_metadata(root.join(path)) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            _ => return None,
        }
    }
    Some(())
}

fn check_memberships(entry: &Entry, root: &Path) -> Option<()> {
    let mut budget = MetadataStats::default();
    for membership in &entry.memberships {
        let declaration = root.join(&membership.declaration);
        let workspace = declaration.parent()?;
        let mut found = Vec::new();
        for dir in members::expand(workspace, &membership.globs, &mut budget).ok()? {
            budget.lookup().ok()?;
            match std::fs::symlink_metadata(dir.join("package.json")) {
                Ok(meta) if meta.is_file() => found.push(relative(root, &dir)?),
                Ok(_) => return None,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                Err(_) => return None,
            }
        }
        if found != membership.members {
            return None;
        }
    }
    Some(())
}

/// The parts of an analysis an entry records.
#[derive(Debug, Default, Clone)]
pub(crate) struct Parts {
    pub(crate) files: Vec<CachedFile>,
    pub(crate) memberships: Vec<Membership>,
    pub(crate) absent: Vec<String>,
}

/// The files, memberships and absent nearer manifests of a completed
/// analysis, or `None` when they are not reusable: a manifest or lookup
/// outside the project root, or a metadata failure that could be transient.
pub(crate) fn parts(
    metadata: &FreshMetadata,
    result: &ScriptResolution,
    root: &Path,
) -> Option<Parts> {
    if matches!(result.issue, Some(ScriptIssue::Metadata(_))) || result.packages.is_empty() {
        return None;
    }
    let absent = metadata
        .absent()?
        .iter()
        .map(|path| relative(root, path))
        .collect::<Option<Vec<_>>>()?;
    let mut files: Vec<CachedFile> = Vec::new();
    for read in &metadata.reads {
        let path = relative(root, &read.path)?;
        if files.iter().any(|f| f.path == path) {
            continue;
        }
        files.push(CachedFile {
            path,
            identity: read.identity,
            sha256: hex(&read.fingerprint.sha256),
        });
    }
    let memberships = metadata
        .workspaces
        .iter()
        .map(|w| {
            Some(Membership {
                declaration: relative(root, &w.root.join(w.declaration.file()))?,
                globs: w.globs.clone(),
                members: w
                    .members
                    .iter()
                    .map(|m| relative(root, m))
                    .collect::<Option<_>>()?,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    Some(Parts {
        files,
        memberships,
        absent,
    })
}

/// The discovery facts inside `root` that an entry records, or `None` when
/// one inside the root cannot be represented. Facts above the root only
/// chose the entry and are observed again by every classification.
pub(crate) fn discovered(observed: &Observed, root: &Path) -> Option<Parts> {
    let inside = |path: &Path| path.starts_with(root).then(|| relative(root, path));
    let mut parts = Parts::default();
    for (path, identity, sha256) in &observed.files {
        if let Some(path) = inside(path) {
            parts.files.push(CachedFile {
                path: path?,
                identity: *identity,
                sha256: sha256.clone(),
            });
        }
    }
    for path in &observed.absent {
        if let Some(path) = inside(path) {
            parts.absent.push(path?);
        }
    }
    if let Some((declaration, globs, found)) = &observed.membership {
        parts.memberships.push(Membership {
            declaration: inside(declaration)??,
            globs: globs.clone(),
            members: found
                .iter()
                .map(|m| relative(root, m))
                .collect::<Option<_>>()?,
        });
    }
    Some(parts)
}

/// Whether `entry` holds every file of `parts` with the same content.
pub(crate) fn holds(entry: &Entry, parts: &Parts) -> bool {
    parts.files.iter().all(|file| {
        entry
            .files
            .iter()
            .any(|f| f.path == file.path && f.sha256 == file.sha256)
    })
}

impl Parts {
    /// Both analyses' facts, or `None` when they disagree: one file read
    /// with two contents, or read and found absent.
    pub(crate) fn join(mut self, other: Parts) -> Option<Parts> {
        for file in other.files {
            match self.files.iter().find(|f| f.path == file.path) {
                Some(known) if known.sha256 == file.sha256 => (),
                Some(_) => return None,
                None => self.files.push(file),
            }
        }
        for membership in other.memberships {
            if !self.memberships.contains(&membership) {
                self.memberships.push(membership);
            }
        }
        for path in other.absent {
            if !self.absent.contains(&path) {
                self.absent.push(path);
            }
        }
        if self.files.iter().any(|f| self.absent.contains(&f.path)) {
            return None;
        }
        Some(self)
    }
}

/// The project entry after an analysis: the validated `previous` entry
/// extended by this analysis's files and memberships, or only the latter
/// when they disagree or exceed the bound.
pub(crate) fn merge(previous: Option<Entry>, parts: Parts) -> Entry {
    let Parts {
        files,
        memberships,
        absent,
    } = parts;
    let fresh = || Entry {
        version: ENTRY_VERSION,
        files: files.clone(),
        memberships: memberships.clone(),
        absent: absent.clone(),
        results: Vec::new(),
    };
    let Some(mut entry) = previous else {
        return fresh();
    };
    for file in &files {
        match entry.files.iter_mut().find(|f| f.path == file.path) {
            Some(known) if known.sha256 == file.sha256 => known.identity = file.identity,
            Some(_) => return fresh(),
            None => entry.files.push(file.clone()),
        }
    }
    // One declaration may be recorded under several glob lists: a runner's
    // own and the combined lists discovery expands.
    for membership in &memberships {
        match entry
            .memberships
            .iter()
            .find(|m| m.declaration == membership.declaration && m.globs == membership.globs)
        {
            Some(known) if known == membership => (),
            Some(_) => return fresh(),
            None => entry.memberships.push(membership.clone()),
        }
    }
    for path in &absent {
        if entry.files.iter().any(|f| &f.path == path) {
            return fresh();
        }
        if !entry.absent.contains(path) {
            entry.absent.push(path.clone());
        }
    }
    if entry.files.iter().any(|f| entry.absent.contains(&f.path)) {
        return fresh();
    }
    if entry.files.len() > MAX_ENTRY_FILES
        || entry.memberships.len() > MAX_ENTRY_FILES
        || entry.absent.len() > MAX_ENTRY_ABSENT
    {
        return fresh();
    }
    entry
}

/// Drop the oldest results until the serialized entry fits its bound.
pub(crate) fn fit(entry: &mut Entry) {
    while !entry.results.is_empty()
        && serde_json::to_vec(entry).map_or(true, |bytes| bytes.len() > MAX_ENTRY_BYTES)
    {
        entry.results.remove(0);
    }
}

//! Fresh, bounded package metadata at the execution boundary. No tool probes.
//!
//! Besides single-package lookups this adapter lists workspace members
//! (lane C): only the root manifest, the runner's workspace declaration and
//! the manifests of directories its globs name are read. Every read is
//! recorded with the file identity taken before its bytes, for the project
//! analysis cache. With an overlay of bytes the cache has just validated, a
//! file in the overlay is served from it under the same budget charges, and
//! any other present file is read fresh and marks the analysis as extended.
use super::members;
use arc_detector::scripts::{
    cache::FileIdentity, MetadataError as E, MetadataFingerprint, MetadataStats, PackageMetadata,
    PackageSnapshot, WorkspaceMember, MAX_METADATA_BYTES, MAX_PACKAGE_BYTES, MAX_PACKAGE_SCRIPTS,
    MAX_RETAINED_SCRIPT_BYTES,
};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{File, OpenOptions},
    io::{self, Read},
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
};

/// Where a runner declares its workspace globs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum Declaration {
    /// `package.json` `workspaces` (Bun, npm and Yarn).
    #[default]
    PackageJson,
    /// `pnpm-workspace.yaml` `packages` (pnpm ignores `workspaces`).
    Pnpm,
}
impl Declaration {
    pub(crate) fn of(program: &str) -> Self {
        if Path::new(program).file_name().is_some_and(|n| n == "pnpm") {
            Self::Pnpm
        } else {
            Self::PackageJson
        }
    }
    pub(crate) fn file(self) -> &'static str {
        match self {
            Self::PackageJson => "package.json",
            Self::Pnpm => "pnpm-workspace.yaml",
        }
    }
}

/// One bounded, identity-checked read and its content digest.
#[derive(Clone)]
pub(crate) struct FileRead {
    pub(crate) path: PathBuf,
    pub(crate) identity: FileIdentity,
    pub(crate) fingerprint: MetadataFingerprint,
}

/// One listed workspace, re-evaluated by `revalidate` and by the cache.
#[derive(Clone)]
pub(crate) struct ListedWorkspace {
    pub(crate) root: PathBuf,
    pub(crate) declaration: Declaration,
    pub(crate) declaration_fingerprint: MetadataFingerprint,
    pub(crate) globs: Vec<String>,
    pub(crate) members: Vec<PathBuf>,
}

#[derive(Clone, Default)]
pub(crate) struct FreshMetadata {
    packages: BTreeMap<PathBuf, PackageSnapshot>,
    origins: BTreeMap<PathBuf, PathBuf>,
    names: BTreeMap<PathBuf, Option<String>>,
    /// Canonical member directories: built from a canonical root and
    /// non-symlink entries, so they need no further lookup.
    member_dirs: BTreeSet<PathBuf>,
    declaration: Declaration,
    pub(crate) reads: Vec<FileRead>,
    pub(crate) workspaces: Vec<ListedWorkspace>,
    /// Validated bytes and current identities by absolute path.
    overlay: std::sync::Arc<BTreeMap<PathBuf, (Vec<u8>, FileIdentity)>>,
    /// A present file outside the overlay was read.
    pub(crate) extended: bool,
}

impl FreshMetadata {
    /// The adapter for an invocation of `program`.
    pub(crate) fn for_program(program: &str) -> Self {
        Self {
            declaration: Declaration::of(program),
            ..Self::default()
        }
    }
    /// The adapter for `program` over the cache's validated bytes.
    pub(crate) fn with_overlay(
        program: &str,
        overlay: std::sync::Arc<BTreeMap<PathBuf, (Vec<u8>, FileIdentity)>>,
    ) -> Self {
        Self {
            overlay,
            ..Self::for_program(program)
        }
    }
    /// A bounded read, from the overlay when it holds `path`.
    fn read(
        &mut self,
        path: &Path,
        budget: &mut MetadataStats,
    ) -> Result<Option<(Vec<u8>, FileIdentity)>, E> {
        let Some((bytes, identity)) = self.overlay.get(path) else {
            let read = read(path, budget)?;
            self.extended |= read.is_some();
            return Ok(read);
        };
        // The charges of `read_checked`, which produced these bytes.
        budget.lookup()?;
        if MAX_METADATA_BYTES.saturating_sub(budget.bytes) == 0 {
            return Err(E::AggregateBytes);
        }
        budget.read(bytes.len())?;
        Ok(Some((bytes.clone(), *identity)))
    }
    /// The nearer manifests an upward package lookup found absent: from each
    /// invocation directory up to, not including, the package it resolved.
    /// `None` when a resolved package is not an ancestor of its origin.
    pub(crate) fn absent(&self) -> Option<Vec<PathBuf>> {
        let mut absent = Vec::new();
        for (origin, dir) in &self.origins {
            if !origin.starts_with(dir) {
                return None;
            }
            for directory in origin.ancestors().take_while(|a| a != dir) {
                let path = directory.join("package.json");
                if !absent.contains(&path) {
                    absent.push(path);
                }
            }
        }
        Some(absent)
    }
}

pub(crate) fn identity(meta: &std::fs::Metadata) -> FileIdentity {
    FileIdentity {
        dev: meta.dev(),
        ino: meta.ino(),
        size: meta.size(),
        mtime_ns: i128::from(meta.mtime()) * 1_000_000_000 + i128::from(meta.mtime_nsec()),
        ctime_ns: i128::from(meta.ctime()) * 1_000_000_000 + i128::from(meta.ctime_nsec()),
    }
}

/// Open a regular file without following a symlink or blocking on a FIFO.
pub(crate) fn open(path: &Path) -> Result<Option<(File, FileIdentity)>, E> {
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(E::Unreadable),
    };
    let meta = file.metadata().map_err(|_| E::Unreadable)?;
    if !meta.is_file() {
        return Err(E::Unreadable);
    }
    Ok(Some((file, identity(&meta))))
}

pub(crate) fn read(
    path: &Path,
    budget: &mut MetadataStats,
) -> Result<Option<(Vec<u8>, FileIdentity)>, E> {
    read_checked(path, budget, || {})
}

fn read_checked(
    path: &Path,
    budget: &mut MetadataStats,
    after_read: impl FnOnce(),
) -> Result<Option<(Vec<u8>, FileIdentity)>, E> {
    budget.lookup()?;
    let remaining = MAX_METADATA_BYTES
        .checked_sub(budget.bytes)
        .ok_or(E::AggregateBytes)?;
    if remaining == 0 {
        return Err(E::AggregateBytes);
    }
    // O_NONBLOCK prevents a FIFO/device from blocking even before fstat, and
    // O_NOFOLLOW rejects a manifest symlink instead of following a moving target.
    let Some((file, identity)) = open(path)? else {
        return Ok(None);
    };
    let allowance = MAX_PACKAGE_BYTES.min(remaining);
    let mut bytes = Vec::new();
    let read_result = (&file).take((allowance + 1) as u64).read_to_end(&mut bytes);
    // Include the overflow sentinel in the checked charge. No oversized input
    // reaches JSON decoding or script-map retention.
    budget.read(bytes.len())?;
    read_result.map_err(|_| E::Unreadable)?;
    after_read();
    // Check both the open file and the directory entry: a rename can leave
    // the descriptor stable while replacing the path used for classification.
    let after = file.metadata().map_err(|_| E::Changed)?;
    let path_after = std::fs::symlink_metadata(path).map_err(|_| E::Changed)?;
    if !path_after.is_file()
        || identity != self::identity(&after)
        || identity != self::identity(&path_after)
    {
        return Err(E::Changed);
    }
    Ok(Some((bytes, identity)))
}

#[cfg(test)]
pub(super) fn race_read(path: &Path, change: impl FnOnce()) -> Result<(), E> {
    read_checked(path, &mut MetadataStats::default(), change).map(|_| ())
}
pub(crate) fn fingerprint(bytes: &[u8]) -> MetadataFingerprint {
    MetadataFingerprint {
        bytes: bytes.len(),
        sha256: Sha256::digest(bytes).into(),
    }
}
fn scripts(
    object: &serde_json::Map<String, serde_json::Value>,
    budget: &mut MetadataStats,
) -> Result<BTreeMap<String, String>, E> {
    let Some(value) = object.get("scripts") else {
        return Ok(BTreeMap::new());
    };
    let map = value.as_object().ok_or(E::Invalid)?;
    if map.len() > MAX_PACKAGE_SCRIPTS {
        return Err(E::ScriptLimit);
    }
    // Validate the aggregate before cloning any names/bodies into the snapshot.
    let mut bytes = budget.script_bytes;
    for (name, body) in map {
        let body = body.as_str().ok_or(E::Invalid)?;
        if name.is_empty() || name.contains('\0') || body.contains('\0') {
            return Err(E::Invalid);
        }
        bytes = bytes
            .checked_add(name.len())
            .and_then(|n| n.checked_add(body.len()))
            .filter(|n| *n <= MAX_RETAINED_SCRIPT_BYTES)
            .ok_or(E::RetainedScriptBytes)?;
    }
    let scripts = map
        .iter()
        .map(|(name, body)| (name.clone(), body.as_str().unwrap().to_owned()))
        .collect();
    budget.scripts(&scripts)?;
    Ok(scripts)
}
fn canonical(cwd: &Path, budget: &mut MetadataStats) -> Result<PathBuf, E> {
    budget.lookup()?;
    cwd.canonicalize().map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            E::Missing
        } else {
            E::Unreadable
        }
    })
}

impl FreshMetadata {
    /// Read, retain and record `<dir>/package.json`; `None` when absent.
    fn load(
        &mut self,
        dir: &Path,
        budget: &mut MetadataStats,
    ) -> Result<Option<(PackageSnapshot, serde_json::Value)>, E> {
        let path = dir.join("package.json");
        let Some((bytes, identity)) = self.read(&path, budget)? else {
            return Ok(None);
        };
        let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|_| E::Invalid)?;
        let object = value.as_object().ok_or(E::Invalid)?;
        let snapshot = PackageSnapshot {
            package_dir: dir.to_owned(),
            fingerprint: fingerprint(&bytes),
            scripts: scripts(object, budget)?,
        };
        budget.file()?;
        let name = object
            .get("name")
            .and_then(|n| n.as_str())
            .map(str::to_owned);
        self.names.insert(dir.to_owned(), name);
        self.packages.insert(dir.to_owned(), snapshot.clone());
        self.reads.push(FileRead {
            path,
            identity,
            fingerprint: snapshot.fingerprint.clone(),
        });
        Ok(Some((snapshot, value)))
    }

    /// The declared globs of the workspace rooted at the canonical `root`.
    fn globs(&mut self, root: &Path, budget: &mut MetadataStats) -> Result<ListedWorkspace, E> {
        let (_, manifest) = self.load(root, budget)?.ok_or(E::Missing)?;
        let (globs, fingerprint) = match self.declaration {
            Declaration::PackageJson => (
                members::package_json_globs(&manifest)?,
                self.packages[root].fingerprint.clone(),
            ),
            Declaration::Pnpm => {
                let path = root.join(Declaration::Pnpm.file());
                let (bytes, identity) = self.read(&path, budget)?.ok_or(E::Missing)?;
                let fingerprint = fingerprint(&bytes);
                self.reads.push(FileRead {
                    path,
                    identity,
                    fingerprint: fingerprint.clone(),
                });
                (members::pnpm_globs(&bytes)?, fingerprint)
            }
        };
        Ok(ListedWorkspace {
            root: root.to_owned(),
            declaration: self.declaration,
            declaration_fingerprint: fingerprint,
            globs,
            members: Vec::new(),
        })
    }

    /// Fresh reads of a listed workspace: the same declaration, the same
    /// glob membership and unchanged member manifests.
    fn recheck(
        &self,
        listed: &ListedWorkspace,
        checked: &mut BTreeMap<PathBuf, MetadataFingerprint>,
        checked_files: &mut BTreeMap<PathBuf, FileIdentity>,
        budget: &mut MetadataStats,
    ) -> Result<(), E> {
        let path = listed.root.join(listed.declaration.file());
        let (bytes, identity) = read(&path, budget)?.ok_or(E::Changed)?;
        if fingerprint(&bytes) != listed.declaration_fingerprint {
            return Err(E::Changed);
        }
        checked_files.insert(path, identity);
        let mut members = Vec::new();
        for dir in members::expand(&listed.root, &listed.globs, budget)? {
            let path = dir.join("package.json");
            if let Some((bytes, identity)) = read(&path, budget)? {
                let hash = fingerprint(&bytes);
                if self.packages.get(&dir).map(|p| &p.fingerprint) != Some(&hash) {
                    return Err(E::Changed);
                }
                checked.insert(dir.clone(), hash);
                checked_files.insert(path, identity);
                members.push(dir);
            }
        }
        if members != listed.members {
            return Err(E::Changed);
        }
        Ok(())
    }

    pub(super) fn revalidate_inner(
        &self,
        packages: &[PackageSnapshot],
        budget: &mut MetadataStats,
        after_membership: impl FnOnce(),
    ) -> Result<(), E> {
        // Repeat discovery as well as content checks: a newly created nearer
        // package.json must not silently change which script Bun will resolve.
        let mut checked = BTreeMap::new();
        let mut checked_files = BTreeMap::new();
        for (origin, expected) in &self.origins {
            let mut directory = canonical(origin, budget)?;
            loop {
                if let Some(hash) = checked.get(&directory) {
                    if &directory != expected || self.packages[expected].fingerprint != *hash {
                        return Err(E::Changed);
                    }
                    break;
                }
                let path = directory.join("package.json");
                if let Some((bytes, identity)) = read(&path, budget)? {
                    let hash = fingerprint(&bytes);
                    if &directory != expected || self.packages[expected].fingerprint != hash {
                        return Err(E::Changed);
                    }
                    checked.insert(directory, hash);
                    checked_files.insert(path, identity);
                    break;
                }
                if !directory.pop() {
                    return Err(E::Missing);
                }
            }
        }
        for listed in &self.workspaces {
            self.recheck(listed, &mut checked, &mut checked_files, budget)?;
        }
        for package in packages {
            if checked.get(&package.package_dir) != Some(&package.fingerprint) {
                return Err(E::Changed);
            }
        }
        after_membership();
        // Check membership a second time, then compare all consulted path
        // identities from the content reads. No cached or fresh result can
        // rely on a file replaced during a later part of the validation.
        let mut final_budget = MetadataStats::default();
        for listed in &self.workspaces {
            let mut members = Vec::new();
            for dir in members::expand(&listed.root, &listed.globs, &mut final_budget)? {
                final_budget.lookup()?;
                match std::fs::symlink_metadata(dir.join("package.json")) {
                    Ok(meta) if meta.is_file() => members.push(dir),
                    Ok(_) => return Err(E::Changed),
                    Err(error) if error.kind() == io::ErrorKind::NotFound => (),
                    Err(_) => return Err(E::Changed),
                }
            }
            if members != listed.members {
                return Err(E::Changed);
            }
        }
        for read in &self.reads {
            final_budget.lookup()?;
            let meta = std::fs::symlink_metadata(&read.path).map_err(|_| E::Changed)?;
            if !meta.is_file() || checked_files.get(&read.path) != Some(&identity(&meta)) {
                return Err(E::Changed);
            }
        }
        Ok(())
    }
}

impl PackageMetadata for FreshMetadata {
    fn package(&mut self, cwd: &Path, budget: &mut MetadataStats) -> Result<PackageSnapshot, E> {
        if let Some(snapshot) = self.member_dirs.get(cwd).and_then(|d| self.packages.get(d)) {
            return Ok(snapshot.clone());
        }
        let origin = canonical(cwd, budget)?;
        if let Some(dir) = self.origins.get(&origin) {
            return Ok(self.packages[dir].clone());
        }
        let mut directory = origin.clone();
        loop {
            if let Some(snapshot) = self.packages.get(&directory) {
                let snapshot = snapshot.clone();
                self.origins.insert(origin, directory);
                return Ok(snapshot);
            }
            if let Some((snapshot, _)) = self.load(&directory, budget)? {
                self.origins.insert(origin, directory);
                return Ok(snapshot);
            }
            if !directory.pop() {
                return Err(E::Missing);
            }
        }
    }
    fn revalidate(
        &mut self,
        packages: &[PackageSnapshot],
        budget: &mut MetadataStats,
    ) -> Result<(), E> {
        self.revalidate_inner(packages, budget, || {})
    }
    fn workspace_members(
        &mut self,
        root: &Path,
        budget: &mut MetadataStats,
    ) -> Result<Option<Vec<WorkspaceMember>>, E> {
        let root = canonical(root, budget)?;
        if self.workspaces.iter().any(|w| w.root == root) {
            // One listing per analysis keeps revalidation exact.
            return Err(E::Invalid);
        }
        let mut listed = self.globs(&root, budget)?;
        // The root itself, which no glob names, supplies its manifest name.
        let mut members = vec![WorkspaceMember {
            name: self.names.get(&root).cloned().flatten(),
            dir: root.clone(),
        }];
        for dir in members::expand(&root, &listed.globs, budget)? {
            if self.load(&dir, budget)?.is_none() {
                continue;
            }
            self.member_dirs.insert(dir.clone());
            members.push(WorkspaceMember {
                name: self.names.get(&dir).cloned().flatten(),
                dir: dir.clone(),
            });
            listed.members.push(dir);
        }
        self.workspaces.push(listed);
        Ok(Some(members))
    }
}

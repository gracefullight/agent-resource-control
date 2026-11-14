//! Fresh, bounded package metadata at the execution boundary. No tool probes.
use arc_detector::{
    scripts::{
        MetadataError as E, MetadataFingerprint, MetadataStats, PackageMetadata, PackageSnapshot,
        MAX_METADATA_BYTES, MAX_PACKAGE_BYTES, MAX_PACKAGE_SCRIPTS, MAX_RETAINED_SCRIPT_BYTES,
    },
    Classification,
};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    fs::OpenOptions,
    io::{self, Read},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
};

#[derive(Default)]
pub(super) struct FreshMetadata {
    packages: BTreeMap<PathBuf, PackageSnapshot>,
    origins: BTreeMap<PathBuf, PathBuf>,
}

pub(super) fn classify(argv: &[OsString]) -> Classification {
    let Ok(cwd) = std::env::current_dir() else {
        return arc_detector::classify_argv(argv).legacy();
    };
    let mut metadata = FreshMetadata::default();
    let mut result = arc_detector::analyze_script_argv(argv, &cwd, &mut metadata);
    if !result.packages.is_empty()
        && metadata
            .revalidate(&result.packages, &mut result.metadata)
            .is_err()
    {
        return arc_detector::classify_argv(argv).legacy();
    }
    result.classification
}

fn read(path: &Path, budget: &mut MetadataStats) -> Result<Option<Vec<u8>>, E> {
    budget.lookup()?;
    let remaining = MAX_METADATA_BYTES
        .checked_sub(budget.bytes)
        .ok_or(E::AggregateBytes)?;
    if remaining == 0 {
        return Err(E::AggregateBytes);
    }
    // O_NONBLOCK prevents a FIFO/device from blocking even before fstat, and
    // O_NOFOLLOW rejects a manifest symlink instead of following a moving target.
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(E::Unreadable),
    };
    if !file.metadata().map_err(|_| E::Unreadable)?.is_file() {
        return Err(E::Unreadable);
    }
    let allowance = MAX_PACKAGE_BYTES.min(remaining);
    let mut bytes = Vec::new();
    let read_result = file.take((allowance + 1) as u64).read_to_end(&mut bytes);
    // Include the overflow sentinel in the checked charge. No oversized input
    // reaches JSON decoding or script-map retention.
    budget.read(bytes.len())?;
    read_result.map_err(|_| E::Unreadable)?;
    Ok(Some(bytes))
}
fn fingerprint(bytes: &[u8]) -> MetadataFingerprint {
    MetadataFingerprint {
        bytes: bytes.len(),
        sha256: Sha256::digest(bytes).into(),
    }
}
fn scripts(bytes: &[u8], budget: &mut MetadataStats) -> Result<BTreeMap<String, String>, E> {
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|_| E::Invalid)?;
    let object = value.as_object().ok_or(E::Invalid)?;
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

impl PackageMetadata for FreshMetadata {
    fn package(&mut self, cwd: &Path, budget: &mut MetadataStats) -> Result<PackageSnapshot, E> {
        let origin = canonical(cwd, budget)?;
        if let Some(dir) = self.origins.get(&origin) {
            return Ok(self.packages[dir].clone());
        }
        let mut directory = origin.clone();
        loop {
            if let Some(snapshot) = self.packages.get(&directory) {
                self.origins.insert(origin, directory);
                return Ok(snapshot.clone());
            }
            if let Some(bytes) = read(&directory.join("package.json"), budget)? {
                let snapshot = PackageSnapshot {
                    package_dir: directory.clone(),
                    fingerprint: fingerprint(&bytes),
                    scripts: scripts(&bytes, budget)?,
                };
                budget.file()?;
                self.origins.insert(origin, directory.clone());
                self.packages.insert(directory, snapshot.clone());
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
        // Repeat discovery as well as content checks: a newly created nearer
        // package.json must not silently change which script Bun will resolve.
        let mut checked = BTreeMap::new();
        for (origin, expected) in &self.origins {
            let mut directory = canonical(origin, budget)?;
            loop {
                if let Some(hash) = checked.get(&directory) {
                    if &directory != expected || self.packages[expected].fingerprint != *hash {
                        return Err(E::Changed);
                    }
                    break;
                }
                if let Some(bytes) = read(&directory.join("package.json"), budget)? {
                    let hash = fingerprint(&bytes);
                    if &directory != expected || self.packages[expected].fingerprint != hash {
                        return Err(E::Changed);
                    }
                    checked.insert(directory, hash);
                    break;
                }
                if !directory.pop() {
                    return Err(E::Missing);
                }
            }
        }
        for package in packages {
            if checked.get(&package.package_dir) != Some(&package.fingerprint) {
                return Err(E::Changed);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;

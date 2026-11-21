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
    // These native workload recognizers have no endpoint dependency. Do not
    // make their local admission depend on unrelated environment or clocks.
    let direct = arc_detector::classify_argv(argv);
    if direct.locality.is_local_compute() {
        return direct;
    }
    let Some(context) = super::capture_argv(argv) else {
        return local_without_endpoint_context(argv).unwrap_or_else(unknown);
    };
    let result = classify_with_context(argv, &context, || arc_ipc::locality::now_ms().unwrap_or(0));
    if super::proof::revalidate_context(&context)
        && context.is_valid_at(arc_ipc::locality::now_ms().unwrap_or(0))
    {
        result
    } else {
        unknown()
    }
}

pub(super) fn classify_with_context(
    argv: &[OsString],
    context: &arc_detector::EndpointContext,
    mut now_ms: impl FnMut() -> u64,
) -> Classification {
    let now = now_ms();
    if !context.is_valid_at(now) {
        return local_without_endpoint_context(argv).unwrap_or_else(unknown);
    }
    let contextual = arc_detector::classify_argv_with_context(argv, context, now);
    // Retain a bounded fallback if a native `sh -c` is not recognized, with
    // the outer argv charged and one nesting level reserved for the shell.
    if contextual.reason == arc_detector::Reason::UnresolvedCommand {
        if let Some(result) = bounded_sh_fallback(argv, context, now) {
            return result;
        }
    }
    if contextual.is_endpoint_locality() {
        return contextual;
    }
    // The script resolver also accepts an absolute Bun executable path, which
    // the syntax-only fallback intentionally does not recognize as a command.
    if argv
        .first()
        .and_then(|program| Path::new(program).file_name())
        .is_none_or(|name| name != "bun")
    {
        return contextual;
    }
    let Ok(cwd) = std::env::current_dir() else {
        return arc_detector::classify_argv(argv).legacy();
    };
    let mut metadata = FreshMetadata::default();
    let now = now_ms();
    if !context.is_valid_at(now) {
        return local_without_endpoint_context(argv).unwrap_or_else(unknown);
    }
    let mut result =
        arc_detector::analyze_script_argv_with_context(argv, &cwd, &mut metadata, context, now);
    if !result.packages.is_empty()
        && metadata
            .revalidate(&result.packages, &mut result.metadata)
            .is_err()
    {
        return arc_detector::classify_argv(argv).legacy();
    }
    if !context.is_valid_at(now_ms()) {
        return local_without_endpoint_context(argv).unwrap_or_else(unknown);
    }
    result.classification
}

fn unknown_with_reason(reason: arc_detector::Reason) -> Classification {
    arc_detector::Classification {
        classification: arc_detector::JobClass::Unknown,
        locality: arc_detector::Locality::Unknown,
        reason,
        uncertainty: arc_detector::Uncertainty::Observe,
    }
}

fn unknown() -> Classification {
    unknown_with_reason(arc_detector::Reason::UnresolvedCommand)
}

fn bounded_sh_fallback(
    argv: &[OsString],
    context: &arc_detector::EndpointContext,
    now: u64,
) -> Option<Classification> {
    let [program, option, source] = argv else {
        return None;
    };
    if program != "sh" || option != "-c" {
        return None;
    }
    let source = source.to_str()?;
    let bytes = argv.iter().try_fold(0usize, |size, word| {
        size.checked_add(word.as_encoded_bytes().len())
    })?;
    if bytes > arc_detector::MAX_INPUT_BYTES {
        return Some(unknown_with_reason(arc_detector::Reason::InputLimit));
    }
    let (classification, stats) = arc_detector::analyze_with_context(
        &arc_detector::ClassifyRequest {
            command: source.to_owned(),
            shell_kind: "sh".into(),
        },
        context,
        now,
    );
    // analyze_with_context already includes its own parser setup charges.
    // Adding the native argv and outer invocation is conservative even when
    // a newer detector handles this exact route directly.
    if stats.tokens.saturating_add(argv.len()) > arc_detector::MAX_TOKENS {
        return Some(unknown_with_reason(arc_detector::Reason::TokenLimit));
    }
    if stats.invocations.saturating_add(1) > arc_detector::MAX_INVOCATIONS {
        return Some(unknown_with_reason(arc_detector::Reason::InvocationLimit));
    }
    if stats.max_depth.saturating_add(1) > arc_detector::MAX_DEPTH {
        return Some(unknown_with_reason(arc_detector::Reason::NestingLimit));
    }
    if stats
        .work
        .saturating_add(bytes)
        .saturating_add(argv.len())
        .saturating_add(1)
        > arc_detector::MAX_WORK
    {
        return Some(unknown_with_reason(arc_detector::Reason::WorkLimit));
    }
    Some(classification)
}

// An unavailable endpoint snapshot cannot make a remote or mixed script local.
// The legacy graph only returns HOST_LOCAL when all supported leaves are local;
// still revalidate every consulted package before retaining that result.
fn local_without_endpoint_context(argv: &[OsString]) -> Option<Classification> {
    let direct = arc_detector::classify_argv(argv);
    if direct.locality.is_local_compute() {
        return Some(direct);
    }
    if argv
        .first()
        .and_then(|program| Path::new(program).file_name())
        .is_none_or(|name| name != "bun")
    {
        return None;
    }
    let cwd = std::env::current_dir().ok()?;
    let mut metadata = FreshMetadata::default();
    let mut result = arc_detector::analyze_script_argv(argv, &cwd, &mut metadata);
    if !result.classification.locality.is_local_compute()
        || result.packages.is_empty()
        || metadata
            .revalidate(&result.packages, &mut result.metadata)
            .is_err()
    {
        return None;
    }
    Some(result.classification)
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

//! Transient package-script analysis contracts. No filesystem or process access.
//!
//! The CLI owns bounded fresh reads; the resolver consumes literal script maps.
//! None of these paths, script bodies or fingerprints is sent over IPC or stored.
use crate::{AnalysisStats, Classification};
pub mod cache;
mod commands;
pub(crate) mod resolver;
use std::{
    collections::BTreeMap,
    fmt,
    path::{Path, PathBuf},
};

pub const MAX_PACKAGE_BYTES: usize = 262_144;
pub const MAX_METADATA_BYTES: usize = 1_048_576;
pub const MAX_PACKAGE_FILES: usize = 16;
pub const MAX_PACKAGE_LOOKUPS: usize = 64;
pub const MAX_PACKAGE_SCRIPTS: usize = 256;
pub const MAX_RETAINED_SCRIPT_BYTES: usize = 65_536;
pub const MAX_GRAPH_EDGES: usize = 128;

/// Content identity is mandatory; length/mtime alone never establishes freshness.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataFingerprint {
    pub bytes: usize,
    pub sha256: [u8; 32],
}

#[derive(Clone, PartialEq, Eq)]
pub struct PackageSnapshot {
    /// Canonical directory of this package.json, not necessarily the caller cwd.
    pub package_dir: PathBuf,
    pub scripts: BTreeMap<String, String>,
    pub fingerprint: MetadataFingerprint,
}
impl fmt::Debug for PackageSnapshot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PackageSnapshot([REDACTED])")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetadataError {
    Missing,
    Unreadable,
    Invalid,
    Changed,
    PackageBytes,
    AggregateBytes,
    FileLimit,
    LookupLimit,
    ScriptLimit,
    RetainedScriptBytes,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MetadataStats {
    pub lookups: usize,
    pub files: usize,
    pub bytes: usize,
    pub script_bytes: usize,
}
impl MetadataStats {
    /// Charge before every attempted filesystem lookup, including misses and revalidation.
    pub fn lookup(&mut self) -> Result<(), MetadataError> {
        let next = self
            .lookups
            .checked_add(1)
            .filter(|n| *n <= MAX_PACKAGE_LOOKUPS)
            .ok_or(MetadataError::LookupLimit)?;
        self.lookups = next;
        Ok(())
    }
    /// Charge before retaining a unique package. A read is capped at max+1 bytes;
    /// all bytes actually read, including revalidation, enter `read` below.
    pub fn file(&mut self) -> Result<(), MetadataError> {
        let next = self
            .files
            .checked_add(1)
            .filter(|n| *n <= MAX_PACKAGE_FILES)
            .ok_or(MetadataError::FileLimit)?;
        self.files = next;
        Ok(())
    }
    pub fn read(&mut self, bytes: usize) -> Result<(), MetadataError> {
        if bytes > MAX_PACKAGE_BYTES {
            return Err(MetadataError::PackageBytes);
        }
        let next = self
            .bytes
            .checked_add(bytes)
            .filter(|n| *n <= MAX_METADATA_BYTES)
            .ok_or(MetadataError::AggregateBytes)?;
        self.bytes = next;
        Ok(())
    }
    /// Names and bodies both count. Validate before retaining/cloning a script map.
    pub fn scripts(&mut self, scripts: &BTreeMap<String, String>) -> Result<(), MetadataError> {
        if scripts.len() > MAX_PACKAGE_SCRIPTS {
            return Err(MetadataError::ScriptLimit);
        }
        let next = scripts
            .iter()
            .try_fold(self.script_bytes, |sum, (name, body)| {
                sum.checked_add(name.len())
                    .and_then(|sum| sum.checked_add(body.len()))
                    .filter(|sum| *sum <= MAX_RETAINED_SCRIPT_BYTES)
                    .ok_or(MetadataError::RetainedScriptBytes)
            })?;
        self.script_bytes = next;
        Ok(())
    }
}

/// The adapter may do bounded I/O, but never tool lookup or command execution.
/// One adapter is created per original invocation; it may memoize only that run.
pub trait PackageMetadata {
    fn package(
        &mut self,
        cwd: &Path,
        budget: &mut MetadataStats,
    ) -> Result<PackageSnapshot, MetadataError>;
    /// Fresh bounded content reads of all consulted manifests before enqueue.
    /// A mismatch causes observe/bypass; execution still uses the original argv.
    fn revalidate(
        &mut self,
        packages: &[PackageSnapshot],
        budget: &mut MetadataStats,
    ) -> Result<(), MetadataError>;
    /// Members named by the workspace globs of the root manifest in `root`
    /// (`package.json` `workspaces`, or `pnpm-workspace.yaml`), sorted by
    /// directory. Only manifests matched by the declared globs are read; no
    /// source scan. `Ok(None)`: this adapter has no workspace analysis, so a
    /// fan-out stays UNKNOWN/OBSERVE. Revalidation must re-evaluate the
    /// same glob membership (lane C).
    fn workspace_members(
        &mut self,
        _root: &Path,
        _budget: &mut MetadataStats,
    ) -> Result<Option<Vec<WorkspaceMember>>, MetadataError> {
        Ok(None)
    }
}

/// One member package named by a workspace root's declared globs.
#[derive(Clone, PartialEq, Eq)]
pub struct WorkspaceMember {
    /// Canonical member directory (holding its package.json).
    pub dir: PathBuf,
    /// The member manifest's `name`, used by name selectors.
    pub name: Option<String>,
}
impl fmt::Debug for WorkspaceMember {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("WorkspaceMember([REDACTED])")
    }
}

/// The classification of one selected workspace member's script (lane C,
/// DET-009 item 2). Kept beside the projected result; the negotiated workspace
/// compound operation can carry these records to the daemon.
#[derive(Clone, PartialEq, Eq)]
pub struct MemberClassification {
    /// Member directory relative to the workspace root.
    pub member: PathBuf,
    pub script: String,
    /// The member's own aggregate: its class and true locality.
    pub classification: Classification,
}
impl fmt::Debug for MemberClassification {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MemberClassification")
            .field("classification", &self.classification)
            .finish_non_exhaustive()
    }
}

/// Whether `program` (an executable basename) names a package runner whose
/// invocations the script resolver analyzes: `bun`, plus the workspace
/// runners lane C adds (`npm`, `pnpm`, `yarn`).
pub fn resolves_scripts(program: &str) -> bool {
    program == "bun" || resolver::workspace::is_runner(program)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptIssue {
    Metadata(MetadataError),
    Cycle,
    Dynamic,
    Unsupported,
    Mixed,
    GraphLimit,
}

/// `analysis` counters are shared across all parsed bodies and lifecycle edges.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScriptResolution {
    pub classification: Classification,
    pub analysis: AnalysisStats,
    pub metadata: MetadataStats,
    pub graph_edges: usize,
    pub packages: Vec<PackageSnapshot>,
    pub issue: Option<ScriptIssue>,
    /// Per-member results of a workspace fan-out, in member order; empty
    /// for a single-package script. `classification` stays the existing
    /// compound projection that admission uses: one class when every member
    /// agrees, MIXED for local plus remote, otherwise UNKNOWN/OBSERVE.
    pub members: Vec<MemberClassification>,
}

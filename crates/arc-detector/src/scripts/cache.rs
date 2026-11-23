//! Project analysis cache (lane C, DET-014, SRS line 710).
//!
//! One bounded entry per project root holds the project's analysis: every
//! manifest and workspace declaration that classifications in that project
//! read, with its file identity and content digest, the nearer manifests an
//! upward lookup found absent, the declared globs and the members they
//! named, the files, absent paths and membership through which root
//! discovery selected the entry, and the results of recent classifications,
//! each keyed by its invocation directory. The workspace root, its members and their
//! subdirectories share the entry; a created nearer manifest is a miss. On
//! reuse every file is read again and compared by digest (equal bytes may
//! refresh a changed mtime or inode) and the declared globs are re-evaluated,
//! so a changed manifest, script or membership is a miss. A classification
//! whose own result is not yet recorded is analyzed over the validated bytes,
//! with other selectors and scripts of the same project sharing the entry.
//!
//! The types here are pure; the CLI adapter persists them in a user-owned
//! state directory independent of database overrides, or in daemon memory,
//! never in /tmp or a path derived from TMPDIR. An entry holds relative manifest paths, file identities, content
//! digests, the declared globs, opaque result keys and classifications: no
//! absolute path, script body or endpoint value. Diagnostics report a hit or
//! a miss.
use super::{resolver::workspace, MemberClassification, MAX_PACKAGE_FILES};
use crate::{budget::Budget, Classification};
use serde::{Deserialize, Serialize};

/// Entries kept in one cache directory; the oldest beyond this are removed.
pub const MAX_ENTRIES: usize = 64;
/// Largest serialized entry read or written.
pub const MAX_ENTRY_BYTES: usize = 262_144;
/// Files validated per entry: every retained manifest plus both workspace
/// declarations, a runner's and the one root discovery also reads.
pub const MAX_ENTRY_FILES: usize = MAX_PACKAGE_FILES + 2;
/// Recorded classification results per project; the oldest are dropped.
pub const MAX_RESULTS: usize = 32;
pub const ENTRY_VERSION: u32 = 5;
/// Absent manifest paths recorded per entry.
pub const MAX_ENTRY_ABSENT: usize = super::MAX_PACKAGE_LOOKUPS;

/// Whether `words` is a workspace fan-out this cache can hold. Pure syntax.
pub fn is_fan_out(words: &[String]) -> bool {
    let cwd = std::path::Path::new(".");
    !matches!(
        workspace::parse(words, cwd, &mut Budget::default()),
        Ok(None)
    )
}

/// The parsed parts of a supported fan-out that its caller decides on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FanOutSyntax {
    pub script: String,
    /// Passthrough arguments follow the script name.
    pub has_args: bool,
    /// npm's explicit `include-workspace-root`, when on the command line.
    pub include_root: Option<bool>,
}

/// The parts of a supported fan-out; `None` for any other command or an
/// unsupported fan-out shape.
pub fn fan_out_syntax(words: &[String]) -> Option<FanOutSyntax> {
    let cwd = std::path::Path::new(".");
    let fan_out = workspace::parse(words, cwd, &mut Budget::default()).ok()??;
    Some(FanOutSyntax {
        script: fan_out.script().to_owned(),
        has_args: fan_out.has_args(),
        include_root: fan_out.include_root(),
    })
}

/// The stat identity compared before any content digest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileIdentity {
    pub dev: u64,
    pub ino: u64,
    pub size: u64,
    pub mtime_ns: i128,
    pub ctime_ns: i128,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CachedFile {
    /// Relative to the workspace root, `/`-separated, never `..`.
    pub path: String,
    pub identity: FileIdentity,
    /// Lower-case hex SHA-256 of the bytes the analysis read.
    pub sha256: String,
}

/// The members one declaration's globs named when the entry was written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Membership {
    /// `package.json` or `pnpm-workspace.yaml`, relative to the root.
    pub declaration: String,
    pub globs: Vec<String>,
    /// Relative member directories, sorted.
    pub members: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CachedMember {
    /// Relative member directory, or `.` for a participating root.
    pub member: String,
    pub script: String,
    pub classification: Classification,
}

/// One classification over the entry's files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CachedResult {
    /// Lower-case hex digest of the invocation and its decision facts.
    pub key: String,
    pub classification: Classification,
    pub members: Vec<CachedMember>,
}
impl CachedResult {
    pub fn member_classifications(&self) -> Vec<MemberClassification> {
        self.members
            .iter()
            .map(|m| MemberClassification {
                member: m.member.clone().into(),
                script: m.script.clone(),
                classification: m.classification,
            })
            .collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub version: u32,
    pub files: Vec<CachedFile>,
    pub memberships: Vec<Membership>,
    /// Relative manifest paths an upward package lookup found absent, so a
    /// newly created nearer manifest is a miss.
    #[serde(default)]
    pub absent: Vec<String>,
    /// Oldest first.
    pub results: Vec<CachedResult>,
}

fn hex64(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f'))
}

impl Entry {
    /// Reject an entry that this version did not write or that is out of
    /// bounds, before any of it is used or any file it names is opened.
    pub fn is_well_formed(&self) -> bool {
        let relative = |path: &str| {
            !path.is_empty()
                && !path.starts_with('/')
                && path.len() <= 4096
                && path.split('/').all(|part| {
                    !part.is_empty() && part != "." && part != ".." && !part.contains('\0')
                })
        };
        self.version == ENTRY_VERSION
            && !self.files.is_empty()
            && self.files.len() <= MAX_ENTRY_FILES
            && self
                .files
                .iter()
                .all(|f| relative(&f.path) && hex64(&f.sha256))
            && self.absent.len() <= MAX_ENTRY_ABSENT
            && self.absent.iter().all(|p| relative(p))
            && !self
                .absent
                .iter()
                .any(|p| self.files.iter().any(|f| &f.path == p))
            && self.memberships.len() <= MAX_ENTRY_FILES
            && self.memberships.iter().all(|m| {
                relative(&m.declaration)
                    && m.members.len() <= MAX_PACKAGE_FILES
                    && m.members.iter().all(|p| relative(p))
            })
            && self.results.len() <= MAX_RESULTS
            && self.results.iter().all(|r| {
                hex64(&r.key)
                    && r.classification.is_consistent()
                    && r.members.len() <= MAX_PACKAGE_FILES + 1
                    && r.members.iter().all(|m| {
                        (m.member == "." || relative(&m.member)) && m.classification.is_consistent()
                    })
            })
    }
    pub fn result(&self, key: &str) -> Option<&CachedResult> {
        self.results.iter().find(|r| r.key == key)
    }
    /// Record `result`, replacing one with the same key and dropping the
    /// oldest beyond [`MAX_RESULTS`].
    pub fn record(&mut self, result: CachedResult) {
        self.results.retain(|r| r.key != result.key);
        self.results.push(result);
        let excess = self.results.len().saturating_sub(MAX_RESULTS);
        self.results.drain(..excess);
    }
}
/// How a classification obtained its project analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// A validated project entry was reused.
    Hit,
    /// The project was analyzed afresh.
    Miss,
    /// No cache location is available, so nothing was reused or kept.
    None,
}
impl Outcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hit => "hit",
            Self::Miss => "miss",
            Self::None => "none",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{JobClass, Reason};

    fn entry() -> Entry {
        Entry {
            version: ENTRY_VERSION,
            files: vec![CachedFile {
                path: "package.json".into(),
                identity: FileIdentity {
                    dev: 1,
                    ino: 2,
                    size: 3,
                    mtime_ns: 4,
                    ctime_ns: 5,
                },
                sha256: "0".repeat(64),
            }],
            memberships: vec![Membership {
                declaration: "package.json".into(),
                globs: vec!["packages/*".into()],
                members: vec!["packages/a".into()],
            }],
            absent: vec!["packages/a/src/package.json".into()],
            results: vec![CachedResult {
                key: "a".repeat(64),
                classification: Classification::new(JobClass::Test, Reason::BunScriptTest),
                members: vec![
                    CachedMember {
                        member: ".".into(),
                        script: "test".into(),
                        classification: Classification::new(JobClass::Test, Reason::BunScriptTest),
                    },
                    CachedMember {
                        member: "packages/a".into(),
                        script: "test".into(),
                        classification: Classification::new(JobClass::Test, Reason::BunScriptTest),
                    },
                ],
            }],
        }
    }

    #[test]
    fn only_bounded_relative_entries_of_this_version_are_well_formed() {
        assert!(entry().is_well_formed());
        let cases: [fn(&mut Entry); 15] = [
            |e: &mut Entry| e.absent[0] = "../package.json".into(),
            |e: &mut Entry| e.absent = vec![e.files[0].path.clone()],
            |e: &mut Entry| e.absent = vec!["a/package.json".into(); MAX_ENTRY_ABSENT + 1],
            |e: &mut Entry| e.version = ENTRY_VERSION + 1,
            |e: &mut Entry| e.files.clear(),
            |e: &mut Entry| e.files[0].path = "/etc/passwd".into(),
            |e: &mut Entry| e.files[0].path = "../package.json".into(),
            |e: &mut Entry| e.files[0].path = "a//b".into(),
            |e: &mut Entry| e.files[0].sha256 = "Z".repeat(64),
            |e: &mut Entry| e.memberships[0].members[0] = "./a".into(),
            |e: &mut Entry| e.results[0].members[1].member = "..".into(),
            |e: &mut Entry| e.results[0].members[1].member = "a/./b".into(),
            |e: &mut Entry| e.results[0].key = "A".repeat(64),
            |e: &mut Entry| {
                e.results[0].classification.uncertainty = crate::Uncertainty::Observe;
            },
            |e: &mut Entry| {
                e.files = vec![e.files[0].clone(); MAX_ENTRY_FILES + 1];
            },
        ];
        for broken in cases {
            let mut e = entry();
            broken(&mut e);
            assert!(!e.is_well_formed(), "{e:?}");
        }
        // Results are bounded, oldest first, and replaced by key.
        let mut e = entry();
        let template = e.results[0].clone();
        for at in 0..MAX_RESULTS + 3 {
            let mut r = template.clone();
            r.key = format!("{at:064x}");
            e.record(r);
        }
        assert_eq!(e.results.len(), MAX_RESULTS);
        assert_eq!(e.results[0].key, format!("{:064x}", 3));
        assert!(e.result(&"a".repeat(64)).is_none() && e.is_well_formed());
        e.record(e.results[5].clone());
        assert_eq!(e.results.len(), MAX_RESULTS);
        assert_eq!(e.results.last().unwrap().key, format!("{:064x}", 8));
        assert_eq!(e.result(&format!("{:064x}", 8)).unwrap().members.len(), 2);
    }

    #[test]
    fn fan_out_detection_is_syntax_only_and_leaves_other_commands_alone() {
        let words = |s: &str| s.split(' ').map(str::to_owned).collect::<Vec<_>>();
        for fan in [
            "bun run --filter * test",
            "bun --filter=./packages/a build",
            "npm run test --workspaces",
            "npm run test -w a",
            "pnpm -r run test",
            "pnpm --filter a run test",
            "yarn workspaces foreach -A run test",
            // Recognized but unsupported shapes are still fan-outs: UNKNOWN.
            "pnpm -r test",
            "yarn workspaces foreach -R run test",
        ] {
            assert!(is_fan_out(&words(fan)), "{fan}");
        }
        for other in [
            "bun run test",
            "bun test",
            "bun run --workspaces build",
            "npm run test",
            "npm test",
            "pnpm run test",
            "pnpm install",
            "yarn run test",
            "cargo test",
        ] {
            assert!(!is_fan_out(&words(other)), "{other}");
        }
        let syntax = |s: &str| fan_out_syntax(&words(s));
        assert_eq!(
            syntax("npm run ci --workspaces --include-workspace-root"),
            Some(FanOutSyntax {
                script: "ci".into(),
                has_args: false,
                include_root: Some(true)
            })
        );
        assert_eq!(
            syntax("npm run ci -w a --no-include-workspace-root")
                .unwrap()
                .include_root,
            Some(false)
        );
        assert_eq!(syntax("pnpm -r verify:ci").unwrap().script, "verify:ci");
        assert!(syntax("bun run --filter * test -- -x").unwrap().has_args);
        assert_eq!(syntax("pnpm -r install"), None);
        assert_eq!(syntax("bun run test"), None);
        assert_eq!(Outcome::Hit.as_str(), "hit");
        assert_eq!(Outcome::Miss.as_str(), "miss");
        assert_eq!(Outcome::None.as_str(), "none");
    }
}

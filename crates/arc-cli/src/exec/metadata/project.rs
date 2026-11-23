//! Project script and workspace analysis (lane C: DET-009 item 2, DET-014),
//! shared by `arcctl exec` and `arcctl classify`.
//!
//! Both commands include this file, so one persistent cache serves both. The
//! cache holds one entry per project root, which bounded metadata reads
//! discover from the invocation directory (`root.rs`): a workspace root, its
//! member directories and their subdirectories share one entry. It records
//! the files that classifications in the project read, the nearer manifests
//! their upward lookups found absent, the workspace memberships they listed,
//! the discovery facts that selected the root, and recent results keyed by
//! the invocation and its directory relative to the root, as native path
//! bytes, so a result never answers another directory. When discovery's
//! facts cannot be recorded, as for a directory whose name is not UTF-8,
//! nothing is reused or kept. A reuse reads every file again and compares
//! its content digest
//! (equal bytes can refresh a changed mtime or inode) and re-evaluates the
//! declared globs, so an added or removed member is a miss. A recorded
//! result for the same invocation and decision facts is then confirmed and
//! reused. Any other supported classification in the project, whether
//! another selector, runner or ordinary script, is analyzed over the
//! validated bytes and is a hit unless it needs a file the entry does not
//! hold. Either way the consulted manifests and the membership are checked
//! once more at the decision boundary, as is the npm configuration that
//! decides root participation. Classification executes nothing.
#[path = "fresh.rs"]
pub(crate) mod fresh;
#[path = "members.rs"]
mod members;
#[path = "npm_config.rs"]
mod npm_config;
#[path = "observation.rs"]
mod observation;
#[path = "remote.rs"]
mod remote;
#[path = "root.rs"]
mod root;
#[path = "store.rs"]
mod store;
#[path = "validate.rs"]
mod validate;

use arc_detector::{
    scripts::{
        cache::{self, CachedMember, CachedResult, Outcome},
        MemberClassification, PackageMetadata, ScriptIssue, ScriptResolution,
    },
    Classification, EndpointContext,
};
use fresh::FreshMetadata;
use sha2::{Digest, Sha256};
use std::{ffi::OsString, path::Path};

/// `exec` admits on the projection; diagnostics can negotiate member details.
#[allow(dead_code)]
pub(crate) struct Analysis {
    pub(crate) classification: Classification,
    pub(crate) members: Vec<MemberClassification>,
    pub(crate) cache: Outcome,
}

fn words(argv: &[OsString]) -> Option<Vec<String>> {
    argv.iter().map(|a| a.to_str().map(str::to_owned)).collect()
}

/// Whether `argv` is a workspace fan-out of a supported runner.
pub(crate) fn is_fan_out(argv: &[OsString]) -> bool {
    words(argv).is_some_and(|w| cache::is_fan_out(&w))
}

/// Only supported script invocations can contribute reusable project analysis.
pub(crate) fn supports(argv: &[OsString]) -> bool {
    if is_fan_out(argv) {
        return true;
    }
    words(argv).is_some_and(|w| {
        w.first()
            .is_some_and(|p| Path::new(p).file_name().is_some_and(|n| n == "bun"))
            && w.get(1).is_some_and(|arg| arg == "run")
    })
}

/// Analyze a supported invocation through the user-owned or in-memory cache.
/// `None` when `argv` is not one, so the caller's path applies.
pub(crate) fn analyze(
    argv: &[OsString],
    cwd: &Path,
    context: Option<(&EndpointContext, u64)>,
) -> Option<Analysis> {
    if !supports(argv) {
        return None;
    }
    Some(analyze_in(store::Store::open(), argv, cwd, context))
}

/// The invocation directory relative to the project root as native path
/// bytes, empty for the root itself: every directory, including one whose
/// name is not UTF-8, has its own key. `None` outside the root.
fn directory_key(root: &Path, cwd: &Path) -> Option<Vec<u8>> {
    Some(
        cwd.strip_prefix(root)
            .ok()?
            .as_os_str()
            .as_encoded_bytes()
            .to_vec(),
    )
}

/// The key of one result: the invocation directory's [`directory_key`], the
/// analyzed invocation, including the npm root-participation flag its
/// configuration decided, and the endpoint facts without their capture time.
fn result_key(directory: &[u8], words: &[String], context: Option<&EndpointContext>) -> String {
    let mut hash = Sha256::new();
    let mut frame = |bytes: &[u8]| {
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    };
    frame(b"arc.analysis-result.v3");
    frame(directory);
    for word in words {
        frame(word.as_bytes());
    }
    match context {
        Some(context) => {
            let mut facts = context.clone();
            facts.captured_at_ms = 0;
            frame(b"context");
            frame(&serde_json::to_vec(&facts).unwrap_or_default());
        }
        None => frame(b"no-context"),
    }
    validate::hex(&hash.finalize())
}

fn unresolved(cache: Outcome) -> Analysis {
    Analysis {
        classification: Classification {
            classification: arc_detector::JobClass::Unknown,
            locality: arc_detector::Locality::Unknown,
            reason: arc_detector::Reason::UnresolvedScript,
            uncertainty: arc_detector::Uncertainty::Observe,
        },
        members: Vec::new(),
        cache,
    }
}

fn analyze_in(
    store: Option<store::Store>,
    argv: &[OsString],
    cwd: &Path,
    context: Option<(&EndpointContext, u64)>,
) -> Analysis {
    let original = words(argv).unwrap_or_default();
    // The project root names the entry; the invocation directory keys the
    // result inside it. The discovery facts that selected the root join the
    // entry; when they cannot be recorded, as for a directory whose name is
    // not UTF-8, nothing is reused or kept.
    let located = cwd.canonicalize().ok().and_then(|cwd| {
        let discovery = root::discover(&cwd);
        let directory = directory_key(&discovery.root, &cwd)?;
        let discovered = validate::discovered(discovery.observed.as_ref()?, &discovery.root)?;
        Some((discovery.root, directory, discovered))
    });
    let none = validate::Parts::default();
    let (directory, discovered) = match &located {
        Some((_, directory, discovered)) => (directory.clone(), discovered),
        None => (Vec::new(), &none),
    };
    let slot = store.zip(located.as_ref().map(|(root, ..)| root.clone()));
    let unavailable = if slot.is_some() {
        Outcome::Miss
    } else {
        Outcome::None
    };
    // Selector facts: npm's root participation, as an explicit flag.
    let npm = npm_config::resolve(&original, cwd);
    if npm == npm_config::Npm::Ambiguous {
        return unresolved(unavailable);
    }
    let words = npm.rewrite(&original);
    let analyzed: Vec<OsString> = words.iter().map(OsString::from).collect();
    // An expired context is never answered from the cache.
    let fresh_context = context.is_none_or(|(c, now)| c.is_valid_at(now));
    let key = result_key(&directory, &words, context.map(|(c, _)| c));
    let mut cached = None;
    if let Some((store, root)) = slot.as_ref().filter(|_| fresh_context) {
        if let Some(mut entry) = store.load(&store::key(root)) {
            if let Some((overlay, refreshed)) = validate::read(&mut entry, root) {
                let held = validate::holds(&entry, discovered);
                if let Some(recorded) = entry.result(&key).cloned().filter(|_| held) {
                    if validate::confirm(&entry, root) && npm_config::resolve(&original, cwd) == npm
                    {
                        if refreshed {
                            store.save(&store::key(root), &entry);
                        }
                        observation::report("recorded", &recorded.classification);
                        return Analysis {
                            classification: recorded.classification,
                            members: recorded.member_classifications(),
                            cache: Outcome::Hit,
                        };
                    }
                } else {
                    cached = Some((entry, overlay, held));
                }
            }
        }
    }
    let program = &words[0];
    let source = || match &cached {
        Some((_, overlay, _)) => FreshMetadata::with_overlay(program, overlay.clone()),
        None => FreshMetadata::for_program(program),
    };
    let mut metadata = source();
    let mut result = run(&analyzed, cwd, &mut metadata, context);
    // A narrowly provable remote member is checked independently of the
    // caller directory. Never lend the outer runner's endpoint facts to it.
    // The graph retains per-member leaves even when its projection is
    // unresolved; only replace that projection after every remote leaf has
    // its own fresh, repeated proof.
    let fan_out = cache::is_fan_out(&words);
    let mut remote_proof = false;
    if fresh_context
        && context.is_some()
        && fan_out
        && result.classification.classification == arc_detector::JobClass::Unknown
    {
        let mut candidate_metadata = source();
        let mut candidate = run(&analyzed, cwd, &mut candidate_metadata, None);
        if remote::prove_mixed_members(&candidate, &words, cwd) {
            candidate.classification = Classification {
                classification: arc_detector::JobClass::Mixed,
                locality: arc_detector::Locality::Mixed,
                reason: arc_detector::Reason::CompoundMixed,
                uncertainty: arc_detector::Uncertainty::None,
            };
            result = candidate;
            metadata = candidate_metadata;
            remote_proof = true;
        }
    }
    // A hit reused only validated bytes: no fresh manifest, and every file
    // discovery read already in the entry.
    let outcome = match (&cached, &slot) {
        (Some((.., held)), _) if *held && !metadata.extended => Outcome::Hit,
        (_, Some(_)) => Outcome::Miss,
        _ => Outcome::None,
    };
    // The decision boundary: fresh reads of every consulted manifest and of
    // the membership, then the selector facts once more.
    if !result.packages.is_empty()
        && metadata
            .revalidate(&result.packages, &mut result.metadata)
            .is_err()
    {
        // A manifest changed during analysis: the legacy syntax-only answer.
        return Analysis {
            classification: arc_detector::classify_argv(argv).legacy(),
            members: Vec::new(),
            cache: outcome,
        };
    }
    if npm_config::resolve(&original, cwd) != npm {
        return unresolved(outcome);
    }
    if let Some((store, root)) = slot.as_ref().filter(|_| fresh_context) {
        let parts = validate::parts(&metadata, &result, root)
            .and_then(|parts| parts.join(discovered.clone()));
        if let Some(parts) = parts {
            let mut entry = validate::merge(cached.map(|(entry, ..)| entry), parts);
            // A remote proof includes CLI configuration outside the project,
            // and a contextual fan-out the resolver refused may be provable
            // later: neither result is reused from the project's entry.
            let reusable = !(remote_proof
                || context.is_some() && fan_out && result.issue == Some(ScriptIssue::Unsupported));
            if let Some(recorded) = reusable.then(|| record(&key, &result)).flatten() {
                entry.record(recorded);
            }
            validate::fit(&mut entry);
            store.save(&store::key(root), &entry);
        }
    }
    observation::report("computed", &result.classification);
    Analysis {
        classification: result.classification,
        members: result.members,
        cache: outcome,
    }
}

fn run(
    argv: &[OsString],
    cwd: &Path,
    metadata: &mut FreshMetadata,
    context: Option<(&EndpointContext, u64)>,
) -> ScriptResolution {
    match context {
        Some((context, now)) => {
            arc_detector::analyze_script_argv_with_context(argv, cwd, metadata, context, now)
        }
        None => arc_detector::analyze_script_argv(argv, cwd, metadata),
    }
}

fn record(key: &str, result: &ScriptResolution) -> Option<CachedResult> {
    Some(CachedResult {
        key: key.to_owned(),
        classification: result.classification,
        members: result
            .members
            .iter()
            .map(|m| {
                Some(CachedMember {
                    member: m.member.to_str()?.to_owned(),
                    script: m.script.clone(),
                    classification: m.classification,
                })
            })
            .collect::<Option<_>>()?,
    })
}

#[cfg(test)]
#[path = "discovery_tests.rs"]
mod discovery_tests;
#[cfg(test)]
#[path = "project_tests.rs"]
mod tests;

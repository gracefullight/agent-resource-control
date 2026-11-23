//! Fresh, bounded package metadata at the execution boundary. No tool probes.
//!
//! The adapter itself lives in `metadata/fresh.rs`, inside the project
//! analysis that `arcctl classify` shares (lane C).
use arc_detector::{scripts::PackageMetadata, Classification};
use std::{ffi::OsString, path::Path};
#[cfg(test)]
use {
    arc_detector::scripts::{
        MetadataError as E, MetadataStats, MAX_METADATA_BYTES, MAX_PACKAGE_BYTES,
    },
    std::{collections::BTreeMap, fs::OpenOptions, path::PathBuf},
};

mod project;
pub(super) use project::fresh::FreshMetadata;

pub(super) fn classify(argv: &[OsString]) -> Classification {
    super::workspace_compound::clear();
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
        super::workspace_compound::clear();
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
        .and_then(|name| name.to_str())
        .is_none_or(|name| !arc_detector::scripts::resolves_scripts(name))
    {
        return contextual;
    }
    let Ok(cwd) = std::env::current_dir() else {
        return arc_detector::classify_argv(argv).legacy();
    };
    let now = now_ms();
    if !context.is_valid_at(now) {
        return local_without_endpoint_context(argv).unwrap_or_else(unknown);
    }
    // A workspace fan-out goes through the shared project analysis cache.
    if let Some(analysis) = project::analyze(argv, &cwd, Some((context, now))) {
        if !context.is_valid_at(now_ms()) {
            super::workspace_compound::clear();
            return local_without_endpoint_context(argv).unwrap_or_else(unknown);
        }
        super::workspace_compound::capture(&analysis.members, analysis.classification);
        return analysis.classification;
    }
    let mut metadata = FreshMetadata::default();
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
        .and_then(|name| name.to_str())
        .is_none_or(|name| !arc_detector::scripts::resolves_scripts(name))
    {
        return None;
    }
    let cwd = std::env::current_dir().ok()?;
    if let Some(analysis) = project::analyze(argv, &cwd, None) {
        let local = analysis.classification;
        let compound = super::workspace_compound::capture(&analysis.members, local);
        return (local.locality.is_local_compute() || compound).then_some(local);
    }
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

#[cfg(test)]
mod tests;

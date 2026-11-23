//! Workspace analysis of `arcctl classify` (lane C, DET-009 item 2, DET-014).
//!
//! The daemon classifies shell syntax only; it has no filesystem context of
//! the caller. For supported package scripts and workspace fan-outs, [`refine`]
//! resolves scripts CLI-side with bounded fresh manifest reads through the
//! same project analysis, adapter and cache as `arcctl exec`, with the same
//! endpoint-context rules, and returns the projected classification together
//! with the diagnostics shown under [`DIAGNOSTICS_KEY`] by `--json
//! --diagnostics`: `{"cache":"hit"|"miss"|"none","members":[{"member",
//! "script","classification","locality","reason","uncertainty"}]}`.
//! Classification executes nothing. The negotiated compound carries bounded
//! relative member names and script names, never absolute paths or script bodies.
#[allow(dead_code, clippy::duplicate_mod)] // Also the analysis of `arcctl exec`.
#[path = "../exec/metadata/project.rs"]
mod project;

use arc_ipc::{Classification, ClassifyRequest, JobClass, Locality, Reason, Uncertainty};
use std::{ffi::OsString, path::Path};

pub(super) const DIAGNOSTICS_KEY: &str = "analysis";

/// The result to show and its diagnostics; `None` when this command is not
/// a supported script invocation, so `daemon` stands unchanged.
pub(super) fn refine(
    socket: &Path,
    input: &ClassifyRequest,
    daemon: Classification,
    include_compound: bool,
) -> (Classification, Option<serde_json::Value>) {
    let Some(argv) = arc_detector::capture_shell_argv(input)
        .filter(|invocations| invocations.len() == 1)
        .and_then(|mut invocations| invocations.pop())
    else {
        return (daemon, None);
    };
    if !project::supports(&argv) {
        return (daemon, None);
    }
    let Ok(cwd) = std::env::current_dir() else {
        return (unknown(), Some(diagnostics(None)));
    };
    let analysis = classify(&argv, &cwd, arc_ipc::locality::now_ms);
    let mut details = diagnostics(Some(&analysis));
    if include_compound {
        if let Some(compound) =
            crate::exec::workspace_compound::candidate(&analysis.members, analysis.classification)
                .and_then(|candidate| {
                    crate::exec::workspace_compound::negotiate(socket, &candidate, None)
                })
        {
            details["compound"] = serde_json::to_value(compound).unwrap_or_default();
        }
    }
    (analysis.classification, Some(details))
}

/// The admission rules of `arcctl exec` for the same argv: contextual
/// analysis while the captured facts are valid and revalidate, otherwise
/// only a local result.
fn classify(argv: &[OsString], cwd: &Path, now_ms: impl Fn() -> Option<u64>) -> project::Analysis {
    let now = || now_ms().unwrap_or(0);
    if let Some(context) = crate::exec::capture_argv(argv).filter(|c| c.is_valid_at(now())) {
        let Some(mut analysis) = project::analyze(argv, cwd, Some((&context, now()))) else {
            return unresolved();
        };
        if context.is_valid_at(now()) {
            if !crate::exec::revalidate_context(&context) {
                analysis.classification = unknown();
            }
            return analysis;
        }
    }
    let Some(mut analysis) = project::analyze(argv, cwd, None) else {
        return unresolved();
    };
    if !analysis.classification.locality.is_local_compute() {
        analysis.classification = unknown();
    }
    analysis
}

fn diagnostics(analysis: Option<&project::Analysis>) -> serde_json::Value {
    let Some(analysis) = analysis else {
        return serde_json::json!({"cache": "none", "members": []});
    };
    let members: Vec<_> = analysis
        .members
        .iter()
        .map(|m| {
            let mut value = serde_json::to_value(m.classification).unwrap_or_default();
            value["member"] = m.member.display().to_string().into();
            value["script"] = m.script.clone().into();
            value
        })
        .collect();
    serde_json::json!({"cache": analysis.cache.as_str(), "members": members})
}

fn unresolved() -> project::Analysis {
    project::Analysis {
        classification: unknown(),
        members: Vec::new(),
        cache: arc_detector::scripts::cache::Outcome::None,
    }
}

fn unknown() -> Classification {
    Classification {
        classification: JobClass::Unknown,
        locality: Locality::Unknown,
        reason: Reason::UnresolvedCommand,
        uncertainty: Uncertainty::Observe,
    }
}

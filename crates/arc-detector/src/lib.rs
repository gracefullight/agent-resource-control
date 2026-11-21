//! Pure syntax classification. No tool lookup, evaluation, I/O or persistence.
mod argv;
pub use argv::{analyze_argv, classify_argv};
mod budget;
mod compute_location;
mod lexer;
mod locality;
pub mod native;
mod parser;
mod python;
mod remote_ci;
mod remote_endpoints;
mod rules;
pub mod scripts;
pub use scripts::resolver::{analyze_script_argv, analyze_script_argv_with_context};
mod types;

pub use budget::{
    AnalysisStats, MAX_DEPTH, MAX_INPUT_BYTES, MAX_INVOCATIONS, MAX_TOKENS, MAX_WORK,
};
pub use locality::{
    analyze_argv_with_context, analyze_with_context, capture_shell_argv,
    classify_argv_with_context, classify_with_context, EndpointContext, EndpointFact, EndpointKind,
    CONTEXT_ENV_KEYS, MAX_CONTEXT_AGE_MS, MAX_CONTEXT_BYTES, MAX_CONTEXT_ENTRIES,
    MAX_CONTEXT_VALUE_BYTES,
};
pub use types::{Classification, ClassifyRequest, JobClass, Locality, Reason, Uncertainty};

/// Classify a literal command within the documented restricted shell grammar.
pub fn classify(request: &ClassifyRequest) -> Classification {
    analyze(request).0
}

/// Deterministic counters for regression evidence. Contains no input-derived data.
pub fn analyze(request: &ClassifyRequest) -> (Classification, AnalysisStats) {
    let mut budget = AnalysisStats::default();
    if request.command.len() > MAX_INPUT_BYTES {
        return (Classification::unknown(Reason::InputLimit), budget);
    }
    let result = (|| {
        if !budget.one_of(&request.shell_kind, &["sh", "bash", "zsh"])? {
            return Ok(Classification::unknown(Reason::UnsupportedShell));
        }
        let mut leaves = Vec::new();
        parser::Parser::new(&request.command).parse(0, &mut budget, &mut leaves)?;
        aggregate(&leaves, &mut budget)
    })();
    (result.unwrap_or_else(Classification::unknown), budget)
}

fn aggregate(leaves: &[Classification], b: &mut budget::Budget) -> budget::Checked<Classification> {
    let Some(first) = leaves.first().copied() else {
        return Ok(Classification::unknown(Reason::EmptyCommand));
    };
    let mut unknown = None;
    let (mut local, mut remote, mut same) = (false, false, true);
    for leaf in leaves {
        b.charge(1)?;
        if leaf.classification == JobClass::Unknown && unknown.is_none() {
            unknown = Some(*leaf);
        }
        local |= leaf.locality.is_local_compute();
        remote |= leaf.locality == Locality::Remote;
        same &= leaf.classification == first.classification;
    }
    Ok(if let Some(unknown) = unknown {
        unknown
    } else if leaves.len() == 1 {
        first
    } else if local && remote {
        Classification::new(JobClass::Mixed, Reason::CompoundMixed)
    } else if same {
        Classification::new(
            first.classification,
            if local {
                Reason::CompoundLocal
            } else {
                Reason::CompoundRemote
            },
        )
    } else {
        Classification::unknown(Reason::UnresolvedCompound)
    })
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod coverage_tests;

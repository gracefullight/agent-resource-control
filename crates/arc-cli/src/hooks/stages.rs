//! Mixed command stages on the hook route (AT-TRF-12, DET-016).
//!
//! Exactly `bun test && gh workflow run ci.yml` and `make test && gh workflow
//! run ci.yml` are rewritten to `<arcctl> hook-exec --arc-request <id> --
//! <stage 1> && <arcctl> hook-exec --arc-request <id> -- <stage 2>`, with one
//! fresh request id per rewrite, and exactly their three stage argv are
//! admitted by `hook-exec`. The vendor's shell keeps `&&`; arcctl never
//! schedules a stage, and the hook resolves and launches nothing.
#[allow(clippy::duplicate_mod)] // Also the execution route's definition.
#[path = "stages/compound.rs"]
mod compound;

use super::{HookContext, VerifiedSurface};
use std::ffi::OsString;

/// Stage argv accepted by `hook-exec` in addition to the existing literals.
pub(super) fn literal_argv(argv: &[OsString]) -> bool {
    compound::stage_argv(argv)
}

/// The per-stage rewrite of one separable compound, or `None` to leave the
/// source to today's pass-through. Classification never executes a stage.
pub(super) fn rewrite(
    command: &str,
    context: &HookContext,
    surface: Option<&VerifiedSurface>,
) -> Option<String> {
    compound::stages(command)?;
    surface?;
    let executable = context.executable.as_deref()?;
    if !super::is_executable(executable) {
        return None;
    }
    let request = arc_core::process_identity::random_job_id().ok()?;
    compound::rewrite(command, executable, &request)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context() -> HookContext {
        HookContext {
            executable: std::env::current_exe().ok(),
        }
    }

    #[test]
    fn unseparable_compounds_are_never_rewritten_into_stages() {
        let context = context();
        let surface = VerifiedSurface;
        for source in [
            "bun test $(printf x) && gh workflow run ci.yml",
            "bun test && gh workflow run ci.yml > dispatch.log",
            "bun test && gh workflow run \"$WORKFLOW\"",
            "bash -lc 'bun test && gh workflow run ci.yml'",
            "make test && gh workflow run ci.yml; `date`",
        ] {
            assert_eq!(rewrite(source, &context, Some(&surface)), None, "{source}");
            assert_eq!(
                super::super::rewrite_literal(source, &context, Some(&surface)),
                None,
                "{source}"
            );
        }
        // A whole compound is never one stage argv.
        for argv in [
            vec!["bash", "-c", "bun test && gh workflow run ci.yml"],
            vec!["bun", "test", "&&", "gh", "workflow", "run", "ci.yml"],
        ] {
            let argv: Vec<OsString> = argv.into_iter().map(OsString::from).collect();
            assert!(!literal_argv(&argv) && !super::super::literal_argv(&argv));
        }
    }

    #[test]
    fn the_two_compounds_are_rewritten_per_stage_with_one_fresh_request() {
        let context = context();
        let surface = VerifiedSurface;
        let executable = context.executable.clone().unwrap();
        let quoted = format!("'{}'", executable.to_str().unwrap().replace('\'', "'\\''"));
        let mut requests = Vec::new();
        for (source, test) in [
            ("bun test && gh workflow run ci.yml", "bun test"),
            ("make test && gh workflow run ci.yml", "make test"),
        ] {
            let rewritten =
                super::super::rewrite_literal(source, &context, Some(&surface)).unwrap();
            let prefix = format!("{quoted} hook-exec --arc-request ");
            let request = &rewritten[prefix.len()..prefix.len() + 32];
            assert_eq!(
                rewritten,
                format!(
                    "{prefix}{request} -- {test} && {prefix}{request} -- gh workflow run ci.yml"
                )
            );
            assert!(arc_ipc::request::RequestId::try_from(request.to_owned()).is_ok());
            requests.push(request.to_owned());
        }
        assert_ne!(
            requests[0], requests[1],
            "each rewrite names its own request"
        );
        for stage in [
            &["bun", "test"][..],
            &["make", "test"],
            &["gh", "workflow", "run", "ci.yml"],
        ] {
            let argv: Vec<OsString> = stage.iter().map(OsString::from).collect();
            assert!(super::super::literal_argv(&argv), "{stage:?}");
        }
    }

    #[test]
    fn rewrite_fails_open_without_a_proven_surface_or_usable_executable() {
        let surface = VerifiedSurface;
        let source = "bun test && gh workflow run ci.yml";
        assert_eq!(rewrite(source, &context(), None), None);
        for executable in [
            None,
            Some("arcctl".into()),
            Some("/nonexistent/arcctl".into()),
            Some(std::env::temp_dir()),
        ] {
            let context = HookContext { executable };
            assert_eq!(rewrite(source, &context, Some(&surface)), None);
        }
    }
}

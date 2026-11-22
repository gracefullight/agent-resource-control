//! Agent Test commands on the hook route (AT-TRF-05, AT-TRF-06).
//!
//! Registered from `hooks.rs`, after the stage and container rewrites. Each
//! literal in the table is rewritten, as an exact whole command, to one
//! `<arcctl> hook-exec -- <command>` child and its argv is accepted by
//! `hook-exec`. Every other source and argv is left to today's behaviour: an
//! argument, a redirection, extra whitespace or a path-qualified program is
//! not a literal. The hook resolves and launches nothing, and emits no
//! approval decision.
use super::{HookContext, VerifiedSurface};
use std::ffi::{OsStr, OsString};

/// Whole Test commands managed through the hook, one argv per entry.
const LITERALS: &[&[&str]] = &[&["cargo", "test"], &["bun", "run", "test"]];

/// Test argv accepted by `hook-exec` in addition to the existing literals.
pub(super) fn literal_argv(argv: &[OsString]) -> bool {
    LITERALS.iter().any(|literal| {
        argv.len() == literal.len()
            && argv
                .iter()
                .zip(literal.iter())
                .all(|(word, expected)| word.as_os_str() == OsStr::new(expected))
    })
}

/// The one-child rewrite of a Test literal, or `None` to leave it alone.
pub(super) fn rewrite(
    command: &str,
    context: &HookContext,
    surface: Option<&VerifiedSurface>,
) -> Option<String> {
    if !LITERALS.iter().any(|literal| literal.join(" ") == command) {
        return None;
    }
    surface?;
    let executable = context.executable.as_deref()?;
    if !executable.is_absolute() || !super::is_executable(executable) {
        return None;
    }
    let quoted = format!("'{}'", executable.to_str()?.replace('\'', "'\\''"));
    Some(format!("{quoted} hook-exec -- {command}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_whole_literal_commands_are_ever_test_literals() {
        let context = HookContext {
            executable: std::env::current_exe().ok(),
        };
        let surface = VerifiedSurface;
        for source in [
            "cargo test -p arc-cli",
            "cargo test --workspace",
            "cargo  test",
            "cargo test ",
            "cargo test > test.log",
            "cargo test && cargo build",
            "bun run test:unit",
            "bun run test --watch",
            "/usr/local/bin/cargo test",
            "CARGO_TERM_COLOR=never cargo test",
            "vitest",
        ] {
            assert_eq!(rewrite(source, &context, Some(&surface)), None, "{source}");
        }
        for words in [
            &["cargo"][..],
            &["cargo", "test", "-p", "arc-cli"],
            &["cargo test"],
            &["bun", "run", "test", "--watch"],
            &["bun", "run"],
            &["/usr/local/bin/cargo", "test"],
            &["vitest"],
        ] {
            let argv: Vec<OsString> = words.iter().map(OsString::from).collect();
            assert!(!literal_argv(&argv), "{words:?}");
        }
        // The existing literals belong to the other tables, not this one.
        for existing in ["cargo build", "bun run build", "docker build ."] {
            assert_eq!(rewrite(existing, &context, Some(&surface)), None);
        }
    }

    #[test]
    fn the_table_is_exactly_cargo_test_and_bun_run_test() {
        assert_eq!(LITERALS, [&["cargo", "test"][..], &["bun", "run", "test"]]);
    }

    #[test]
    fn every_listed_literal_is_rewritten_to_one_child_and_accepted() {
        let executable = std::env::current_exe().unwrap();
        let context = HookContext {
            executable: Some(executable.clone()),
        };
        let surface = VerifiedSurface;
        for literal in LITERALS {
            let source = literal.join(" ");
            let expected = format!(
                "'{}' hook-exec -- {source}",
                executable.to_str().unwrap().replace('\'', "'\\''")
            );
            assert_eq!(
                super::super::rewrite_literal(&source, &context, Some(&surface)).as_deref(),
                Some(expected.as_str())
            );
            let argv: Vec<OsString> = literal.iter().map(OsString::from).collect();
            assert!(super::super::literal_argv(&argv), "{literal:?}");
            // Without a verified surface or a usable arcctl the hook fails open.
            assert_eq!(rewrite(&source, &context, None), None);
            let unusable = HookContext { executable: None };
            assert_eq!(rewrite(&source, &unusable, Some(&surface)), None);
        }
    }
}

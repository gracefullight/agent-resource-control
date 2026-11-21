//! The two exact mixed compounds that are separated into stages (AT-TRF-12,
//! DET-016). One definition serves the hook route and the execution route.
//!
//! Matching is byte-exact. Any other spelling, including extra whitespace,
//! quoting, substitution, redirection or a nested shell, is not a compound
//! here and stays with the shell that received it.
use std::{
    ffi::{OsStr, OsString},
    path::Path,
};

const TEST_STAGES: [&str; 2] = ["bun test", "make test"];
const REMOTE_STAGE: &str = "gh workflow run ci.yml";

/// The local Test stage and the remote CI stage of one separable source.
pub(super) fn stages(source: &str) -> Option<[&'static str; 2]> {
    TEST_STAGES.into_iter().find_map(|test| {
        let remote = source.strip_prefix(test)?.strip_prefix(" && ")?;
        (remote == REMOTE_STAGE).then_some([test, REMOTE_STAGE])
    })
}

/// Exactly the three stage argv the rewrite hands to `hook-exec`.
pub(super) fn stage_argv(argv: &[OsString]) -> bool {
    TEST_STAGES.into_iter().chain([REMOTE_STAGE]).any(|stage| {
        stage.split(' ').count() == argv.len()
            && stage
                .split(' ')
                .zip(argv)
                .all(|(word, argument)| argument.as_os_str() == OsStr::new(word))
    })
}

/// `<arcctl> hook-exec --arc-request <id> -- <stage 1> && <arcctl> hook-exec
/// --arc-request <id> -- <stage 2>`. The shell that runs this text keeps `&&`,
/// so a failing first stage still prevents the second and the status is the
/// shell's. `request` is interpolated unquoted and must be a job-shaped id.
pub(super) fn rewrite(source: &str, executable: &Path, request: &str) -> Option<String> {
    let [test, remote] = stages(source)?;
    let job_shaped = request.len() == 32
        && request
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    if !executable.is_absolute() || !job_shaped {
        return None;
    }
    let quoted = format!("'{}'", executable.to_str()?.replace('\'', "'\\''"));
    let stage = |words: &str| format!("{quoted} hook-exec --arc-request {request} -- {words}");
    Some(format!("{} && {}", stage(test), stage(remote)))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn exactly_two_sources_are_separated_into_their_stages() {
        assert_eq!(
            stages("bun test && gh workflow run ci.yml"),
            Some(["bun test", "gh workflow run ci.yml"])
        );
        assert_eq!(
            stages("make test && gh workflow run ci.yml"),
            Some(["make test", "gh workflow run ci.yml"])
        );
        for source in [
            "",
            "bun test",
            "gh workflow run ci.yml",
            "bun test && gh workflow run build.yml",
            "bun test  && gh workflow run ci.yml",
            "bun test && gh workflow run ci.yml ",
            " bun test && gh workflow run ci.yml",
            "bun test; gh workflow run ci.yml",
            "bun test || gh workflow run ci.yml",
            "bun test & gh workflow run ci.yml",
            "cargo test && gh workflow run ci.yml",
            "bun test $(printf x) && gh workflow run ci.yml",
            "bun test && gh workflow run ci.yml > dispatch.log",
            "bun test && gh workflow run \"$WORKFLOW\"",
            "bash -lc 'bun test && gh workflow run ci.yml'",
            "make test `printf x` && gh workflow run ci.yml",
            "make test && gh workflow run ci.yml && gh workflow run ci.yml",
            "bun test && make test && gh workflow run ci.yml",
        ] {
            assert_eq!(stages(source), None, "{source:?}");
            assert_eq!(rewrite(source, Path::new("/bin/arcctl"), ID), None);
        }
    }

    #[test]
    fn only_the_three_stage_argv_are_admitted() {
        let argv = |words: &[&str]| -> Vec<OsString> { words.iter().map(OsString::from).collect() };
        for accepted in [
            &["bun", "test"][..],
            &["make", "test"],
            &["gh", "workflow", "run", "ci.yml"],
        ] {
            assert!(stage_argv(&argv(accepted)), "{accepted:?}");
        }
        for rejected in [
            &[][..],
            &["bun"],
            &["bun", "test", "--watch"],
            &["bun test"],
            &["make"],
            &["make", "build"],
            &["cargo", "test"],
            &["gh", "workflow", "run"],
            &["gh", "workflow", "run", "build.yml"],
            &["gh", "workflow", "run", "ci.yml", "--ref", "main"],
            &["/usr/bin/make", "test"],
            &["bash", "-c", "bun test && gh workflow run ci.yml"],
        ] {
            assert!(!stage_argv(&argv(rejected)), "{rejected:?}");
        }
    }

    #[test]
    fn rewrite_names_one_request_and_keeps_the_shell_operator() {
        let rewritten = rewrite(
            "make test && gh workflow run ci.yml",
            Path::new("/opt/arc's bin/arcctl"),
            ID,
        )
        .unwrap();
        let wrapper = "'/opt/arc'\\''s bin/arcctl' hook-exec --arc-request 0123456789abcdef0123456789abcdef -- ";
        assert_eq!(
            rewritten,
            format!("{wrapper}make test && {wrapper}gh workflow run ci.yml")
        );
        // The request is interpolated into shell source: only a job-shaped id.
        for request in [
            "",
            "0123456789ABCDEF0123456789ABCDEF",
            "0123456789abcdef0123456789abcde",
            "0123456789abcdef0123456789abcdef0",
            "$(touch x)0123456789abcdef012345",
        ] {
            assert_eq!(
                rewrite(
                    "bun test && gh workflow run ci.yml",
                    Path::new("/bin/arcctl"),
                    request
                ),
                None,
                "{request:?}"
            );
        }
        assert_eq!(
            rewrite(
                "bun test && gh workflow run ci.yml",
                Path::new("arcctl"),
                ID
            ),
            None
        );
    }
}

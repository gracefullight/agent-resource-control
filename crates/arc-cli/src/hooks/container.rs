//! Local container builds on the hook route (AT-TRF-03, DET-011 HOST items).
//!
//! Registered from `hooks.rs`. Exactly `docker build .` and `docker buildx
//! build .` are rewritten to one `hook-exec` child and accepted there. The
//! hook resolves no endpoint and contacts no socket: the later `hook-exec`
//! process captures it, so a remote or unresolved engine keeps its REMOTE_EXEC
//! or UNKNOWN pass-through and only a proven local one takes a reservation.
use super::{HookContext, VerifiedSurface};
use std::ffi::{OsStr, OsString};

const LITERALS: [&[&str]; 2] = [
    &["docker", "build", "."],
    &["docker", "buildx", "build", "."],
];

/// Container argv accepted by `hook-exec` in addition to the existing literals.
pub(super) fn literal_argv(argv: &[OsString]) -> bool {
    LITERALS.iter().any(|literal| {
        argv.len() == literal.len()
            && argv
                .iter()
                .zip(literal.iter())
                .all(|(word, expected)| word.as_os_str() == OsStr::new(expected))
    })
}

/// The one-child rewrite of a container literal, or `None` to leave it alone.
/// Only the exact source is rewritten, so the shell has nothing left to
/// expand, redirect or sequence around the managed child.
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

    fn argv(words: &[&str]) -> Vec<OsString> {
        words.iter().map(OsString::from).collect()
    }

    #[test]
    fn exactly_the_two_container_literals_are_rewritten_and_accepted() {
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
                rewrite(&source, &context, Some(&surface)).as_deref(),
                Some(expected.as_str())
            );
            // The shared entry delegates here and adds no request option.
            assert_eq!(
                super::super::rewrite_literal(&source, &context, Some(&surface)).as_deref(),
                Some(expected.as_str())
            );
            assert!(literal_argv(&argv(literal)) && super::super::literal_argv(&argv(literal)));
            // Without a verified surface or a usable arcctl the hook fails open.
            assert_eq!(rewrite(&source, &context, None), None);
            for executable in [None, Some("arcctl".into()), Some("/missing/arcctl".into())] {
                let unusable = HookContext { executable };
                assert_eq!(rewrite(&source, &unusable, Some(&surface)), None);
            }
        }
    }

    #[test]
    fn every_other_docker_source_and_argv_is_left_alone() {
        let context = HookContext {
            executable: std::env::current_exe().ok(),
        };
        let surface = VerifiedSurface;
        for source in [
            "docker build",
            "docker build . ",
            "docker  build .",
            "docker build ./app",
            "docker build -t app .",
            "docker --context prod build .",
            "docker buildx build --builder remote .",
            "docker buildx build --push .",
            "docker build . && docker push app",
            "docker build . > build.log",
            "docker build $(pwd)",
            "DOCKER_HOST=tcp://192.0.2.30:2376 docker build .",
            "docker compose build",
            "docker run app",
            "/usr/local/bin/docker build .",
        ] {
            assert_eq!(rewrite(source, &context, Some(&surface)), None, "{source}");
            assert_eq!(
                super::super::rewrite_literal(source, &context, Some(&surface)),
                None,
                "{source}"
            );
        }
        for words in [
            &["docker"][..],
            &["docker", "build"],
            &["docker", "build", ".", "."],
            &["docker", "build", "./app"],
            &["docker", "--context", "prod", "build", "."],
            &["docker", "buildx", "build", "--push", "."],
            &["docker", "buildx", "bake", "."],
            &["/usr/local/bin/docker", "build", "."],
            &["docker build ."],
            &["podman", "build", "."],
        ] {
            let argv = argv(words);
            assert!(!literal_argv(&argv), "{words:?}");
            assert!(!super::super::literal_argv(&argv), "{words:?}");
        }
    }
}

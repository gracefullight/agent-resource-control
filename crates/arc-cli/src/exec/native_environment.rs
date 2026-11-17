//! Preserve the caller's native-tool environment at the single spawn boundary.
use arc_detector::Classification;
use arc_ipc::traffic::LineageContext;
use std::{ffi::OsStr, process::Command};

/// Supported shells set `_` to the executable they invoke. If they set it to
/// this wrapper, restore the PATH spelling they would have used for the tool.
/// Otherwise retain the inherited value unchanged.
pub(super) fn restore_shell_underscore(command: &mut Command, program: &OsStr) {
    let (Some(underscore), Ok(wrapper), Some(path)) = (
        std::env::var_os("_"),
        std::env::current_exe(),
        std::env::var_os("PATH"),
    ) else {
        return;
    };
    if underscore == wrapper.as_os_str() {
        if let Some(program_path) = crate::hooks::resolved_executable(program, &path) {
            command.env("_", program_path);
        }
    }
}

pub(super) fn configure(
    command: &mut Command,
    classification: Classification,
    context: &LineageContext,
    preserve_environment: bool,
) {
    configure_for_caller(
        command,
        classification,
        context,
        std::env::var_os(super::traffic::CONTEXT_ENV).is_some(),
        preserve_environment,
    );
}

fn configure_for_caller(
    command: &mut Command,
    classification: Classification,
    context: &LineageContext,
    supplied: bool,
    preserve_environment: bool,
) {
    if !preserve_environment && !classification.is_native_tool() && !supplied {
        command.env(super::traffic::CONTEXT_ENV, context.as_str());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arc_detector::Reason;
    use std::ffi::OsString;

    #[test]
    fn native_environment_all_seven_routes_preserve_inherited_lineage() {
        let key = super::super::traffic::CONTEXT_ENV;
        let context = LineageContext::try_from("a".repeat(64)).unwrap();
        for (argv, reason) in [
            (vec!["go", "build"], Reason::GoBuild),
            (vec!["go", "test"], Reason::GoTest),
            (vec!["go", "vet"], Reason::GoVet),
            (vec!["swift", "build"], Reason::SwiftBuild),
            (vec!["swift", "test"], Reason::SwiftTest),
            (vec!["cmake", "--build", "."], Reason::CmakeBuild),
            (vec!["ctest"], Reason::Ctest),
        ] {
            let tokens: Vec<OsString> = argv.iter().map(OsString::from).collect();
            let classification = arc_detector::classify_argv(&tokens);
            assert_eq!(classification.reason, reason);
            assert!(classification.is_native_tool());
            for supplied in [false, true] {
                let mut command = Command::new(argv[0]);
                configure_for_caller(&mut command, classification, &context, supplied, false);
                assert!(
                    command.get_envs().all(|(name, _)| name != key),
                    "{argv:?}, supplied={supplied}"
                );
            }
        }
    }

    #[test]
    fn native_environment_other_families_keep_legacy_injection_only_when_absent() {
        let context = LineageContext::try_from("a".repeat(64)).unwrap();
        let tokens = [OsString::from("cargo"), OsString::from("test")];
        let classification = arc_detector::classify_argv(&tokens);
        for supplied in [false, true] {
            let mut command = Command::new("cargo");
            configure_for_caller(&mut command, classification, &context, supplied, false);
            let actual = command
                .get_envs()
                .find(|(name, _)| *name == super::super::traffic::CONTEXT_ENV);
            assert_eq!(actual.is_some(), !supplied);
            if let Some((_, Some(value))) = actual {
                assert_eq!(value, context.as_str());
            }
        }
    }

    #[test]
    fn hook_environment_keeps_absent_empty_and_supplied_lineage() {
        let context = LineageContext::try_from("a".repeat(64)).unwrap();
        for words in [&["cargo", "build"][..], &["bun", "run", "build"][..]] {
            let argv: Vec<OsString> = words.iter().map(OsString::from).collect();
            let classification = arc_detector::classify_argv(&argv);
            // Both empty and nonempty values are supplied to the child by the
            // inherited environment. No environment operation is added here.
            for supplied in [false, true] {
                let mut command = Command::new(words[0]);
                configure_for_caller(&mut command, classification, &context, supplied, true);
                assert!(command
                    .get_envs()
                    .all(|(name, _)| name != super::super::traffic::CONTEXT_ENV));
            }
        }
    }
}

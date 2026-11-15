//! Preserve the caller's native-tool environment at the single spawn boundary.
use arc_detector::Classification;
use arc_ipc::traffic::LineageContext;
use std::process::Command;

pub(super) fn configure(
    command: &mut Command,
    classification: Classification,
    context: &LineageContext,
) {
    configure_for_caller(
        command,
        classification,
        context,
        std::env::var_os(super::traffic::CONTEXT_ENV).is_some(),
    );
}

fn configure_for_caller(
    command: &mut Command,
    classification: Classification,
    context: &LineageContext,
    supplied: bool,
) {
    if !classification.is_native_tool() && !supplied {
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
                configure_for_caller(&mut command, classification, &context, supplied);
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
            configure_for_caller(&mut command, classification, &context, supplied);
            let actual = command
                .get_envs()
                .find(|(name, _)| *name == super::super::traffic::CONTEXT_ENV);
            assert_eq!(actual.is_some(), !supplied);
            if let Some((_, Some(value))) = actual {
                assert_eq!(value, context.as_str());
            }
        }
    }
}

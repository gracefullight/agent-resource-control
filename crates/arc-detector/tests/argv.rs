use arc_detector::{
    analyze_argv, classify_argv, JobClass, Reason, MAX_INPUT_BYTES, MAX_TOKENS, MAX_WORK,
};
use std::ffi::OsString;
fn args(values: &[&str]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}
#[test]
fn direct_boundaries_are_not_shell_syntax() {
    for (argv, class) in [
        (
            vec![
                "cargo",
                "build",
                "",
                "two words",
                ";secret",
                "$(secret)",
                "|",
                "--",
            ],
            JobClass::Build,
        ),
        (vec!["cargo", "test"], JobClass::Test),
        (vec!["cargo", "clippy"], JobClass::Lint),
        (vec!["cargo", "fmt"], JobClass::Lint),
        (
            vec!["gh", "workflow", "run", "two words"],
            JobClass::RemoteCi,
        ),
        (
            vec!["ssh", "remote.example", "printf", ""],
            JobClass::RemoteExec,
        ),
        (vec!["cargo build"], JobClass::Unknown),
        (vec!["cargo", "build; secret"], JobClass::Unknown),
        (
            vec!["/path with spaces/tool", "--leading-dash"],
            JobClass::Unknown,
        ),
    ] {
        let original = args(&argv);
        assert_eq!(classify_argv(&original).classification, class);
        assert_eq!(original, args(&argv));
    }
    assert_eq!(classify_argv(&[]).reason, Reason::EmptyCommand);
    assert_eq!(classify_argv(&args(&[""])).reason, Reason::EmptyCommand);
    assert_eq!(
        classify_argv(&args(&["cargo", "build\0secret"])).reason,
        Reason::UnsupportedSyntax
    );
}
#[test]
fn native_bytes_remain_unmodified_and_unknown() {
    use std::os::unix::ffi::OsStringExt;
    let original = vec![
        OsString::from("cargo"),
        OsString::from_vec(b"build\xffsecret".to_vec()),
    ];
    let saved = original.clone();
    assert_eq!(classify_argv(&original).reason, Reason::UnsupportedSyntax);
    assert_eq!(original, saved);
}
#[test]
fn argv_budgets_bound_allocations_and_work() {
    let tokens = vec![OsString::new(); MAX_TOKENS + 1];
    assert_eq!(classify_argv(&tokens).reason, Reason::TokenLimit);
    let bytes = vec![OsString::from("x".repeat(MAX_INPUT_BYTES + 1))];
    assert_eq!(classify_argv(&bytes).reason, Reason::InputLimit);
    let mut exact = vec![OsString::from("cargo"), OsString::from("build")];
    exact.resize(MAX_TOKENS, OsString::new());
    let (result, stats) = analyze_argv(&exact);
    assert_eq!(result.classification, JobClass::Build);
    assert_eq!(stats.tokens, MAX_TOKENS);
    assert_eq!(stats.invocations, 1);
    assert!(stats.work <= MAX_WORK);
}

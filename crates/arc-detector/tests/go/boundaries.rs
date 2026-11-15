use super::support;
use arc_detector::{
    analyze_argv, classify, ClassifyRequest, JobClass, Reason, Uncertainty, MAX_DEPTH,
    MAX_INPUT_BYTES, MAX_INVOCATIONS, MAX_TOKENS, MAX_WORK,
};
use std::ffi::OsString;

#[test]
fn exact_byte_and_token_limits_accept_without_project_inspection() {
    let source = format!("{}.go", "a".repeat(MAX_INPUT_BYTES - "gobuild".len() - 3));
    support::known(&["go", "build", &source], Reason::GoBuild);
    let mut selectors = support::argv(&["go", "vet"]);
    selectors.resize(MAX_TOKENS, OsString::from("."));
    let (result, stats) = analyze_argv(&selectors);
    assert_eq!(result.reason, Reason::GoVet);
    assert_eq!(stats.tokens, MAX_TOKENS);
    assert_eq!(stats.invocations, 1);
    assert_eq!(stats.max_depth, 0);
    assert!(stats.work <= MAX_WORK);
    assert!(stats.invocations <= MAX_INVOCATIONS && stats.max_depth <= MAX_DEPTH);
    selectors.push(".".into());
    assert_eq!(analyze_argv(&selectors).0.reason, Reason::TokenLimit);
}

#[test]
fn work_limit_refuses_before_scanning_a_large_attached_value() {
    let value = format!("-tags={}", "x".repeat(14_000));
    let input = support::argv(&["go", "build", &value]);
    let (classification, stats) = analyze_argv(&input);
    assert_eq!(classification.reason, Reason::WorkLimit);
    assert_eq!(classification.uncertainty, Uncertainty::Observe);
    assert_eq!(analyze_argv(&input), (classification, stats));
    assert!(stats.work <= MAX_WORK && stats.tokens <= MAX_TOKENS);
}

#[test]
fn numeric_duration_and_opaque_boundaries_are_finite() {
    for value in ["1", "0001", "1024"] {
        for flag in ["-p", "-parallel", "-count"] {
            support::known(&["go", "test", flag, value], Reason::GoTest);
        }
    }
    for unit in ["ns", "us", "ms", "s", "m", "h"] {
        for flag in ["-timeout", "-benchtime"] {
            for number in ["1", "4294967295"] {
                support::known(
                    &["go", "test", flag, &format!("{number}{unit}")],
                    Reason::GoTest,
                );
            }
            for number in ["0", "4294967296", "-1", "1.5", "+1"] {
                support::observe(&["go", "test", flag, &format!("{number}{unit}")]);
            }
        }
    }
    for value in ["1", "1d", "1µs", "1s1m", "1e3s", "s", "", "-pattern"] {
        support::observe(&["go", "test", "-timeout", value]);
    }
    // Selectors and opaque values are lexical data; no regex, shell or path evaluation.
    support::known(
        &["go", "test", "-run", "[invalid regex", "./..."],
        Reason::GoTest,
    );
    support::known(
        &["go", "build", "-tags", "$(poison); value 日本語"],
        Reason::GoBuild,
    );
    support::known(&["go", "build", "path 日本語/a.go"], Reason::GoBuild);
}

#[test]
fn literal_command_surface_does_not_gain_native_classification() {
    for command in [
        "go build -p 2 ./...",
        "go test -run TestNative",
        "go vet -printf .",
        "'/tool path/go' -C project build",
        "go build && cargo build",
    ] {
        let result = classify(&ClassifyRequest {
            command: command.into(),
            shell_kind: "sh".into(),
        });
        assert_eq!(result.classification, JobClass::Unknown);
        assert_eq!(result.uncertainty, Uncertainty::Observe);
    }
}

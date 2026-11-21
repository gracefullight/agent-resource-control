use super::*;

#[test]
fn sh_fallback_charges_outer_argv_and_depth_without_shell_lane_support() {
    let context = arc_detector::EndpointContext {
        captured_at_ms: 10_000,
        ..Default::default()
    };
    let source = |text: String| vec![OsString::from("sh"), OsString::from("-c"), text.into()];
    let classify = |text| bounded_sh_fallback(&source(text), &context, 10_000).unwrap();
    assert_eq!(classify("cargo test".into()).classification, JobClass::Test);
    for (text, reason) in [
        (
            format!(
                "{}cargo test",
                " ".repeat(arc_detector::MAX_INPUT_BYTES - 10)
            ),
            arc_detector::Reason::InputLimit,
        ),
        (
            format!(
                "{}cargo test{}",
                "( ".repeat(arc_detector::MAX_DEPTH),
                " )".repeat(arc_detector::MAX_DEPTH)
            ),
            arc_detector::Reason::NestingLimit,
        ),
        (
            format!("cargo test{}", " x".repeat(arc_detector::MAX_TOKENS - 2)),
            arc_detector::Reason::TokenLimit,
        ),
        (
            format!(
                "cargo test {}",
                "x".repeat(arc_detector::MAX_INPUT_BYTES - 4 - "cargo test ".len())
            ),
            arc_detector::Reason::WorkLimit,
        ),
        (
            format!(
                "{}cargo test",
                "cargo test; ".repeat(arc_detector::MAX_INVOCATIONS - 1)
            ),
            arc_detector::Reason::InvocationLimit,
        ),
    ] {
        let result = classify(text);
        assert_eq!(result.classification, JobClass::Unknown);
        assert_eq!(result.uncertainty, arc_detector::Uncertainty::Observe);
        assert_eq!(result.reason, reason);
    }
}

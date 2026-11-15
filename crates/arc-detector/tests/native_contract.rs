#[path = "support/native.rs"]
mod support;

#[test]
fn frozen_contract_has_all_required_seams_within_the_evidence_budget() {
    let contract = include_str!("../../../.arc-verify/native-tool-families/contract.md");
    assert!((200..=8000).contains(&contract.len()));
    assert!(contract.lines().count() < 500);
    for seam in [
        "6918f14acd866efd5acd8a0ae3225aad36eeb29c",
        "GO_BUILD",
        "GO_TEST",
        "GO_VET",
        "SWIFT_BUILD",
        "SWIFT_TEST",
        "CMAKE_BUILD",
        "CTEST",
        "traffic.native-tools",
        "amendment 18",
        "task-2-go",
        "task-3-swift",
        "task-4-cmake",
        "requiresIntegrationGates:false",
    ] {
        assert!(contract.contains(seam), "missing frozen seam {seam}");
    }
}

#[test]
fn additive_vocabulary_and_legacy_projection_are_exact() {
    support::vocabulary();
}
#[test]
fn native_boundaries_reject_before_family_dispatch() {
    for prefix in [
        &["go", "build"][..],
        &["swift", "test"],
        &["cmake", "--build", "build"],
        &["ctest"],
    ] {
        support::limits(prefix);
    }
    for wrapper in [
        &["env", "go", "build"][..],
        &["sh", "-c", "swift build"],
        &["xcrun", "swift", "test"],
        &["custom", "ctest"],
    ] {
        support::observe(wrapper);
    }
}
#[test]
fn literal_protocol_remains_legacy_unknown() {
    for command in [
        "go build",
        "go test",
        "go vet",
        "swift build",
        "swift test",
        "cmake --build build",
        "ctest",
    ] {
        let result = arc_detector::classify(&arc_detector::ClassifyRequest {
            command: command.into(),
            shell_kind: "sh".into(),
        });
        assert_eq!(result.classification, arc_detector::JobClass::Unknown);
        assert_eq!(result.uncertainty, arc_detector::Uncertainty::Observe);
    }
}

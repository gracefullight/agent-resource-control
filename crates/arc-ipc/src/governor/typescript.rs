//! Test-only exporter, intentionally without #[ts(export)] automatic writes.
use super::*;
use std::{fs, path::PathBuf};
use ts_rs::{Config, TS};

fn generated() -> String {
    let cfg = Config::new();
    let declarations = [
        GovernorSwitchKey::decl(&cfg),
        GovernorSwitchState::decl(&cfg),
        GetGovernorSwitchRequest::decl(&cfg),
        SetGovernorSwitchRequest::decl(&cfg),
        GovernorConflictReason::decl(&cfg),
        GovernorRejectionReason::decl(&cfg),
        GovernorMutationResult::decl(&cfg),
        GovernorSwitchReadResult::decl(&cfg),
    ];
    let mut out = String::from("// Generated from Rust arc-ipc::governor by ts-rs 12.0.1. Do not edit.\n// biome-ignore-all format: Preserve reproducible ts-rs declarations.\n// Revisions/timestamps are validated integers in 0..=9007199254740991.\n// Regenerate: ARC_GOVERNOR_EXPORT=1 cargo test -p arc-ipc --lib governor::typescript::regenerate -- --exact\n\n");
    for declaration in declarations {
        out.push_str("export ");
        out.push_str(&declaration);
        out.push('\n');
    }
    out.lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}
fn path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../apps/desktop/src/bridge/governor.generated.ts")
}
fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../apps/desktop/tests/fixtures/governor-wire.json")
}
fn fixtures() -> String {
    use serde_json::json;
    let key = GovernorSwitchKey::GovernorEnabled;
    let default = GovernorSwitchState::default();
    let paused = GovernorSwitchState {
        key,
        enabled: false,
        revision: 1,
        updated_at_ms: Some(10),
    };
    let maximum = GovernorSwitchState {
        key,
        enabled: true,
        revision: MAX_SAFE_INTEGER,
        updated_at_ms: Some(MAX_SAFE_INTEGER),
    };
    let request = SetGovernorSwitchRequest {
        key,
        enabled: false,
        expected_revision: 0,
        mutation_id: format!("0-{}", "a".repeat(32)),
    };
    let results = [
        GovernorMutationResult::Committed {
            mutation_id: request.mutation_id.clone(),
            switch: paused.clone(),
            changed: true,
            replayed: false,
        },
        GovernorMutationResult::Committed {
            mutation_id: request.mutation_id.clone(),
            switch: paused.clone(),
            changed: true,
            replayed: true,
        },
        GovernorMutationResult::Committed {
            mutation_id: format!("1-{}", "b".repeat(32)),
            switch: paused.clone(),
            changed: false,
            replayed: false,
        },
        GovernorMutationResult::Conflict {
            mutation_id: request.mutation_id.clone(),
            switch: paused.clone(),
            reason: GovernorConflictReason::RevisionConflict,
        },
        GovernorMutationResult::Conflict {
            mutation_id: request.mutation_id.clone(),
            switch: default.clone(),
            reason: GovernorConflictReason::MutationIdConflict,
        },
        GovernorMutationResult::Rejected {
            mutation_id: request.mutation_id.clone(),
            switch: default.clone(),
            reason: GovernorRejectionReason::ReceiptCapacity,
        },
        GovernorMutationResult::Rejected {
            mutation_id: format!("{}-{}", MAX_SAFE_INTEGER, "c".repeat(32)),
            switch: maximum.clone(),
            reason: GovernorRejectionReason::RevisionExhausted,
        },
    ];
    let read_results = [
        GovernorSwitchReadResult {
            switch: default.clone(),
            mutation_result: None,
        },
        GovernorSwitchReadResult {
            switch: paused.clone(),
            mutation_result: Some(results[1].clone()),
        },
        GovernorSwitchReadResult {
            switch: maximum.clone(),
            mutation_result: Some(results[1].clone()),
        },
    ];
    serde_json::to_string_pretty(&json!({
        "generatedFrom": "Rust arc-ipc::governor; explicit test-only export",
        "states": [default, paused, maximum],
        "reads": [GetGovernorSwitchRequest::default(), GetGovernorSwitchRequest { key, mutation_id:Some(request.mutation_id.clone()) }],
        "readResults": read_results,
        "requests": [request, SetGovernorSwitchRequest { key, enabled:false, expected_revision:1, mutation_id:format!("1-{}","b".repeat(32)) }, SetGovernorSwitchRequest { key, enabled:false, expected_revision:MAX_SAFE_INTEGER, mutation_id:format!("{}-{}",MAX_SAFE_INTEGER,"c".repeat(32)) }],
        "results": results
    })).unwrap() + "\n"
}
#[test]
fn generated_types_are_current_without_writing() {
    let before = fs::read(path()).expect("run the explicit Governor regeneration command");
    assert_eq!(
        before,
        generated().as_bytes(),
        "Governor TypeScript contract drift"
    );
    assert_eq!(before, fs::read(path()).unwrap());
    let before = fs::read(fixture_path()).unwrap();
    assert_eq!(
        before,
        fixtures().as_bytes(),
        "Rust serialized Governor fixture drift"
    );
    assert_eq!(before, fs::read(fixture_path()).unwrap());
}
#[test]
fn regenerate() {
    if std::env::var_os("ARC_GOVERNOR_EXPORT").as_deref() == Some(std::ffi::OsStr::new("1")) {
        fs::write(path(), generated()).unwrap();
        fs::write(fixture_path(), fixtures()).unwrap();
    }
    // Ordinary runs are nonmutating agreement checks; no ignored test can
    // invalidate the maintained complete-native-test coverage receipt policy.
    assert_eq!(fs::read(path()).unwrap(), generated().as_bytes());
    assert_eq!(fs::read(fixture_path()).unwrap(), fixtures().as_bytes());
}

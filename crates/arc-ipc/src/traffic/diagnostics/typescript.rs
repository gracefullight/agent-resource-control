//! Explicit test-only export, using the existing ts-rs/serde contract mechanism.
pub(super) mod outcome;
use super::*;
use crate::{JobClass, Locality, Reason, Uncertainty};
use serde_json::{json, Value};
use std::{fs, path::PathBuf};
use ts_rs::{Config, TS};

fn classifications() -> Vec<Classification> {
    use Reason::*;
    let reasons = [
        CargoBuild,
        CargoTest,
        BunTest,
        BunScriptBuild,
        BunScriptTest,
        BunScriptLint,
        BunScriptTypecheck,
        CargoClippy,
        CargoFmt,
        GhWorkflowRun,
        GhRunWatch,
        GhRunView,
        SshRemoteCommand,
        CompoundLocal,
        CompoundRemote,
        CompoundMixed,
        EmptyCommand,
        UnsupportedShell,
        UnsupportedSyntax,
        UnresolvedCommand,
        UnresolvedScript,
        UnresolvedExpansion,
        UnresolvedCompound,
        SshForwarding,
        SshAmbiguous,
        DockerEndpointUnknown,
        InputLimit,
        TokenLimit,
        InvocationLimit,
        NestingLimit,
        WorkLimit,
    ];
    let mut values = Vec::new();
    for classification in [
        JobClass::Build,
        JobClass::Test,
        JobClass::Lint,
        JobClass::Typecheck,
        JobClass::RemoteCi,
        JobClass::RemoteExec,
        JobClass::Mixed,
        JobClass::Unknown,
    ] {
        for reason in reasons {
            for locality in [
                Locality::HostLocal,
                Locality::Remote,
                Locality::Mixed,
                Locality::Unknown,
            ] {
                for uncertainty in [Uncertainty::None, Uncertainty::Observe] {
                    let value = Classification {
                        classification,
                        locality,
                        reason,
                        uncertainty,
                    };
                    if value.is_consistent() {
                        values.push(value);
                    }
                }
            }
        }
    }
    values
}
fn structure(value: &Value) -> String {
    match value {
        Value::Number(_) => "number".into(),
        Value::Object(fields) => format!(
            "{{ {} }}",
            fields
                .iter()
                .map(|(name, value)| format!(
                    "{}: {}",
                    serde_json::to_string(name).unwrap(),
                    structure(value)
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        _ => panic!("limits contain only objects and numbers"),
    }
}
fn generated() -> String {
    let cfg = Config::new();
    let mut out = String::from("// Generated from Rust arc-ipc::traffic::diagnostics by ts-rs 12.0.1 and serde. Do not edit.\n// biome-ignore-all format: Preserve reproducible Rust declarations.\n// Regenerate: ARC_TRAFFIC_SCRIPTS_EXPORT=1 cargo test -p arc-ipc --lib traffic::diagnostics::typescript::regenerate -- --exact\n// Numbers are validated safe integers; nullable fields are required.\nimport type { DecisionReason, RecordedDecision } from './policy.types';\n\n");
    out.push_str(&format!(
        "export type TrafficClassification = {};\n",
        classifications()
            .iter()
            .map(|v| serde_json::to_string(v).unwrap())
            .collect::<Vec<_>>()
            .join(" | ")
    ));
    out.push_str(&format!(
        "export type ScriptPolicyLimits = {};\n",
        structure(&serde_json::to_value(ScriptPolicyLimits::default()).unwrap())
    ));
    for declaration in [
        outcome::SpawnReasonType::decl(&cfg),
        outcome::OutcomeType::decl(&cfg),
        DiagnosticRequest::decl(&cfg),
        AdmissionState::decl(&cfg),
        AdmissionDiagnostic::decl(&cfg),
        DiagnosticResult::decl(&cfg),
    ] {
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
fn fixture_values() -> Value {
    let id = "0123456789abcdef0123456789abcdef";
    let classification = json!({"classification":"TYPECHECK","locality":"HOST_LOCAL",
        "reason":"BUN_SCRIPT_TYPECHECK","uncertainty":"NONE"});
    let mut results = Vec::new();
    for state in [
        AdmissionState::Queued,
        AdmissionState::Reserved,
        AdmissionState::Running,
        AdmissionState::Completed,
        AdmissionState::Cancelled,
        AdmissionState::Abandoned,
        AdmissionState::Unconfirmed,
        AdmissionState::Unreserved,
    ] {
        let mut admission = json!({"state":state,"sequence":1,"queued_at_ms":10,
            "granted_at_ms":null,"queue_wait_ms":null,"wait_reason":null,"last_decision":null,
            "root_job_id":null,"permit_generation":null,"reservation":null,"permit_active":false,
            "outcome":null});
        if matches!(state, AdmissionState::Reserved | AdmissionState::Running) {
            admission["root_job_id"] = json!(id);
            admission["permit_generation"] = json!(1);
            admission["reservation"] = json!({"cpu_millis":1000,"memory_bytes":1073741824});
            admission["permit_active"] = json!(true);
            admission["granted_at_ms"] = json!(12);
        }
        if matches!(state, AdmissionState::Running | AdmissionState::Completed) {
            admission["queue_wait_ms"] = json!(2);
        }
        if state == AdmissionState::Completed {
            admission["outcome"] = json!({"kind":"EXITED","code":17});
        }
        results.push(json!({"schema_version":1,"job_id":id,"recorded_at_ms":20,
            "liveness_fresh":false,"classification":classification,"admission":admission,
            "limits":ScriptPolicyLimits::default()}));
    }
    results.push(json!({"schema_version":1,"job_id":id,"recorded_at_ms":20,
        "liveness_fresh":false,"classification":null,"admission":null,
        "limits":ScriptPolicyLimits::default()}));
    for value in &results {
        assert!(serde_json::from_value::<DiagnosticResult>(value.clone())
            .unwrap()
            .is_valid());
    }
    json!({"generatedFrom":"Rust arc-ipc::traffic::diagnostics; explicit test-only export",
        "classifications":classifications(),"limits":ScriptPolicyLimits::default(),
        "queries":[{"job_id":id}],"results":results})
}
fn paths() -> (PathBuf, PathBuf) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    (
        root.join("apps/desktop/src/bridge/traffic-scripts.generated.ts"),
        root.join("apps/desktop/tests/fixtures/traffic-scripts-wire.json"),
    )
}
#[test]
fn generated_types_are_current_without_writing() {
    let (wire, fixtures) = paths();
    assert_eq!(fs::read_to_string(wire).unwrap(), generated());
    let expected = serde_json::to_string_pretty(&fixture_values()).unwrap() + "\n";
    assert_eq!(fs::read_to_string(fixtures).unwrap(), expected);
}
#[test]
fn regenerate() {
    let (wire, fixtures) = paths();
    if std::env::var_os("ARC_TRAFFIC_SCRIPTS_EXPORT").as_deref() == Some(std::ffi::OsStr::new("1"))
    {
        fs::write(&wire, generated()).unwrap();
        fs::write(
            &fixtures,
            serde_json::to_string_pretty(&fixture_values()).unwrap() + "\n",
        )
        .unwrap();
    }
    assert_eq!(fs::read_to_string(wire).unwrap(), generated());
    assert_eq!(
        fs::read_to_string(fixtures).unwrap(),
        serde_json::to_string_pretty(&fixture_values()).unwrap() + "\n"
    );
}

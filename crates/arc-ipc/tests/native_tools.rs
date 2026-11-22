//! Compiled native vocabulary contract. No daemon handlers or native tool executions.
use arc_ipc::{
    governor::{self, GovernorJobRequest},
    jobs::{JobId, ProcessIdentity},
    traffic::{self, diagnostics, native, JobRequest},
    Classification, JobClass, Reason, Request, Response, ERROR_INVALID_REQUEST,
    ERROR_UNSUPPORTED_CAPABILITY,
};
use serde_json::{json, Value};

fn facts() -> Vec<Classification> {
    [
        (JobClass::Build, Reason::GoBuild),
        (JobClass::Test, Reason::GoTest),
        (JobClass::Lint, Reason::GoVet),
        (JobClass::Build, Reason::SwiftBuild),
        (JobClass::Test, Reason::SwiftTest),
        (JobClass::Build, Reason::CmakeBuild),
        (JobClass::Test, Reason::Ctest),
    ]
    .into_iter()
    .map(|(classification, reason)| Classification {
        classification,
        reason,
        locality: arc_ipc::Locality::HostLocal,
        uncertainty: arc_ipc::Uncertainty::None,
    })
    .collect()
}
fn enqueue(classification: Classification) -> Value {
    json!(GovernorJobRequest::Traffic {
        request: JobRequest::Enqueue {
            job_id: JobId::try_from("a".repeat(32)).unwrap(),
            observer: ProcessIdentity::capture(std::process::id()).unwrap(),
            classification,
            lineage_context: None,
            project: None,
            event: None,
        }
    })
}

#[test]
fn native_admission_has_one_operation_and_reuses_governor_envelopes() {
    for classification in facts() {
        let payload = enqueue(classification);
        let typed: native::NativeJobRequest = serde_json::from_value(payload.clone()).unwrap();
        assert_eq!(
            native::decode_job(native::OPERATION_JOB, &payload),
            Ok(typed)
        );
        assert_eq!(governor::decode_job(&payload), Err(ERROR_INVALID_REQUEST));
        assert_eq!(
            traffic::python::decode_job(traffic::OPERATION_PYTHON_TESTS_JOB, &payload),
            Err(ERROR_INVALID_REQUEST)
        );
        assert_eq!(
            traffic::scripts::decode_job(traffic::OPERATION_BUN_SCRIPTS_JOB, &payload),
            Err(ERROR_INVALID_REQUEST)
        );
        for operation in [traffic::OPERATION_TRAFFIC_JOB, traffic::OPERATION_BUN_JOB] {
            assert_eq!(
                traffic::decode_job_for(operation, &payload["request"]),
                Err(ERROR_INVALID_REQUEST)
            );
        }
        for (field, value) in [
            ("classification", json!("UNKNOWN")),
            ("locality", json!("REMOTE")),
            ("uncertainty", json!("OBSERVE")),
        ] {
            let mut malformed = payload.clone();
            malformed["request"]["classification"][field] = value;
            assert_eq!(
                native::decode_job(native::OPERATION_JOB, &malformed),
                Err(ERROR_INVALID_REQUEST)
            );
        }
    }
    let mut old = facts()[0];
    old.reason = Reason::CargoBuild;
    assert_eq!(
        native::decode_job(native::OPERATION_JOB, &enqueue(old)),
        Err(ERROR_INVALID_REQUEST)
    );
    for payload in [
        Value::Null,
        json!([]),
        json!({"reason":"FUTURE"}),
        json!({"x":"x".repeat(governor::MAX_EXECUTION_BYTES)}),
    ] {
        assert_eq!(
            native::decode_job("unsupported", &payload),
            Err(ERROR_UNSUPPORTED_CAPABILITY)
        );
        assert_eq!(
            native::decode_job(native::OPERATION_JOB, &payload),
            Err(ERROR_INVALID_REQUEST)
        );
    }
    let poll = json!({"action":"TRAFFIC","request":{"action":"POLL","job_id":"a".repeat(32),"observer":ProcessIdentity::capture(std::process::id()).unwrap()}});
    assert!(native::decode_job(native::OPERATION_JOB, &poll).is_ok());
    assert_eq!(
        traffic::Capabilities::native_supported().capability,
        native::CAPABILITY
    );
}

#[test]
fn unsupported_probes_are_distinct_from_malformed_and_uncertain_replies() {
    for (operation, capability) in [
        (native::OPERATION_CAPABILITIES, native::CAPABILITY),
        (
            native::OPERATION_DIAGNOSTICS_CAPABILITIES,
            native::DIAGNOSTICS_CAPABILITY,
        ),
    ] {
        let request = Request::new("native-probe", operation);
        let interpret =
            |q: &Request, r: &Response| traffic::interpret_probe_for(q, r, operation, capability);
        let good = Response::ok(&request.request_id, json!({"capability":capability}));
        assert_eq!(interpret(&request, &good), traffic::ProbeResult::Supported);
        assert_eq!(
            interpret(
                &request,
                &Response::error(&request.request_id, ERROR_UNSUPPORTED_CAPABILITY)
            ),
            traffic::ProbeResult::Unsupported
        );
        // A sent mutation may have committed. WRITE_UNCERTAIN is never Unsupported.
        for bad in [
            Response::error(&request.request_id, governor::ERROR_WRITE_UNCERTAIN),
            Response::ok("other-id", json!({"capability":capability})),
            Response::ok(&request.request_id, json!({"capability":"unknown"})),
            Response::ok(&request.request_id, json!([capability])),
            Response::ok(
                &request.request_id,
                json!({"capability":capability,"extra":true}),
            ),
        ] {
            assert_eq!(interpret(&request, &bad), traffic::ProbeResult::Invalid);
        }
        let mut bad = good.clone();
        bad.protocol_version = 2;
        assert_eq!(interpret(&request, &bad), traffic::ProbeResult::Invalid);
        let mut bad = request.clone();
        bad.payload = json!({});
        assert_eq!(interpret(&bad, &good), traffic::ProbeResult::Invalid);
        bad = request.clone();
        bad.protocol_version = 2;
        assert_eq!(interpret(&bad, &good), traffic::ProbeResult::Invalid);
        assert_eq!(
            traffic::interpret_probe_for(&request, &good, operation, "other-capability"),
            traffic::ProbeResult::Invalid
        );
    }
}

#[test]
fn both_older_diagnostic_vocabularies_hide_native_facts_without_changing_storage() {
    let fixtures: Value = serde_json::from_str(include_str!(
        "../../../apps/desktop/tests/fixtures/traffic-scripts-wire.json"
    ))
    .unwrap();
    for classification in facts() {
        let mut payload = fixtures["results"][2].clone(); // running with an active reservation
        payload["classification"] = json!(classification);
        let stored: native::NativeDiagnosticResult = serde_json::from_value(payload).unwrap();
        assert!(stored.is_valid() && stored.admission.as_ref().unwrap().permit_active);
        for projected in [stored.clone().legacy(), stored.clone().before_native()] {
            assert_eq!(projected.classification, Some(classification.legacy()));
            assert!(projected.admission.is_none() && projected.is_valid());
        }
        assert_eq!(stored.classification, Some(classification));
        assert!(stored.admission.as_ref().unwrap().permit_active);
        let mut request = Request::new("native-diagnostic", native::OPERATION_DIAGNOSTICS_JOB);
        request.payload = json!({"job_id":stored.job_id});
        let response = Response::ok(&request.request_id, json!(stored));
        assert_eq!(
            diagnostics::interpret_result(&request, &response),
            Ok(stored)
        );
        assert_eq!(
            diagnostics::interpret_result(&request, &Response::ok("other-id", json!(null))),
            Err(ERROR_INVALID_REQUEST)
        );
    }
    let mut python: native::NativeDiagnosticResult =
        serde_json::from_value(fixtures["results"][2].clone()).unwrap();
    python.classification = Some(Classification {
        classification: JobClass::Test,
        reason: Reason::PythonPytest,
        ..facts()[0]
    });
    assert_eq!(python.clone().before_native(), python);
    assert!(python.legacy().admission.is_none());
}

#[test]
fn protocol_one_dispatch_rejects_unknown_operations_before_reading_native_payloads() {
    for operation in [
        native::OPERATION_CAPABILITIES,
        native::OPERATION_JOB,
        native::OPERATION_DIAGNOSTICS_CAPABILITIES,
        native::OPERATION_DIAGNOSTICS_JOB,
    ] {
        let mut request = Request::new("old-peer", operation);
        request.payload = json!({"future":"GO_BUILD"});
        let response = arc_ipc::dispatch(&request, || unreachable!());
        assert_eq!(response.protocol_version, 1);
        assert_eq!(response.error.unwrap().code, ERROR_UNSUPPORTED_CAPABILITY);
    }
}

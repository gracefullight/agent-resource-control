use arc_ipc::{
    governor::{self, GovernorJobRequest},
    jobs::{JobId, ProcessIdentity},
    traffic::{self, diagnostics, JobRequest},
    Classification, JobClass, Locality, Reason, Request, Response, Uncertainty,
    ERROR_INVALID_REQUEST, ERROR_UNSUPPORTED_CAPABILITY,
};
use serde_json::json;

fn classification(reason: Reason) -> Classification {
    Classification {
        classification: JobClass::Test,
        locality: Locality::HostLocal,
        reason,
        uncertainty: Uncertainty::None,
    }
}
fn enqueue(reason: Reason) -> serde_json::Value {
    json!(GovernorJobRequest::Traffic {
        request: JobRequest::Enqueue {
            job_id: JobId::try_from("a".repeat(32)).unwrap(),
            observer: ProcessIdentity::capture(std::process::id()).unwrap(),
            classification: classification(reason),
            lineage_context: None,
        }
    })
}

#[test]
fn python_vocabulary_is_only_accepted_on_the_negotiated_admission() {
    for reason in [Reason::PythonPytest, Reason::PythonUnittest] {
        let payload = enqueue(reason);
        assert!(traffic::python::decode_job(traffic::OPERATION_PYTHON_TESTS_JOB, &payload).is_ok());
        assert_eq!(governor::decode_job(&payload), Err(ERROR_INVALID_REQUEST));
        assert_eq!(
            traffic::scripts::decode_job(traffic::OPERATION_BUN_SCRIPTS_JOB, &payload),
            Err(ERROR_INVALID_REQUEST)
        );
        for op in [traffic::OPERATION_TRAFFIC_JOB, traffic::OPERATION_BUN_JOB] {
            assert_eq!(
                traffic::decode_job_for(op, &payload["request"]),
                Err(ERROR_INVALID_REQUEST)
            );
        }
        let mut bad = payload.clone();
        bad["request"]["classification"]["locality"] = json!("REMOTE");
        assert_eq!(
            traffic::python::decode_job(traffic::OPERATION_PYTHON_TESTS_JOB, &bad),
            Err(ERROR_INVALID_REQUEST)
        );
    }
    for reason in [Reason::CargoTest, Reason::BunTest, Reason::BunScriptTest] {
        assert_eq!(
            traffic::python::decode_job(traffic::OPERATION_PYTHON_TESTS_JOB, &enqueue(reason)),
            Err(ERROR_INVALID_REQUEST)
        );
    }
    for payload in [json!(null), json!([]), json!({"unknown":"PYTHON_FUTURE"})] {
        assert_eq!(
            traffic::python::decode_job("old.operation", &payload),
            Err(ERROR_UNSUPPORTED_CAPABILITY)
        );
        assert_eq!(
            traffic::python::decode_job(traffic::OPERATION_PYTHON_TESTS_JOB, &payload),
            Err(ERROR_INVALID_REQUEST)
        );
    }
    let poll = json!({"action":"TRAFFIC", "request":{"action":"POLL", "job_id":"a".repeat(32), "observer":ProcessIdentity::capture(std::process::id()).unwrap()}});
    assert!(traffic::python::decode_job(traffic::OPERATION_PYTHON_TESTS_JOB, &poll).is_ok());
    let oversized = json!({"action":"FUTURE", "extra":"x".repeat(governor::MAX_EXECUTION_BYTES)});
    assert_eq!(
        traffic::python::decode_job(traffic::OPERATION_PYTHON_TESTS_JOB, &oversized),
        Err(ERROR_INVALID_REQUEST)
    );
}

#[test]
fn python_probes_validate_capability_id_protocol_and_explicit_unsupported() {
    for (op, capability) in [
        (
            traffic::OPERATION_PYTHON_TESTS_CAPABILITIES,
            traffic::PYTHON_TESTS_CAPABILITY,
        ),
        (
            diagnostics::OPERATION_PYTHON_CAPABILITIES,
            diagnostics::PYTHON_CAPABILITY,
        ),
    ] {
        let request = Request::new("python-probe", op);
        let response = Response::ok(&request.request_id, json!({"capability":capability}));
        let interpret =
            |q: &Request, r: &Response| traffic::interpret_probe_for(q, r, op, capability);
        assert_eq!(
            interpret(&request, &response),
            traffic::ProbeResult::Supported
        );
        assert_eq!(
            interpret(
                &request,
                &Response::error(&request.request_id, ERROR_UNSUPPORTED_CAPABILITY)
            ),
            traffic::ProbeResult::Unsupported
        );
        for bad in [
            Response::ok("wrong-id", json!({"capability":capability})),
            Response::ok(&request.request_id, json!({"capability":"unknown"})),
            Response::ok(&request.request_id, json!([capability])),
        ] {
            assert_eq!(interpret(&request, &bad), traffic::ProbeResult::Invalid);
        }
        let mut bad = response.clone();
        bad.protocol_version = 2;
        assert_eq!(interpret(&request, &bad), traffic::ProbeResult::Invalid);
        let mut bad = request.clone();
        bad.protocol_version = 2;
        assert_eq!(interpret(&bad, &response), traffic::ProbeResult::Invalid);
        let mut bad = request.clone();
        bad.payload = json!({});
        assert_eq!(interpret(&bad, &response), traffic::ProbeResult::Invalid);
    }
    assert_eq!(
        traffic::Capabilities::python_supported().capability,
        traffic::PYTHON_TESTS_CAPABILITY
    );
}

#[test]
fn new_client_old_daemon_rejects_python_operations_before_interpreting_payloads() {
    for op in [
        traffic::OPERATION_PYTHON_TESTS_CAPABILITIES,
        traffic::OPERATION_PYTHON_TESTS_JOB,
        diagnostics::OPERATION_PYTHON_CAPABILITIES,
        diagnostics::OPERATION_PYTHON_JOB,
    ] {
        let mut request = Request::new("old-daemon", op);
        request.payload = json!({"future":"PYTHON_PYTEST"});
        let response = arc_ipc::dispatch(&request, || unreachable!());
        assert_eq!(response.protocol_version, 1);
        assert_eq!(response.error.unwrap().code, ERROR_UNSUPPORTED_CAPABILITY);
    }
}

#[test]
fn old_client_new_daemon_projection_preserves_stored_python_facts() {
    let wire: serde_json::Value = serde_json::from_str(include_str!(
        "../../../apps/desktop/tests/fixtures/traffic-scripts-wire.json"
    ))
    .unwrap();
    for reason in [Reason::PythonPytest, Reason::PythonUnittest] {
        let mut payload = wire["results"][3].clone();
        payload["classification"] = json!(classification(reason));
        let stored: diagnostics::DiagnosticResult = serde_json::from_value(payload).unwrap();
        assert!(stored.is_valid() && stored.admission.is_some());
        let legacy = stored.clone().legacy();
        assert_eq!(legacy.classification, Some(classification(reason).legacy()));
        assert!(legacy.admission.is_none() && legacy.is_valid());
        assert_eq!(stored.classification.unwrap().reason, reason);
        assert!(stored.admission.is_some());
        let mut request = Request::new("python-read", diagnostics::OPERATION_PYTHON_JOB);
        request.payload = json!({"job_id":stored.job_id});
        let response = Response::ok(&request.request_id, json!(stored));
        assert_eq!(
            diagnostics::interpret_result(&request, &response),
            Ok(stored)
        );
    }
    for reason in [Reason::CargoTest, Reason::BunTest, Reason::BunScriptTest] {
        let mut payload = wire["results"][3].clone();
        payload["classification"] = json!(classification(reason));
        let old: diagnostics::DiagnosticResult = serde_json::from_value(payload).unwrap();
        assert_eq!(old.clone().legacy(), old);
    }
}

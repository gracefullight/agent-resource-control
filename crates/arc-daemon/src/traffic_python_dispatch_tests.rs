//! Shared dispatch, owner identity and legacy projections; no separate writer.
use super::*;
use arc_ipc::{
    governor,
    traffic::{self, diagnostics},
};
use serde_json::json;

#[tokio::test]
async fn python_capabilities_and_grammar_guards_precede_the_shared_writer() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let handler: Handler = Arc::new(move |input, peer, _| {
        observed.fetch_add(1, Ordering::SeqCst);
        assert_eq!(peer, std::process::id());
        let Input::Governor(crate::connection::governor_dispatch::Input::Job(
            governor::GovernorJobRequest::Traffic {
                request: traffic::JobRequest::Enqueue { classification, .. },
            },
        )) = input
        else {
            panic!("wrong writer interface")
        };
        assert!(classification.is_python_test());
        Box::pin(async {
            Ok(Value::Governor(
                crate::connection::governor_dispatch::Value::Job(
                    governor::GovernorJobResult::Traffic {
                        result: traffic::JobResult::Queued {
                            sequence: 1,
                            queued_at_ms: 1,
                            reason: traffic::WaitReason::ClassWindow,
                        },
                    },
                ),
            ))
        })
    });
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
        let q = Request::new("python-probe", op);
        let response = exchange(q.clone(), handler.clone()).await;
        assert_eq!(
            traffic::interpret_probe_for(&q, &response, op, capability),
            traffic::ProbeResult::Supported
        );
        assert_eq!(response.protocol_version, 1);
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    for reason in ["PYTHON_PYTEST", "PYTHON_UNITTEST"] {
        let payload = json!({"action":"TRAFFIC", "request":{"action":"ENQUEUE", "job_id":"a".repeat(32), "observer":arc_ipc::jobs::ProcessIdentity::capture(std::process::id()).unwrap(), "classification":{"classification":"TEST", "locality":"HOST_LOCAL", "reason":reason, "uncertainty":"NONE"}}});
        for op in [governor::OPERATION_JOB, traffic::OPERATION_BUN_SCRIPTS_JOB] {
            let mut q = Request::new("legacy-reject", op);
            q.payload = payload.clone();
            assert_eq!(
                exchange(q, handler.clone()).await.error.unwrap().code,
                arc_ipc::ERROR_INVALID_REQUEST
            );
        }
        let mut q = Request::new("python-admit", traffic::OPERATION_PYTHON_TESTS_JOB);
        q.payload = payload;
        let response = exchange(q, handler.clone()).await;
        assert_eq!(response.result.unwrap()["result"]["state"], "QUEUED");
    }
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn legacy_diagnostic_reply_projects_python_without_changing_persisted_facts() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../apps/desktop/tests/fixtures/traffic-scripts-wire.json"
    ))
    .unwrap();
    for reason in ["PYTHON_PYTEST", "PYTHON_UNITTEST"] {
        let mut record = fixture["results"][3].clone();
        record["classification"] = json!({"classification":"TEST", "locality":"HOST_LOCAL", "reason":reason, "uncertainty":"NONE"});
        let stored: diagnostics::DiagnosticResult = serde_json::from_value(record).unwrap();
        let original = stored.clone();
        let copied = stored.clone();
        let handler: Handler = Arc::new(move |input, _, _| {
            let Input::Diagnostics(id) = input else {
                panic!("wrong diagnostic interface")
            };
            assert_eq!(id, copied.job_id);
            let value = copied.clone();
            Box::pin(async move { Ok(Value::Diagnostics(Box::new(value))) })
        });
        for op in [
            diagnostics::OPERATION_JOB,
            diagnostics::OPERATION_PYTHON_JOB,
        ] {
            let mut q = Request::new("python-read", op);
            q.payload = json!({"job_id":stored.job_id});
            let response = exchange(q.clone(), handler.clone()).await;
            let actual = diagnostics::interpret_result(&q, &response).unwrap();
            assert_eq!(
                actual,
                if op == diagnostics::OPERATION_JOB {
                    stored.clone().legacy()
                } else {
                    stored.clone()
                }
            );
        }
        assert_eq!(stored, original);
        assert!(stored.admission.is_some());
    }
}

#[tokio::test]
async fn compatibility_projection_cannot_conceal_invalid_stored_python_facts() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../apps/desktop/tests/fixtures/traffic-scripts-wire.json"
    ))
    .unwrap();
    for invalid_classification in [false, true] {
        let mut record = fixture["results"][3].clone();
        record["classification"] = json!({"classification":if invalid_classification { "BUILD" } else { "TEST" }, "locality":"HOST_LOCAL", "reason":"PYTHON_PYTEST", "uncertainty":"NONE"});
        if !invalid_classification {
            record["admission"]["sequence"] = json!(0);
        }
        let stored: diagnostics::DiagnosticResult = serde_json::from_value(record).unwrap();
        assert!(!stored.is_valid());
        let handler: Handler = Arc::new(move |_, _, _| {
            let value = stored.clone();
            Box::pin(async move { Ok(Value::Diagnostics(Box::new(value))) })
        });
        for op in [
            diagnostics::OPERATION_JOB,
            diagnostics::OPERATION_PYTHON_JOB,
        ] {
            let mut q = Request::new("invalid-python", op);
            q.payload = json!({"job_id":"0123456789abcdef0123456789abcdef"});
            let response = exchange(q, handler.clone()).await;
            assert_eq!(response.error.unwrap().code, arc_ipc::ERROR_DB_BUSY);
        }
    }
}

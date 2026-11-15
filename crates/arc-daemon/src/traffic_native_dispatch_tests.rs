//! Native vocabulary enters the maintained writer only through its negotiated route.
use super::*;
use arc_ipc::{
    governor,
    traffic::{self, diagnostics, native},
};
use serde_json::json;

const FORMS: [(&str, &str); 7] = [
    ("BUILD", "GO_BUILD"),
    ("TEST", "GO_TEST"),
    ("LINT", "GO_VET"),
    ("BUILD", "SWIFT_BUILD"),
    ("TEST", "SWIFT_TEST"),
    ("BUILD", "CMAKE_BUILD"),
    ("TEST", "CTEST"),
];

#[tokio::test]
async fn native_capabilities_and_grammar_guards_precede_the_shared_writer() {
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
        assert!(classification.is_native_tool());
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
        (native::OPERATION_CAPABILITIES, native::CAPABILITY),
        (
            native::OPERATION_DIAGNOSTICS_CAPABILITIES,
            native::DIAGNOSTICS_CAPABILITY,
        ),
    ] {
        let request = Request::new("native-probe", op);
        let response = exchange(request.clone(), handler.clone()).await;
        assert_eq!(
            traffic::interpret_probe_for(&request, &response, op, capability),
            traffic::ProbeResult::Supported
        );
        assert_eq!(response.protocol_version, 1);
        let mut malformed = request;
        malformed.payload = json!({});
        assert_eq!(
            exchange(malformed, handler.clone())
                .await
                .error
                .unwrap()
                .code,
            arc_ipc::ERROR_INVALID_REQUEST
        );
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    for (class, reason) in FORMS {
        let payload = json!({"action":"TRAFFIC", "request":{"action":"ENQUEUE", "job_id":"a".repeat(32),
            "observer":arc_ipc::jobs::ProcessIdentity::capture(std::process::id()).unwrap(),
            "classification":{"classification":class,"locality":"HOST_LOCAL","reason":reason,"uncertainty":"NONE"}}});
        for op in [
            governor::OPERATION_JOB,
            traffic::OPERATION_BUN_SCRIPTS_JOB,
            traffic::OPERATION_PYTHON_TESTS_JOB,
        ] {
            let mut request = Request::new("legacy-reject", op);
            request.payload = payload.clone();
            assert_eq!(
                exchange(request, handler.clone()).await.error.unwrap().code,
                arc_ipc::ERROR_INVALID_REQUEST
            );
        }
        let mut request = Request::new("native-admit", native::OPERATION_JOB);
        request.payload = payload;
        assert_eq!(
            exchange(request, handler.clone()).await.result.unwrap()["result"]["state"],
            "QUEUED"
        );
    }
    assert_eq!(calls.load(Ordering::SeqCst), 7);
}

#[tokio::test]
async fn native_diagnostics_project_every_older_surface_without_mutating_permits() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../apps/desktop/tests/fixtures/traffic-scripts-wire.json"
    ))
    .unwrap();
    for (class, reason) in FORMS {
        let mut record = fixture["results"][1].clone();
        record["classification"] = json!({"classification":class,"locality":"HOST_LOCAL","reason":reason,"uncertainty":"NONE"});
        let stored: diagnostics::DiagnosticResult = serde_json::from_value(record).unwrap();
        let original = stored.clone();
        let copy = stored.clone();
        let handler: Handler = Arc::new(move |input, _, _| {
            let Input::Diagnostics(id) = input else {
                panic!("wrong diagnostic interface")
            };
            assert_eq!(id, copy.job_id);
            let value = copy.clone();
            Box::pin(async move { Ok(Value::Diagnostics(Box::new(value))) })
        });
        for op in [
            diagnostics::OPERATION_JOB,
            diagnostics::OPERATION_PYTHON_JOB,
            native::OPERATION_DIAGNOSTICS_JOB,
        ] {
            let mut request = Request::new("native-read", op);
            request.payload = json!({"job_id":stored.job_id});
            let response = exchange(request.clone(), handler.clone()).await;
            let actual = diagnostics::interpret_result(&request, &response).unwrap();
            assert_eq!(
                actual,
                if op == native::OPERATION_DIAGNOSTICS_JOB {
                    stored.clone()
                } else {
                    stored.clone().before_native()
                }
            );
        }
        assert_eq!(stored, original);
        assert!(stored.admission.as_ref().unwrap().permit_active);
    }
}

use super::*;
use arc_ipc::traffic::{self, diagnostics};

async fn without_handler(request: Request) -> Response {
    let (server, client) = UnixStream::pair().unwrap();
    let metrics = Arc::new(RuntimeMetrics::new(&RuntimeLimits::default()));
    let task = tokio::spawn(async move {
        assert!(
            dispatch(
                &server,
                &request,
                &RuntimeLimits::test_profile(),
                None,
                &metrics
            )
            .await
        );
    });
    let response = crate::transport_test_support::response(&client).await;
    task.await.unwrap();
    response
}

#[tokio::test]
async fn unavailable_operations_reject_before_decoding_any_new_payload_vocabulary() {
    for op in [
        traffic::OPERATION_BUN_SCRIPTS_CAPABILITIES,
        traffic::OPERATION_BUN_SCRIPTS_JOB,
        diagnostics::OPERATION_CAPABILITIES,
        diagnostics::OPERATION_JOB,
    ] {
        for payload in [
            serde_json::Value::Null,
            serde_json::json!(["TYPECHECK", "BUN_SCRIPT_TYPECHECK"]),
            serde_json::json!({"action":"FUTURE_ACTION","class":"UNKNOWN_FUTURE"}),
        ] {
            let mut request = Request::new("old-peer", op);
            request.payload = payload;
            assert_eq!(
                without_handler(request).await.error.unwrap().code,
                arc_ipc::ERROR_UNSUPPORTED_CAPABILITY
            );
        }
    }
}

#[tokio::test]
async fn malformed_or_oversized_diagnostic_and_script_requests_never_admit_writer_work() {
    let calls = Arc::new(AtomicUsize::new(0));
    let count = Arc::clone(&calls);
    let handler: Handler = Arc::new(move |_, _, _| {
        count.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Err(arc_ipc::ERROR_DB_BUSY) })
    });
    for (op, payload) in [
        (
            traffic::OPERATION_BUN_SCRIPTS_CAPABILITIES,
            serde_json::json!({}),
        ),
        (diagnostics::OPERATION_CAPABILITIES, serde_json::json!({})),
        (
            diagnostics::OPERATION_JOB,
            serde_json::json!({"job_id":"a".repeat(32),"extra":1}),
        ),
        (
            diagnostics::OPERATION_JOB,
            serde_json::json!({"job_id":"a".repeat(513)}),
        ),
        (
            diagnostics::OPERATION_JOB,
            serde_json::json!(["a".repeat(32)]),
        ),
        (
            traffic::OPERATION_BUN_SCRIPTS_JOB,
            serde_json::json!({"action":"ENQUEUE","classification":"TYPECHECK"}),
        ),
        (
            traffic::OPERATION_BUN_SCRIPTS_JOB,
            serde_json::json!({"action":"TRAFFIC","request":{"action":"STATUS","job_id":"a".repeat(16385)}}),
        ),
    ] {
        let mut q = Request::new("invalid", op);
        q.payload = payload;
        assert_eq!(
            exchange(q, Arc::clone(&handler)).await.error.unwrap().code,
            arc_ipc::ERROR_INVALID_REQUEST
        );
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test(start_paused = true)]
async fn diagnostic_read_timeout_and_busy_are_unavailable_not_unsupported_or_not_found() {
    for pending in [false, true] {
        let handler: Handler = Arc::new(move |_, _, _| {
            Box::pin(async move {
                if pending {
                    std::future::pending::<()>().await;
                }
                Err(arc_ipc::ERROR_DB_BUSY)
            })
        });
        let mut q = Request::new("timeout", diagnostics::OPERATION_JOB);
        q.payload = serde_json::json!({"job_id":"a".repeat(32)});
        let response = exchange(q, handler).await;
        assert_eq!(response.error.unwrap().code, arc_ipc::ERROR_DB_BUSY);
        assert!(response.result.is_none());
    }
}

#[tokio::test]
async fn malformed_writer_diagnostics_never_cross_the_negotiated_response_boundary() {
    for wrong_id in [false, true] {
        let handler: Handler = Arc::new(move |_, _, _| {
            Box::pin(async move {
                Ok(Value::Diagnostics(Box::new(
                    diagnostics::DiagnosticResult {
                        schema_version: 1,
                        job_id: arc_ipc::jobs::JobId::try_from(if wrong_id {
                            "b".repeat(32)
                        } else {
                            "a".repeat(32)
                        })
                        .unwrap(),
                        recorded_at_ms: 1,
                        liveness_fresh: !wrong_id,
                        classification: None,
                        admission: None,
                        limits: Default::default(),
                    },
                )))
            })
        });
        let mut q = Request::new("bound-result", diagnostics::OPERATION_JOB);
        q.payload = serde_json::json!({"job_id":"a".repeat(32)});
        let response = exchange(q, handler).await;
        assert_eq!(response.error.unwrap().code, arc_ipc::ERROR_DB_BUSY);
        assert!(response.result.is_none());
    }
}

//! The capability probe and the negotiated decision read, with no writer.
use super::*;
use arc_ipc::traffic::fairness as wire;
use arc_policy::{DecisionReason, FairnessReason, NegotiatedReason};
use serde_json::json;

const JOB: &str = "11111111111111111111111111111111";

fn request(operation: &str, payload: serde_json::Value) -> Request {
    let mut request = Request::new("fairness", operation);
    request.payload = payload;
    request
}

fn reads(reason: Option<NegotiatedReason>, calls: Arc<AtomicUsize>) -> Handler {
    Arc::new(move |input, _, _| {
        calls.fetch_add(1, Ordering::SeqCst);
        let known = matches!(&input, Input::Decision(job) if job.as_str() == JOB);
        Box::pin(async move {
            if known {
                Ok(Value::Decision(reason))
            } else {
                Err(arc_ipc::ERROR_INVALID_REQUEST)
            }
        })
    })
}

#[tokio::test]
async fn capability_is_advertised_without_entering_the_writer() {
    let calls = Arc::new(AtomicUsize::new(0));
    let handler = reads(None, Arc::clone(&calls));
    let probe = request(wire::OPERATION_CAPABILITIES, serde_json::Value::Null);
    let response = exchange(probe.clone(), Arc::clone(&handler)).await;
    assert_eq!(
        arc_ipc::traffic::interpret_probe_for(
            &probe,
            &response,
            wire::OPERATION_CAPABILITIES,
            wire::CAPABILITY
        ),
        arc_ipc::traffic::ProbeResult::Supported
    );
    let malformed = request(wire::OPERATION_CAPABILITIES, json!({"capability":"x"}));
    assert_eq!(
        exchange(malformed, handler).await.error.unwrap().code,
        arc_ipc::ERROR_INVALID_REQUEST
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn decision_read_returns_the_raw_reason_of_the_requested_job() {
    for reason in [
        Some(NegotiatedReason::Fairness(FairnessReason::ProjectFairTurn)),
        Some(NegotiatedReason::Recorded(DecisionReason::ClassWindow)),
        None,
    ] {
        let calls = Arc::new(AtomicUsize::new(0));
        let query = request(wire::OPERATION_DECISION, json!({"job_id":JOB}));
        let response = exchange(query.clone(), reads(reason, Arc::clone(&calls))).await;
        let result = wire::interpret_decision(&query, &response).unwrap();
        assert_eq!(result.reason, reason);
        assert_eq!(result.job_id.as_str(), JOB);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
    let fair = request(wire::OPERATION_DECISION, json!({"job_id":JOB}));
    let handler = reads(
        Some(NegotiatedReason::Fairness(FairnessReason::ProjectFairTurn)),
        Arc::new(AtomicUsize::new(0)),
    );
    assert_eq!(
        exchange(fair, handler).await.result.unwrap(),
        json!({"schema_version":1,"job_id":JOB,"reason":"project_fair_turn"})
    );
}

#[tokio::test]
async fn malformed_queries_and_writer_failures_are_errors_never_an_absent_reason() {
    let calls = Arc::new(AtomicUsize::new(0));
    let handler = reads(None, Arc::clone(&calls));
    for payload in [
        json!({}),
        json!({"job_id":JOB,"path":"/tmp"}),
        json!({"job_id":[JOB]}),
        json!({"job_id":"x".repeat(513)}),
    ] {
        let query = request(wire::OPERATION_DECISION, payload);
        assert_eq!(
            exchange(query, Arc::clone(&handler))
                .await
                .error
                .unwrap()
                .code,
            arc_ipc::ERROR_INVALID_REQUEST
        );
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let query = request(wire::OPERATION_DECISION, json!({"job_id":JOB}));
    let busy: Handler = Arc::new(|_, _, _| Box::pin(async { Err(arc_ipc::ERROR_DB_BUSY) }));
    let other: Handler = Arc::new(|_, _, _| Box::pin(async { Ok(Value::Acknowledged) }));
    for handler in [busy, other] {
        let response = exchange(query.clone(), handler).await;
        assert_eq!(
            response.error.as_ref().unwrap().code,
            arc_ipc::ERROR_DB_BUSY
        );
        assert!(wire::interpret_decision(&query, &response).is_err());
    }
}

#[tokio::test]
async fn a_daemon_without_the_writer_rejects_both_operations_as_unsupported() {
    for query in [
        request(wire::OPERATION_CAPABILITIES, serde_json::Value::Null),
        request(wire::OPERATION_DECISION, json!({"job_id":JOB})),
    ] {
        let (server, client) = UnixStream::pair().unwrap();
        let metrics = Arc::new(RuntimeMetrics::new(&RuntimeLimits::default()));
        let sent = query.clone();
        let task = tokio::spawn(async move {
            dispatch(
                &server,
                &sent,
                &RuntimeLimits::test_profile(),
                None,
                &metrics,
            )
            .await
        });
        let response = crate::transport_test_support::response(&client).await;
        assert!(task.await.unwrap());
        assert_eq!(
            response.error.unwrap().code,
            arc_ipc::ERROR_UNSUPPORTED_CAPABILITY
        );
    }
    // Any other protocol version is left to the ordinary dispatcher.
    let mut other = request(wire::OPERATION_CAPABILITIES, serde_json::Value::Null);
    other.protocol_version += 1;
    let (server, _client) = UnixStream::pair().unwrap();
    let metrics = Arc::new(RuntimeMetrics::new(&RuntimeLimits::default()));
    assert!(
        !fairness::dispatch(
            &server,
            &other,
            &RuntimeLimits::test_profile(),
            None,
            &metrics
        )
        .await
    );
}

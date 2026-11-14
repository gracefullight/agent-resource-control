use super::*;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
#[path = "traffic_scripts_dispatch_tests.rs"]
mod scripts;
fn fallback() -> policy::EffectivePolicy {
    use arc_policy::{
        evaluator::{evaluate, MissingObservation},
        ObservationProvider,
    };
    let mut result = None;
    MissingObservation.with_observation(&mut |input| result = Some(evaluate(input)));
    result.unwrap()
}

async fn exchange(request: Request, handler: Handler) -> Response {
    let (server, client) = UnixStream::pair().unwrap();
    let metrics = Arc::new(RuntimeMetrics::new(&RuntimeLimits::default()));
    let task = tokio::spawn(async move {
        assert!(
            dispatch(
                &server,
                &request,
                &RuntimeLimits::test_profile(),
                Some(&handler),
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
async fn invalid_shapes_and_unsolicited_ack_never_enter_the_writer() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = Arc::clone(&calls);
    let handler: Handler = Arc::new(move |_, _, _| {
        observed.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Err(arc_ipc::ERROR_DB_BUSY) })
    });
    for (operation, payload) in [
        (policy::OPERATION_POLICY_CAPABILITIES, serde_json::json!({})),
        (policy::OPERATION_POLICY_CURRENT, serde_json::json!([])),
        (
            policy::OPERATION_POLICY_JOB,
            serde_json::json!({"job_id":"11111111111111111111111111111111","extra":true}),
        ),
        (
            policy::OPERATION_POLICY_JOB,
            serde_json::json!({"job_id":["11111111111111111111111111111111"]}),
        ),
        (
            policy::OPERATION_POLICY_JOB,
            serde_json::json!({"job_id":"x".repeat(513)}),
        ),
        (launch::OPERATION_TRAFFIC_LAUNCH_ACK, serde_json::json!({})),
        (launch::OPERATION_TRAFFIC_LAUNCH, serde_json::json!({})),
    ] {
        let mut request = Request::new("invalid", operation);
        request.payload = payload;
        assert_eq!(
            exchange(request, Arc::clone(&handler))
                .await
                .error
                .unwrap()
                .code,
            arc_ipc::ERROR_INVALID_REQUEST
        );
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let request = policy::capabilities_request("capability");
    assert!(
        policy::interpret_capabilities(&request, &exchange(request.clone(), handler).await).is_ok()
    );
}
#[tokio::test(start_paused = true)]
async fn writer_unavailability_and_absolute_read_timeout_remain_distinct_from_malformed() {
    for pending in [false, true] {
        let handler: Handler = Arc::new(move |_, _, _| {
            Box::pin(async move {
                if pending {
                    std::future::pending::<()>().await;
                }
                Err(arc_ipc::ERROR_DB_BUSY)
            })
        });
        let request = policy::current_request("unavailable");
        let response = exchange(request.clone(), handler).await;
        assert_eq!(
            policy::interpret_current(&request, &response),
            Err(policy::PolicyReadError::Unavailable)
        );
    }
}
#[tokio::test(start_paused = true)]
async fn missing_ack_expires_the_same_one_shot_fence_without_renewal() {
    struct Guard(Arc<AtomicBool>);
    impl Drop for Guard {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    let released = Arc::new(AtomicBool::new(false));
    let owned = Arc::clone(&released);
    let handler: Handler = Arc::new(move |_, _, _| {
        let guard = Guard(Arc::clone(&owned));
        Box::pin(async move {
            Ok(Value::Launch {
                result: launch::LaunchResult::Ready {
                    permit_generation: 1,
                    policy: fallback(),
                },
                fence: Some((Instant::now() + Duration::from_millis(20), Box::new(guard))),
            })
        })
    });
    let (server, client) = UnixStream::pair().unwrap();
    let observer =
        arc_core::process_identity::ProcessIdentity::capture(std::process::id()).unwrap();
    let mut request = Request::new("release", launch::OPERATION_TRAFFIC_LAUNCH);
    request.payload = serde_json::json!(launch::LaunchRequest {
        job_id: arc_ipc::jobs::JobId::try_from("11111111111111111111111111111111".to_owned())
            .unwrap(),
        observer,
        permit_generation: 1
    });
    let metrics = Arc::new(RuntimeMetrics::new(&RuntimeLimits::default()));
    let task = tokio::spawn(async move {
        dispatch(
            &server,
            &request,
            &RuntimeLimits::default(),
            Some(&handler),
            &metrics,
        )
        .await
    });
    let response = crate::transport_test_support::response(&client).await;
    assert!(matches!(
        launch::decode_launch_result(response.result.as_ref().unwrap()).unwrap(),
        launch::LaunchResult::Ready { .. }
    ));
    assert!(task.await.unwrap());
    assert!(released.load(Ordering::SeqCst));
    drop(client);
}

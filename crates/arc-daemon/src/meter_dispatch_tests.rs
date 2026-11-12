//! Deterministic in-process transport test; no production daemon is launched.
use super::*;
use crate::transport_test_support::{request, setup};

#[tokio::test]
async fn meter_dispatch_preserves_version_payload_and_legacy_health_behavior() {
    let (limits, metrics) = setup();
    let (server, client) = UnixStream::pair().unwrap();
    let requests = Requests::new(&limits, Arc::clone(&metrics)).unwrap();
    let task = tokio::spawn(handle_with_meter(
        server,
        limits,
        requests,
        Arc::clone(&metrics),
        |_| async { panic!("Meter must not invoke database diagnostics") },
        |_, _| async { panic!("Meter must not invoke job handling") },
        (
            |_, _, _| async { panic!("Meter must not invoke Traffic handling") },
            None,
        ),
    ));
    let mut query = arc_ipc::meter::snapshot_request("meter");
    query.protocol_version = 99;
    assert_eq!(
        request(&client, &query).await.error.unwrap().code,
        arc_ipc::ERROR_PROTOCOL_INCOMPATIBLE
    );
    query.protocol_version = 1;
    for invalid in [
        serde_json::json!({}),
        serde_json::json!({"path":"/other"}),
        serde_json::json!({"pid":1}),
    ] {
        query.payload = invalid;
        assert_eq!(
            request(&client, &query).await.error.unwrap().code,
            arc_ipc::ERROR_INVALID_REQUEST
        );
    }
    query.payload = serde_json::Value::Null;
    let response = request(&client, &query).await;
    assert_eq!(
        response.error.unwrap().code,
        arc_ipc::meter::ERROR_METER_UNAVAILABLE
    );
    assert!(response.result.is_none());
    for operation in [arc_ipc::OPERATION_HELLO, arc_ipc::OPERATION_HEALTH] {
        assert_eq!(
            request(&client, &Request::new("legacy", operation))
                .await
                .status,
            arc_ipc::STATUS_OK
        );
    }
    drop(client);
    task.await.unwrap();
    assert!(Gauge::ALL
        .iter()
        .all(|gauge| metrics.snapshot().gauge(*gauge).current == 0));
}

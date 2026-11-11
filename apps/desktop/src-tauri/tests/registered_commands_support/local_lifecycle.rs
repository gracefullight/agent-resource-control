#[test]
fn registered_timeout_and_shutdown_are_bounded_and_redacted() {
    let (release_tx, release_rx) = mpsc::channel();
    let peer = Peer::new(1, move |stream, _| {
        stream.write_all(&100u32.to_be_bytes()).unwrap();
        stream.write_all(b"{").unwrap();
        release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    });
    let app = App::new(peer.bridge());
    let start = Instant::now();
    assert_eq!(
        app.invoke("daemon_health", json!({})),
        Err(error("uncertain", true))
    );
    assert!(start.elapsed() < Duration::from_millis(2300));
    release_tx.send(()).unwrap();

    let (received_tx, received_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let peer = Peer::new(1, move |_, _| {
        received_tx.send(()).unwrap();
        release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
    });
    let bridge = peer.bridge();
    let app = App::new(bridge.clone());
    let window = app.window.clone();
    let read = thread::spawn(move || invoke(&window, "daemon_health", json!({})));
    received_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    let start = Instant::now();
    assert_eq!(
        app.invoke("recent_jobs", json!({})),
        Err(error("busy", true))
    );
    assert!(start.elapsed() < Duration::from_secs(1));
    bridge.shutdown();
    assert_eq!(read.join().unwrap(), Err(error("cancelled", false)));
    assert_eq!(
        app.invoke("daemon_health", json!({})),
        Err(error("cancelled", false))
    );
    release_tx.send(()).unwrap();
}

#[test]
fn unsafe_receipt_clock_is_local_and_does_not_forge_a_timestamp() {
    let peer = Peer::result(health());
    let bridge = DesktopBridge::with_socket_and_clock(peer.path.clone(), || MAX_TIMESTAMP + 1);
    assert_eq!(
        App::new(bridge).invoke("daemon_health", json!({})),
        Err(error("local", false))
    );
}

#[test]
fn public_native_bridge_matches_handler_validation_and_safe_failure_mapping() {
    let peer = Peer::result(health());
    assert_eq!(
        peer.bridge().daemon_health().unwrap().received_at_ms,
        RECEIVED
    );
    let peer = Peer::result(recent());
    assert_eq!(peer.bridge().recent_jobs().unwrap().data.limit, 50);
    let peer = Peer::result(found(snapshot(17)));
    assert_eq!(
        json!(peer.bridge().job_by_id(JOB).unwrap().data),
        projected(17)
    );
    let bridge = DesktopBridge::from_socket_path("/missing/arc-desktop-fixture.sock".into());
    assert!(!format!("{bridge:?}").contains("/missing"));
    assert_eq!(
        bridge.job_by_id(SENTINEL),
        Err(DesktopError::new(ErrorCode::InvalidRequest))
    );
    bridge.shutdown();
    assert_eq!(
        bridge.daemon_health(),
        Err(DesktopError::new(ErrorCode::Cancelled))
    );
    use arc_ipc::client::Failure as F;
    for failure in [
        F::Unavailable,
        F::Uncertain,
        F::Busy,
        F::Rejected,
        F::Protocol,
        F::InvalidReply,
        F::InvalidEndpoint,
        F::Local,
        F::Witness,
        F::Cancelled,
        F::Unsupported,
    ] {
        let rejected = DesktopError::from(failure);
        assert!(!json!(rejected).to_string().contains(SENTINEL));
        assert_eq!(
            rejected.retryable,
            matches!(failure, F::Unavailable | F::Uncertain | F::Busy)
        );
    }
}

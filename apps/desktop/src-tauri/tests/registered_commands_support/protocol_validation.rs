#[test]
fn registered_commands_reject_malformed_envelopes_and_wrong_protocol() {
    for mode in 0..11 {
        let peer = Peer::new(1, move |stream, request| match mode {
            0 => stream.write_all(&0u32.to_be_bytes()).unwrap(),
            1 => stream
                .write_all(&(arc_ipc::MAX_FRAME_BYTES + 1).to_be_bytes())
                .unwrap(),
            2 => arc_ipc::write_frame(stream, b"{malformed").unwrap(),
            3 => arc_ipc::write_frame(stream, b"[]").unwrap(),
            _ => {
                let mut response = Response::ok(request.request_id, health());
                match mode {
                    4 => response.protocol_version = 2,
                    5 => response.request_id = SENTINEL.into(),
                    6 => response.error = Response::error("x", SENTINEL).error,
                    7 => response.status = SENTINEL.into(),
                    8 => response.result = None,
                    9 => {
                        response = Response::error(response.request_id, "REJECTED");
                        response.error.as_mut().unwrap().message = Some(SENTINEL.into());
                    }
                    _ => {
                        response = Response::error(
                            response.request_id,
                            arc_ipc::ERROR_PROTOCOL_INCOMPATIBLE,
                        )
                    }
                }
                arc_ipc::write_response(stream, &response).unwrap();
            }
        });
        let rejected = App::new(peer.bridge())
            .invoke("daemon_health", json!({}))
            .unwrap_err();
        assert_eq!(
            rejected,
            error(
                if mode == 4 || mode == 10 {
                    "protocol"
                } else if mode == 9 {
                    "rejected"
                } else {
                    "invalid_reply"
                },
                false
            ),
            "mode {mode}"
        );
        assert!(!rejected.to_string().contains(SENTINEL));
    }
    for invalid in [
        json!(["running", 1, "off"]),
        json!({"daemon":"running","protocol_version":1,"sweeper":"off","command":SENTINEL}),
        json!({"daemon":SENTINEL,"protocol_version":1,"sweeper":"off"}),
        json!({"daemon":"running","protocol_version":1,"sweeper":"on"}),
        json!({"daemon":"running","protocol_version":2,"sweeper":"off"}),
    ] {
        let expected = if invalid.get("protocol_version") == Some(&json!(2)) {
            "protocol"
        } else {
            "invalid_reply"
        };
        let peer = Peer::result(invalid);
        assert_eq!(
            App::new(peer.bridge()).invoke("daemon_health", json!({})),
            Err(error(expected, false))
        );
    }
}

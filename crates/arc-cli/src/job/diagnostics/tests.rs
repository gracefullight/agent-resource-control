//! Native framed read compatibility, with no daemon or test-tool launch.
use super::*;
use arc_ipc::{read_message, write_response, Response, ERROR_UNSUPPORTED_CAPABILITY};
use std::{
    io,
    os::unix::net::{UnixListener, UnixStream},
    path::PathBuf,
    thread,
};

fn accept(listener: &UnixListener) -> UnixStream {
    let end = Instant::now() + Duration::from_secs(60);
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(60)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_secs(60)))
                    .unwrap();
                return stream;
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                assert!(
                    Instant::now() < end,
                    "diagnostic peer did not receive the expected request"
                );
                thread::sleep(Duration::from_millis(1));
            }
            Err(e) => panic!("diagnostic peer accept: {e}"),
        }
    }
}

#[test]
fn python_diagnostics_negotiate_and_only_explicit_unsupported_uses_legacy_reads() {
    for mode in [
        "python",
        "old",
        "unsupported",
        "bad-cap",
        "array-cap",
        "wrong-id",
        "wrong-protocol",
        "invalid-fact",
    ] {
        let fixture = crate::native_fixture::Fixture::with_binary(PathBuf::new());
        let listener = UnixListener::bind(&fixture.socket).unwrap();
        listener.set_nonblocking(true).unwrap();
        let wire: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../../apps/desktop/tests/fixtures/traffic-scripts-wire.json"
        ))
        .unwrap();
        let mut payload = wire["results"][3].clone();
        payload["classification"] = json!({"classification":"TEST", "locality":"HOST_LOCAL", "reason":"PYTHON_PYTEST", "uncertainty":"NONE"});
        let stored: DiagnosticResult = serde_json::from_value(payload.clone()).unwrap();
        let expected = if mode == "old" {
            stored.clone().legacy()
        } else {
            stored.clone()
        };
        let server = thread::spawn(move || {
            let count = match mode {
                "old" => 3,
                "unsupported" | "python" | "invalid-fact" => 2,
                _ => 1,
            };
            let mut requests = Vec::new();
            for index in 0..count {
                let mut stream = accept(&listener);
                let request: Request = read_message(&mut stream).unwrap();
                assert_eq!(request.protocol_version, 1);
                let expected_op = if index == 0 {
                    OPERATION_PYTHON_CAPABILITIES
                } else if matches!(mode, "old" | "unsupported") && index == 1 {
                    OPERATION_CAPABILITIES
                } else if mode == "old" {
                    OPERATION_JOB
                } else {
                    OPERATION_PYTHON_JOB
                };
                assert_eq!(request.operation, expected_op);
                let response = if index == 0 && matches!(mode, "old" | "unsupported")
                    || mode == "unsupported"
                {
                    Response::error(&request.request_id, ERROR_UNSUPPORTED_CAPABILITY)
                } else if expected_op.ends_with("capabilities") {
                    assert!(request.payload.is_null());
                    let capability = if expected_op == OPERATION_CAPABILITIES {
                        CAPABILITY
                    } else {
                        PYTHON_CAPABILITY
                    };
                    let value = match mode {
                        "bad-cap" => json!({"capability":"wrong"}),
                        "array-cap" => json!([capability]),
                        _ => json!({"capability":capability}),
                    };
                    let mut response = Response::ok(&request.request_id, value);
                    if mode == "wrong-id" {
                        response.request_id = "different".into();
                    }
                    if mode == "wrong-protocol" {
                        response.protocol_version = 2;
                    }
                    response
                } else {
                    assert_eq!(request.payload, json!({"job_id":stored.job_id}));
                    let result = if mode == "old" {
                        stored.clone().legacy()
                    } else {
                        stored.clone()
                    };
                    let mut value = json!(result);
                    if mode == "invalid-fact" {
                        value["classification"]["locality"] = json!("REMOTE");
                    }
                    Response::ok(&request.request_id, value)
                };
                write_response(&mut stream, &response).unwrap();
                requests.push(request.operation);
            }
            requests
        });
        let result = read(&fixture.socket, &expected.job_id);
        let requests = server.join().unwrap();
        match mode {
            "python" | "old" => {
                assert_eq!(result, Ok(expected.clone()));
                let rendered = render(&expected);
                assert!(rendered.contains("Liveness refreshed: false"));
                assert!(rendered.contains(if mode == "python" {
                    "PYTHON_PYTEST"
                } else {
                    "UNRESOLVED_COMMAND"
                }));
            }
            "unsupported" => assert_eq!(result, Err(Failure::Unsupported)),
            "wrong-protocol" => assert_eq!(result, Err(Failure::Protocol)),
            _ => assert_eq!(result, Err(Failure::InvalidReply), "{mode}"),
        }
        assert_eq!(
            requests.contains(&OPERATION_CAPABILITIES.to_owned()),
            matches!(mode, "old" | "unsupported")
        );
    }
}

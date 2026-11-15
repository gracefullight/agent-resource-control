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
fn diagnostics_negotiate_native_python_and_legacy_without_hiding_invalid_replies() {
    use arc_ipc::traffic::native;
    for mode in [
        "native",
        "python",
        "old",
        "unsupported",
        "bad-cap",
        "array-cap",
        "wrong-id",
        "wrong-protocol",
        "invalid-fact",
        "native-on-python",
        "native-on-old",
        "python-on-old",
    ] {
        let fixture = crate::native_fixture::Fixture::with_binary(PathBuf::new());
        let listener = UnixListener::bind(&fixture.socket).unwrap();
        listener.set_nonblocking(true).unwrap();
        let wire: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../../apps/desktop/tests/fixtures/traffic-scripts-wire.json"
        ))
        .unwrap();
        let mut payload = wire["results"][3].clone();
        let python = matches!(mode, "python" | "old" | "python-on-old");
        payload["classification"] = json!({"classification":"TEST","locality":"HOST_LOCAL",
            "reason":if python { "PYTHON_PYTEST" } else { "GO_TEST" },"uncertainty":"NONE"});
        let stored: DiagnosticResult = serde_json::from_value(payload).unwrap();
        let expected = if mode == "old" {
            stored.clone().legacy()
        } else {
            stored.clone()
        };
        let routes = [
            (
                native::OPERATION_DIAGNOSTICS_CAPABILITIES,
                native::OPERATION_DIAGNOSTICS_JOB,
                native::DIAGNOSTICS_CAPABILITY,
            ),
            (
                OPERATION_PYTHON_CAPABILITIES,
                OPERATION_PYTHON_JOB,
                PYTHON_CAPABILITY,
            ),
            (OPERATION_CAPABILITIES, OPERATION_JOB, CAPABILITY),
        ];
        let selected = match mode {
            "python" | "native-on-python" => 1,
            "old" | "native-on-old" | "python-on-old" => 2,
            "unsupported" => 3,
            _ => 0,
        };
        let invalid_probe = matches!(
            mode,
            "bad-cap" | "array-cap" | "wrong-id" | "wrong-protocol"
        );
        let count = if selected == 3 {
            3
        } else {
            selected + 1 + usize::from(!invalid_probe)
        };
        let server = thread::spawn(move || {
            let mut requests = Vec::new();
            for index in 0..count {
                let mut stream = accept(&listener);
                let request: Request = read_message(&mut stream).unwrap();
                assert_eq!(request.protocol_version, 1);
                let expected_op = if index <= selected {
                    routes[index].0
                } else {
                    routes[selected].1
                };
                assert_eq!(request.operation, expected_op);
                let mut response = if index < selected {
                    assert!(request.payload.is_null());
                    Response::error(&request.request_id, ERROR_UNSUPPORTED_CAPABILITY)
                } else if index == selected {
                    assert!(request.payload.is_null());
                    let capability = routes[selected].2;
                    let value = match mode {
                        "bad-cap" => json!({"capability":"wrong"}),
                        "array-cap" => json!([capability]),
                        _ => json!({"capability":capability}),
                    };
                    Response::ok(&request.request_id, value)
                } else {
                    assert_eq!(request.payload, json!({"job_id":stored.job_id}));
                    let mut value = json!(if mode == "old" {
                        stored.clone().legacy()
                    } else {
                        stored.clone()
                    });
                    if mode == "invalid-fact" {
                        value["classification"]["locality"] = json!("REMOTE");
                    }
                    Response::ok(&request.request_id, value)
                };
                if mode == "wrong-id" {
                    response.request_id = "different".into();
                }
                if mode == "wrong-protocol" {
                    response.protocol_version = 2;
                }
                write_response(&mut stream, &response).unwrap();
                requests.push(request.operation);
            }
            requests
        });
        let result = read(&fixture.socket, &expected.job_id);
        let requests = server.join().unwrap();
        match mode {
            "native" | "python" | "old" => {
                assert_eq!(result, Ok(expected.clone()));
                let rendered = render(&expected);
                assert!(rendered.contains("Liveness refreshed: false"));
                assert!(rendered.contains(match mode {
                    "native" => "GO_TEST",
                    "python" => "PYTHON_PYTEST",
                    _ => "UNRESOLVED_COMMAND",
                }));
            }
            "unsupported" => assert_eq!(result, Err(Failure::Unsupported)),
            "wrong-protocol" => assert_eq!(result, Err(Failure::Protocol)),
            _ => assert_eq!(result, Err(Failure::InvalidReply), "{mode}"),
        }
        assert_eq!(requests.len(), count);
        assert_eq!(requests[0], native::OPERATION_DIAGNOSTICS_CAPABILITIES);
    }
}

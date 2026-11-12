//! Deterministic same-user socket fixtures through the production Tauri router.
//! These validate registration and transport, not production daemon admission.
use super::consumer_reads::{read, reply, Socket};
use arc_desktop::{
    bridge::DesktopBridge,
    configure_with_ports,
    error::DesktopError,
    lifecycle::{CreditLink, CreditOpener, DesktopPorts, DesktopShell},
};
use arc_ipc::{jobs::JobId, policy::*, write_response, Request, Response};
use serde_json::{json, Value};
use std::{
    os::unix::net::{UnixListener, UnixStream},
    sync::Arc,
    thread,
    time::{Duration, Instant},
};
use tauri::{
    ipc::{CallbackFn, InvokeBody},
    test::{mock_builder, mock_context, noop_assets, MockRuntime},
    webview::InvokeRequest,
};

const JOB: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

struct Ports;
impl DesktopShell for Ports {
    fn hide_main(&self) -> Result<(), DesktopError> {
        Ok(())
    }
    fn show_main(&self) -> Result<(), DesktopError> {
        Ok(())
    }
    fn unminimize_main(&self) -> Result<(), DesktopError> {
        Ok(())
    }
    fn focus_main(&self) -> Result<(), DesktopError> {
        Ok(())
    }
    fn exit_desktop(&self) {}
}
impl CreditOpener for Ports {
    fn open(&self, _: CreditLink) -> Result<(), DesktopError> {
        Ok(())
    }
}

struct App {
    _app: tauri::App<MockRuntime>,
    window: tauri::WebviewWindow<MockRuntime>,
}
impl App {
    fn new(bridge: DesktopBridge) -> Self {
        let app = configure_with_ports(
            mock_builder(),
            bridge,
            DesktopPorts {
                shell: Arc::new(Ports),
                opener: Arc::new(Ports),
            },
        )
        .build(mock_context(noop_assets()))
        .unwrap();
        let window = tauri::WebviewWindowBuilder::new(&app, "main", Default::default())
            .build()
            .unwrap();
        Self { _app: app, window }
    }
    fn invoke(&self, command: &str, body: Value) -> Result<Value, Value> {
        tauri::test::get_ipc_response(
            &self.window,
            InvokeRequest {
                cmd: command.into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: "tauri://localhost".parse().unwrap(),
                body: InvokeBody::Json(body),
                headers: Default::default(),
                invoke_key: tauri::test::INVOKE_KEY.into(),
            },
        )
        .map(|body| body.deserialize().unwrap())
    }
}

fn job() -> JobPolicyResult {
    JobPolicyResult {
        schema_version: 1,
        job_id: JobId::try_from(JOB.to_owned()).unwrap(),
        effective: super::fixture::fallback(),
        decision_availability: DecisionAvailability::Available,
        last_decision: Some(RecordedDecision {
            policy_version: POLICY_VERSION.into(),
            kind: DecisionKind::Wait,
            reason: DecisionReason::ClassWindow,
            decided_at_ms: 100,
            permit_generation: None,
            observation: super::fixture::fallback().observation,
        }),
    }
}

fn next_request(listener: &UnixListener) -> (UnixStream, Request) {
    listener.set_nonblocking(true).unwrap();
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        match listener.accept() {
            Ok((mut stream, _)) => {
                stream.set_nonblocking(false).unwrap();
                let request = read(&mut stream);
                return (stream, request);
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(
                    Instant::now() < deadline,
                    "registered command never reached socket"
                );
                thread::sleep(Duration::from_millis(1));
            }
            Err(error) => panic!("registered fixture accept failed: {error}"),
        }
    }
}

fn capability(listener: &UnixListener) {
    let (mut stream, request) = next_request(listener);
    assert_eq!(request.operation, OPERATION_POLICY_CAPABILITIES);
    assert!(request.payload.is_null());
    reply(&mut stream, &request, json!(Capabilities::supported()));
}

#[test]
fn registered_policy_commands_reach_the_authenticated_bridge_and_preserve_decision_metadata() {
    let socket = Socket::new();
    let app = App::new(socket.bridge());
    let server = thread::spawn(move || {
        for operation in [OPERATION_POLICY_CURRENT, OPERATION_POLICY_JOB] {
            capability(&socket.listener);
            let (mut stream, request) = next_request(&socket.listener);
            assert_eq!(request.protocol_version, 1);
            assert_eq!(request.operation, operation);
            if operation == OPERATION_POLICY_CURRENT {
                assert!(request.payload.is_null());
                reply(&mut stream, &request, json!(super::fixture::fallback()));
            } else {
                assert_eq!(request.payload, json!({"job_id": JOB}));
                reply(&mut stream, &request, json!(job()));
            }
        }
    });
    let current = app.invoke("policy_current", json!({}));
    let selected = app.invoke("policy_job", json!({"job_id": JOB}));
    // Always join the peer before asserting, preserving its socket/error evidence.
    server.join().unwrap();
    let current = current.unwrap();
    assert_eq!(
        current,
        json!({"data": super::fixture::fallback(), "received_at_ms": 99000})
    );
    assert_eq!(
        decode_current(&current["data"]).unwrap().mode,
        PolicyMode::L0
    );
    let selected = selected.unwrap();
    assert_eq!(selected, json!({"data": job(), "received_at_ms": 99000}));
    let result = decode_job(&selected["data"], &JobId::try_from(JOB.to_owned()).unwrap()).unwrap();
    let decision = result.last_decision.unwrap();
    assert_eq!(decision.decided_at_ms, 100);
    assert_eq!(decision.observation.age_ms, None);
    assert_eq!(result.effective.reason, PolicyReason::ObservationMissing);
}

#[test]
fn registered_policy_job_rejects_bad_ids_and_missing_snake_case_argument_without_socket_io() {
    let socket = Socket::new();
    socket.listener.set_nonblocking(true).unwrap();
    let app = App::new(socket.bridge());
    for id in ["../private".to_owned(), "A".repeat(32), "a".repeat(33)] {
        assert_eq!(
            app.invoke("policy_job", json!({"job_id": id})),
            Err(json!({"code": "invalid_request", "retryable": false}))
        );
    }
    for payload in [json!({}), json!({"jobId": JOB}), json!({"job_id": [JOB]})] {
        assert!(app.invoke("policy_job", payload).is_err());
    }
    assert_eq!(
        socket.listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[test]
fn registered_policy_commands_preserve_explicit_old_daemon_unsupported_without_reading_a_policy() {
    for (command, payload) in [
        ("policy_current", json!({})),
        ("policy_job", json!({"job_id": JOB})),
    ] {
        let socket = Socket::new();
        let app = App::new(socket.bridge());
        let server = thread::spawn(move || {
            let (mut stream, request) = next_request(&socket.listener);
            assert_eq!(request.operation, OPERATION_POLICY_CAPABILITIES);
            write_response(
                &mut stream,
                &Response::error(request.request_id, arc_ipc::ERROR_UNSUPPORTED_CAPABILITY),
            )
            .unwrap();
            socket.listener.set_nonblocking(true).unwrap();
            assert_eq!(
                socket.listener.accept().unwrap_err().kind(),
                std::io::ErrorKind::WouldBlock
            );
        });
        let result = app.invoke(command, payload);
        server.join().unwrap();
        assert_eq!(
            result,
            Err(json!({"code": "unsupported", "retryable": false}))
        );
    }
}

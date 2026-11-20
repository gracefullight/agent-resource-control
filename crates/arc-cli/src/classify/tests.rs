use super::*;
use arc_detector::EndpointContext;
use serde_json::{json, Value};
use std::{
    fs,
    io::Read,
    os::unix::{fs::DirBuilderExt, io::AsRawFd, net::UnixListener},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::Duration,
};

const CAPTURED: u64 = 10_000;

fn shell(command: &str) -> ClassifyRequest {
    ClassifyRequest {
        command: command.into(),
        shell_kind: "sh".into(),
    }
}

fn context(facts: Value, environment: Value) -> EndpointContext {
    serde_json::from_value(json!({
        "captured_at_ms": CAPTURED,
        "facts": facts,
        "environment": environment,
    }))
    .unwrap()
}

fn gh() -> EndpointContext {
    context(
        json!([{ "kind":"CI_HOST", "selector":"gh", "endpoints":["https://github.com"] }]),
        json!({}),
    )
}

fn ssh() -> EndpointContext {
    context(
        json!([
            {"kind":"SSH_DESTINATION", "selector":"fixture", "endpoints":["ssh://192.0.2.30:22"]},
            {"kind":"SSH_DESTINATION", "selector":"@local-addresses", "endpoints":["192.0.2.10"]}
        ]),
        json!({}),
    )
}

fn builder(endpoint: &str) -> EndpointContext {
    context(
        json!([
            {"kind":"DOCKER_BUILDER", "selector":"fixture", "endpoints":[endpoint]},
            {"kind":"SSH_DESTINATION", "selector":"@local-addresses", "endpoints":["192.0.2.10"]}
        ]),
        json!({}),
    )
}

fn peer(
    input: &ClassifyRequest,
    context: Option<EndpointContext>,
    now: Option<u64>,
    reply: impl FnOnce(Request, &mut UnixStream) + Send + 'static,
) -> Result<Classification, &'static str> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let root = arc_core::runtime_root().unwrap().join(format!(
        "cl{:x}-{:x}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
    let socket = root.join("ipc");
    let listener = UnixListener::bind(&socket).unwrap();
    let server = thread::spawn(move || {
        let mut ready = libc::pollfd {
            fd: listener.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        assert_eq!(
            unsafe { libc::poll(&mut ready, 1, 60_000) },
            1,
            "peer accept watchdog"
        );
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(60)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(60)))
            .unwrap();
        let request: Request = arc_ipc::read_message(&mut stream).unwrap();
        assert_eq!(request.protocol_version, 1);
        reply(request, &mut stream);
        // The client closes after the result; an unsolicited extra request is
        // a failure, not a test retry.
        assert_eq!(stream.read(&mut [0u8; 1]).unwrap(), 0);
    });
    let result = classify(&socket, input, context, || now);
    server.join().unwrap();
    fs::remove_dir_all(root).unwrap();
    result
}

#[test]
fn locality_operation_uses_the_exact_shell_and_synthetic_facts() {
    for (command, captured, expected) in [
        ("gh workflow run build.yml", gh(), JobClass::RemoteCi),
        ("ssh fixture make", ssh(), JobClass::RemoteExec),
        (
            "git push origin main",
            context(
                json!([{ "kind":"GIT_REMOTE", "selector":"origin", "endpoints":["https://github.com/fixture/repository.git"] }]),
                json!({}),
            ),
            JobClass::RemoteCi,
        ),
        (
            "docker buildx build --builder fixture .",
            builder("tcp://192.0.2.30:2376"),
            JobClass::RemoteExec,
        ),
        (
            "docker buildx build --builder fixture .",
            context(
                json!([
                    {"kind":"DOCKER_BUILDER", "selector":"fixture", "endpoints":["unix:///fixture/docker.sock", "tcp://192.0.2.30:2376"]},
                    {"kind":"SSH_DESTINATION", "selector":"@local-addresses", "endpoints":["192.0.2.10"]}
                ]),
                json!({}),
            ),
            JobClass::Unknown,
        ),
        (
            "cargo build",
            context(json!([]), json!({})),
            JobClass::Build,
        ),
        (
            "kubectl exec fixture -- make",
            context(
                json!([{ "kind":"KUBERNETES_CONTEXT", "selector":"@current", "endpoints":["https://192.0.2.30:6443"] }]),
                json!({}),
            ),
            JobClass::Unknown,
        ),
        (
            "cargo test && gh workflow run build.yml",
            gh(),
            JobClass::Mixed,
        ),
        (
            "gh workflow run build.yml",
            context(json!([]), json!({})),
            JobClass::Unknown,
        ),
        (
            "gh workflow run build.yml",
            context(
                json!([{ "kind":"CI_HOST", "selector":"gh", "endpoints":["https://github.com"] }]),
                json!({"GH_HOST":"localhost"}),
            ),
            JobClass::Unknown,
        ),
    ] {
        let input = shell(command);
        let expected_command = command.to_string();
        let expected_context = serde_json::to_value(&captured).unwrap();
        let result = peer(
            &input,
            Some(captured),
            Some(CAPTURED),
            move |request, stream| {
                assert_eq!(request.operation, OPERATION_CLASSIFY);
                assert_eq!(
                    request.payload["command"],
                    json!({
                        "kind":"SHELL", "command":expected_command, "shell_kind":"sh"
                    })
                );
                assert_eq!(request.payload["context"], expected_context);
                let response = arc_ipc::locality::classify_response_at(&request, CAPTURED);
                arc_ipc::write_response(stream, &response).unwrap();
            },
        )
        .unwrap();
        assert_eq!(result.classification, expected, "{command}");
        if expected == JobClass::Unknown {
            assert_eq!(
                (result.locality, result.uncertainty),
                (Locality::Unknown, Uncertainty::Observe)
            );
        }
    }
}

#[test]
fn unknown_peer_result_for_unproved_unix_socket_is_preserved() {
    let input = shell("docker buildx build --builder fixture .");
    let captured = builder("unix:///fixture/docker.sock");
    let expected_context = serde_json::to_value(&captured).unwrap();
    let result = peer(
        &input,
        Some(captured),
        Some(CAPTURED),
        move |request, stream| {
            assert_eq!(request.operation, OPERATION_CLASSIFY);
            assert_eq!(request.payload["context"], expected_context);
            arc_ipc::write_response(
                stream,
                &arc_ipc::Response::ok(
                    request.request_id,
                    json!({
                        "classification":"UNKNOWN", "locality":"UNKNOWN",
                        "reason":"ENDPOINT_AMBIGUOUS", "uncertainty":"OBSERVE"
                    }),
                ),
            )
            .unwrap();
        },
    )
    .unwrap();
    assert_eq!(
        (
            result.classification,
            result.locality,
            result.reason,
            result.uncertainty
        ),
        (
            JobClass::Unknown,
            Locality::Unknown,
            Reason::EndpointAmbiguous,
            Uncertainty::Observe
        )
    );
}

#[test]
fn missing_or_expired_context_never_displays_a_confident_result() {
    for (captured, now) in [
        (None, Some(CAPTURED)),
        (Some(gh()), Some(CAPTURED + 5_001)),
        (Some(gh()), Some(CAPTURED - 1)),
        (Some(gh()), None),
    ] {
        let result = peer(
            &shell("gh workflow run build.yml"),
            captured,
            now,
            |request, stream| {
                assert_eq!(request.operation, OPERATION_CLASSIFY);
                // A peer reply made when facts were fresh is still checked when it
                // reaches the caller. Missing capture also rejects this assertion.
                arc_ipc::write_response(
                    stream,
                    &arc_ipc::Response::ok(
                        request.request_id,
                        json!({
                            "classification":"REMOTE_CI", "locality":"REMOTE",
                            "reason":"GH_WORKFLOW_RUN", "uncertainty":"NONE"
                        }),
                    ),
                )
                .unwrap();
            },
        )
        .unwrap();
        assert_eq!(
            (result.classification, result.locality, result.uncertainty),
            (JobClass::Unknown, Locality::Unknown, Uncertainty::Observe)
        );
    }
}

#[test]
fn legacy_fallback_checks_the_same_fresh_context_before_accepting_a_result() {
    let mut conflicting = gh();
    conflicting
        .environment
        .insert("GH_HOST".into(), "localhost".into());
    let empty = context(json!([]), json!({}));
    for (command, captured, now, expected) in [
        ("cargo test", Some(empty), Some(CAPTURED), JobClass::Test),
        (
            "gh workflow run build.yml",
            Some(gh()),
            Some(CAPTURED),
            JobClass::RemoteCi,
        ),
        (
            "ssh fixture make",
            Some(ssh()),
            Some(CAPTURED),
            JobClass::RemoteExec,
        ),
        (
            "cargo test && gh workflow run build.yml",
            Some(gh()),
            Some(CAPTURED),
            JobClass::Mixed,
        ),
        (
            "docker buildx build --builder fixture .",
            Some(builder("tcp://192.0.2.30:2376")),
            Some(CAPTURED),
            JobClass::Unknown,
        ),
        (
            "docker buildx build --builder fixture .",
            Some(builder("unix:///fixture/docker.sock")),
            Some(CAPTURED),
            JobClass::Unknown,
        ),
        (
            "gh workflow run build.yml",
            None,
            Some(CAPTURED),
            JobClass::Unknown,
        ),
        ("ssh fixture make", None, Some(CAPTURED), JobClass::Unknown),
        (
            "cargo test && gh workflow run build.yml",
            None,
            Some(CAPTURED),
            JobClass::Unknown,
        ),
        (
            "gh workflow run build.yml",
            Some(conflicting),
            Some(CAPTURED),
            JobClass::Unknown,
        ),
        (
            "gh workflow run build.yml",
            Some(gh()),
            Some(CAPTURED + 5_001),
            JobClass::Unknown,
        ),
    ] {
        let input = shell(command);
        let expected_command = command.to_string();
        let result = peer(&input, captured, now, move |request, stream| {
            assert_eq!(request.operation, OPERATION_CLASSIFY);
            arc_ipc::write_response(
                stream,
                &arc_ipc::Response::error(
                    request.request_id,
                    arc_ipc::ERROR_UNSUPPORTED_CAPABILITY,
                ),
            )
            .unwrap();
            let legacy: Request = arc_ipc::read_message(stream).unwrap();
            assert_eq!(legacy.protocol_version, 1);
            assert_eq!(legacy.operation, OPERATION_JOB_CLASSIFY);
            assert_eq!(
                legacy.payload,
                json!({"command":expected_command,"shell_kind":"sh"})
            );
            let response = arc_ipc::dispatch(&legacy, || unreachable!());
            arc_ipc::write_response(stream, &response).unwrap();
        })
        .unwrap();
        assert_eq!(result.classification, expected, "{command}");
        if expected == JobClass::Unknown {
            assert_eq!(
                (result.locality, result.uncertainty),
                (Locality::Unknown, Uncertainty::Observe)
            );
        }
    }
}

#[test]
fn unsupported_shell_and_oversized_input_keep_legacy_unknown_reasons() {
    let mut unsupported = shell("cargo test");
    unsupported.shell_kind = "powershell".into();
    let long = shell(&"x".repeat(arc_detector::MAX_INPUT_BYTES + 1));
    for (input, reason) in [
        (unsupported, Reason::UnsupportedShell),
        (long, Reason::InputLimit),
    ] {
        let result = peer(&input, None, Some(CAPTURED), |request, stream| {
            assert_eq!(request.operation, OPERATION_JOB_CLASSIFY);
            let response = arc_ipc::dispatch(&request, || unreachable!());
            arc_ipc::write_response(stream, &response).unwrap();
        })
        .unwrap();
        assert_eq!(
            (result.classification, result.reason),
            (JobClass::Unknown, reason)
        );
    }
}

mod repair;

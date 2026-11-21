//! The negotiated classification operation: which daemon generation is asked
//! what for a local container build, and what each answer displays.
use super::*;

const SOCKET: &str = "unix:///fixture/docker.sock";
const COMMANDS: [(&str, &str); 2] = [
    ("docker buildx build .", "@selected/@current"),
    ("docker build .", "@current"),
];

/// Facts as capture emits them for a context and builder on one proven socket.
fn local_engine(builder: &str) -> EndpointContext {
    context(
        json!([
            {"kind":"DOCKER_CONTEXT", "selector":"@current", "endpoints":[SOCKET]},
            {"kind":"DOCKER_BUILDER", "selector":builder, "endpoints":[SOCKET]},
            {"kind":"DOCKER_CONTEXT", "selector":"@local-proof/0", "endpoints":[SOCKET]}
        ]),
        json!({}),
    )
}

fn fields(result: Classification) -> (JobClass, Locality, Reason, Uncertainty) {
    (
        result.classification,
        result.locality,
        result.reason,
        result.uncertainty,
    )
}

fn build(request: &Request, locality: &str) -> arc_ipc::Response {
    arc_ipc::Response::ok(
        &request.request_id,
        json!({
            "classification":"BUILD", "locality":locality,
            "reason":"LOCAL_ENDPOINT_BUILD", "uncertainty":"NONE"
        }),
    )
}

/// Answer the locality operation with the projection, then read the one
/// negotiated request that must follow with the same payload.
fn projected_then_negotiated(request: Request, stream: &mut UnixStream) -> Request {
    assert_eq!(request.operation, OPERATION_CLASSIFY);
    arc_ipc::write_response(stream, &build(&request, "HOST_LOCAL")).unwrap();
    let negotiated: Request = arc_ipc::read_message(stream).unwrap();
    assert_eq!(negotiated.protocol_version, 1);
    assert_eq!(negotiated.operation, compute_location::OPERATION_CLASSIFY);
    assert_eq!(negotiated.payload, request.payload);
    negotiated
}

#[test]
fn negotiated_daemon_names_a_local_container_for_both_build_commands() {
    for (command, builder) in COMMANDS {
        let captured = local_engine(builder);
        let expected_context = serde_json::to_value(&captured).unwrap();
        let result = peer(
            &shell(command),
            Some(captured),
            Some(CAPTURED),
            move |request, stream| {
                assert_eq!(request.operation, OPERATION_CLASSIFY);
                assert_eq!(
                    request.payload["command"],
                    json!({"kind":"SHELL", "command":command, "shell_kind":"sh"})
                );
                assert_eq!(request.payload["context"], expected_context);
                // Both replies are the real operations' own answers.
                let earlier = arc_ipc::locality::classify_response_at(&request, CAPTURED);
                assert_eq!(earlier.result.as_ref().unwrap()["locality"], "HOST_LOCAL");
                arc_ipc::write_response(stream, &earlier).unwrap();
                let negotiated: Request = arc_ipc::read_message(stream).unwrap();
                assert_eq!(negotiated.operation, compute_location::OPERATION_CLASSIFY);
                assert_eq!(negotiated.payload, request.payload);
                let response = arc_ipc::locality::classify_response_at(&negotiated, CAPTURED);
                arc_ipc::write_response(stream, &response).unwrap();
            },
        )
        .unwrap();
        assert_eq!(
            fields(result),
            (
                JobClass::Build,
                Locality::LocalContainer,
                Reason::LocalEndpointBuild,
                Uncertainty::None
            ),
            "{command}"
        );
    }
}

#[test]
fn earlier_locality_daemon_keeps_the_host_local_projection() {
    // The second case is a peer that names the location on an operation that
    // did not negotiate it: it is read as the projection all the same.
    for locality in ["HOST_LOCAL", "LOCAL_CONTAINER"] {
        for (command, builder) in COMMANDS {
            let result = peer(
                &shell(command),
                Some(local_engine(builder)),
                Some(CAPTURED),
                move |request, stream| {
                    assert_eq!(request.operation, OPERATION_CLASSIFY);
                    arc_ipc::write_response(stream, &build(&request, locality)).unwrap();
                    let negotiated: Request = arc_ipc::read_message(stream).unwrap();
                    assert_eq!(negotiated.operation, compute_location::OPERATION_CLASSIFY);
                    arc_ipc::write_response(
                        stream,
                        &arc_ipc::Response::error(
                            &negotiated.request_id,
                            arc_ipc::ERROR_UNSUPPORTED_CAPABILITY,
                        ),
                    )
                    .unwrap();
                },
            )
            .unwrap();
            assert!(result.is_consistent(), "{command}");
            assert_eq!(
                fields(result),
                (
                    JobClass::Build,
                    Locality::HostLocal,
                    Reason::LocalEndpointBuild,
                    Uncertainty::None
                ),
                "{command} {locality}"
            );
        }
    }
}

#[test]
fn protocol_1_daemon_is_asked_and_answers_in_protocol_1_vocabulary() {
    for (command, builder) in COMMANDS {
        let result = peer(
            &shell(command),
            Some(local_engine(builder)),
            Some(CAPTURED),
            move |request, stream| {
                assert_eq!(request.operation, OPERATION_CLASSIFY);
                arc_ipc::write_response(
                    stream,
                    &arc_ipc::Response::error(
                        &request.request_id,
                        arc_ipc::ERROR_UNSUPPORTED_CAPABILITY,
                    ),
                )
                .unwrap();
                // The negotiated operation is never offered to this peer.
                let legacy: Request = arc_ipc::read_message(stream).unwrap();
                assert_eq!(legacy.operation, OPERATION_JOB_CLASSIFY);
                assert_eq!(
                    legacy.payload,
                    json!({"command":command, "shell_kind":"sh"})
                );
                let response = arc_ipc::dispatch(&legacy, || unreachable!());
                arc_ipc::write_response(stream, &response).unwrap();
            },
        )
        .unwrap();
        assert_eq!(result, result.legacy(), "{command}");
        assert_eq!(
            (result.classification, result.locality, result.uncertainty),
            (JobClass::Unknown, Locality::Unknown, Uncertainty::Observe),
            "{command}"
        );
    }
}

#[test]
fn a_negotiated_reply_is_displayed_only_when_it_names_the_same_proven_record() {
    let unknown = (JobClass::Unknown, Locality::Unknown, Uncertainty::Observe);
    // The same socket without a local proof: neither claim is shown.
    let unproved = context(
        json!([
            {"kind":"DOCKER_CONTEXT", "selector":"@current", "endpoints":[SOCKET]},
            {"kind":"DOCKER_BUILDER", "selector":"@current", "endpoints":[SOCKET]}
        ]),
        json!({}),
    );
    let result = peer(
        &shell("docker build ."),
        Some(unproved),
        Some(CAPTURED),
        |request, stream| {
            let negotiated = projected_then_negotiated(request, stream);
            arc_ipc::write_response(stream, &build(&negotiated, "LOCAL_CONTAINER")).unwrap();
        },
    )
    .unwrap();
    assert_eq!(
        (result.classification, result.locality, result.uncertainty),
        unknown
    );

    for (reply, expected_reason) in [
        // Facts aged between the two requests: the daemon's own UNKNOWN.
        (
            json!({"classification":"UNKNOWN", "locality":"UNKNOWN",
                "reason":"ENDPOINT_AMBIGUOUS", "uncertainty":"OBSERVE"}),
            Reason::EndpointAmbiguous,
        ),
        // A different confident record for the same request.
        (
            json!({"classification":"REMOTE_EXEC", "locality":"REMOTE",
                "reason":"REMOTE_ENDPOINT_EXEC", "uncertainty":"NONE"}),
            Reason::UnresolvedCommand,
        ),
    ] {
        let result = peer(
            &shell("docker build ."),
            Some(local_engine("@current")),
            Some(CAPTURED),
            move |request, stream| {
                let negotiated = projected_then_negotiated(request, stream);
                arc_ipc::write_response(
                    stream,
                    &arc_ipc::Response::ok(&negotiated.request_id, reply),
                )
                .unwrap();
            },
        )
        .unwrap();
        assert_eq!(
            (result.classification, result.locality, result.uncertainty),
            unknown
        );
        assert_eq!(result.reason, expected_reason);
    }

    // A failed or malformed negotiated exchange is an error, not a fallback.
    for malformed in [false, true] {
        let result = peer(
            &shell("docker build ."),
            Some(local_engine("@current")),
            Some(CAPTURED),
            move |request, stream| {
                let negotiated = projected_then_negotiated(request, stream);
                let response = if malformed {
                    // A container location on a record that cannot have one.
                    arc_ipc::Response::ok(
                        &negotiated.request_id,
                        json!({"classification":"BUILD", "locality":"LOCAL_CONTAINER",
                            "reason":"CARGO_BUILD", "uncertainty":"NONE"}),
                    )
                } else {
                    arc_ipc::Response::error(&negotiated.request_id, arc_ipc::ERROR_INVALID_REQUEST)
                };
                arc_ipc::write_response(stream, &response).unwrap();
            },
        );
        assert_eq!(
            result,
            Err(if malformed {
                crate::IPC_ERROR
            } else {
                arc_ipc::ERROR_INVALID_REQUEST
            })
        );
    }
}

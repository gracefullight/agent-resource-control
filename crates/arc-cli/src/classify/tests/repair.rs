use super::*;

#[test]
fn nested_bash_rejects_legacy_peer_results_when_startup_can_change_endpoints() {
    for command in [
        "bash -c 'gh workflow run build.yml'",
        "bash -lc 'cargo test'",
    ] {
        for captured in [Some(gh()), None] {
            let result = peer(
                &shell(command),
                captured,
                Some(CAPTURED),
                |request, stream| {
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
                    assert_eq!(legacy.operation, OPERATION_JOB_CLASSIFY);
                    let response = arc_ipc::dispatch(&legacy, || unreachable!());
                    arc_ipc::write_response(stream, &response).unwrap();
                },
            )
            .unwrap();
            assert_eq!(
                (result.classification, result.locality, result.uncertainty),
                (JobClass::Unknown, Locality::Unknown, Uncertainty::Observe),
                "{command}"
            );
        }
    }
}

#[test]
fn locality_peer_disagreement_never_turns_remote_work_into_a_local_result() {
    // Task-1's synthetic gh fact is sufficient to establish REMOTE_CI. A
    // controlled peer can disagree without relying on another lane's parser.
    let captured = gh();
    let input = shell("gh workflow run build.yml");
    let current = peer(
        &input,
        Some(captured.clone()),
        Some(CAPTURED),
        |request, stream| {
            assert_eq!(request.operation, OPERATION_CLASSIFY);
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
        (current.classification, current.locality),
        (JobClass::RemoteCi, Locality::Remote)
    );

    let old = peer(&input, Some(captured), Some(CAPTURED), |request, stream| {
        assert_eq!(request.operation, OPERATION_CLASSIFY);
        arc_ipc::write_response(
            stream,
            &arc_ipc::Response::ok(
                request.request_id,
                json!({
                    "classification":"BUILD", "locality":"HOST_LOCAL",
                    "reason":"CARGO_BUILD", "uncertainty":"NONE"
                }),
            ),
        )
        .unwrap();
    })
    .unwrap();
    assert_eq!(
        (old.classification, old.locality, old.uncertainty),
        (JobClass::Unknown, Locality::Unknown, Uncertainty::Observe)
    );
}

#[test]
fn locality_v1_schema_mismatch_is_probed_without_hiding_other_invalid_requests() {
    let input = shell("git push");
    let captured = context(
        json!([{ "kind":"GIT_PUSH_DEFAULT", "selector":"@bare-push", "endpoints":["https://github.com/fixture/repository.git"] }]),
        json!({}),
    );
    for probe_reply in 0..3 {
        let result = peer(
            &input,
            Some(captured.clone()),
            Some(CAPTURED),
            move |request, stream| {
                assert_eq!(request.operation, OPERATION_CLASSIFY);
                assert_eq!(
                    request.payload["context"]["facts"][0]["kind"],
                    "GIT_PUSH_DEFAULT"
                );
                arc_ipc::write_response(
                    stream,
                    &arc_ipc::Response::error(&request.request_id, arc_ipc::ERROR_INVALID_REQUEST),
                )
                .unwrap();
                let stripped: Request = arc_ipc::read_message(stream).unwrap();
                assert_eq!(stripped.protocol_version, 1);
                assert_eq!(stripped.operation, OPERATION_CLASSIFY);
                let mut expected = request.payload;
                expected["context"]["facts"] = json!([]);
                assert_eq!(
                    stripped.payload, expected,
                    "probe changed more than the unsupported fact"
                );
                let response = match probe_reply {
                    0 => arc_ipc::Response::ok(
                        &stripped.request_id,
                        json!({
                            "classification":"UNKNOWN", "locality":"UNKNOWN",
                            "reason":"UNRESOLVED_COMMAND", "uncertainty":"OBSERVE"
                        }),
                    ),
                    1 => arc_ipc::Response::error(
                        &stripped.request_id,
                        arc_ipc::ERROR_INVALID_REQUEST,
                    ),
                    2 => arc_ipc::Response::ok(
                        &stripped.request_id,
                        json!({
                            "classification":"BUILD", "locality":"HOST_LOCAL",
                            "reason":"CARGO_BUILD", "uncertainty":"NONE"
                        }),
                    ),
                    _ => unreachable!(),
                };
                arc_ipc::write_response(stream, &response).unwrap();
            },
        );
        if probe_reply != 1 {
            let result = result.unwrap();
            assert_eq!(
                (result.classification, result.locality, result.uncertainty),
                (JobClass::Unknown, Locality::Unknown, Uncertainty::Observe)
            );
        } else {
            assert_eq!(result, Err(arc_ipc::ERROR_INVALID_REQUEST));
        }
    }
}

#[test]
fn pnpm_fingerprint_stays_local_and_old_peer_fails_closed() {
    let input = shell("pnpm run deploy");
    let digest = "a".repeat(64);
    let captured = context(
        json!([{ "kind":"PNPM_SCRIPT", "selector":"deploy", "endpoints":["cargo-build-gh-workflow-ci-v1", digest] }]),
        json!({}),
    );
    let result = peer(&input, Some(captured), Some(CAPTURED), |request, stream| {
        assert_eq!(
            request.payload["context"]["facts"][0]["endpoints"],
            json!(["cargo-build-gh-workflow-ci-v1"])
        );
        arc_ipc::write_response(
            stream,
            &arc_ipc::Response::error(&request.request_id, arc_ipc::ERROR_INVALID_REQUEST),
        )
        .unwrap();
        let stripped: Request = arc_ipc::read_message(stream).unwrap();
        assert_eq!(stripped.payload["context"]["facts"], json!([]));
        arc_ipc::write_response(
            stream,
            &arc_ipc::Response::ok(
                &stripped.request_id,
                json!({"classification":"UNKNOWN", "locality":"UNKNOWN",
                "reason":"UNRESOLVED_SCRIPT", "uncertainty":"OBSERVE"}),
            ),
        )
        .unwrap();
    })
    .unwrap();
    assert_eq!(
        (result.classification, result.locality, result.uncertainty),
        (JobClass::Unknown, Locality::Unknown, Uncertainty::Observe)
    );
}

#[test]
fn old_locality_peers_fail_closed_for_make_shell_and_act_proofs() {
    for (kind, selector, value) in [
        ("MAKEFILE", "test", "test:\n\tcargo test\n"),
        ("SHELL_STARTUP", "zsh", "safe-nonlogin-v1"),
        ("ACT_LOCAL", "act", "unix:///fixture/docker.sock"),
    ] {
        let captured = context(
            json!([{ "kind":kind, "selector":selector, "endpoints":[value] }]),
            json!({}),
        );
        let result = peer(
            &shell("make test"),
            Some(captured),
            Some(CAPTURED),
            |request, stream| {
                arc_ipc::write_response(
                    stream,
                    &arc_ipc::Response::error(&request.request_id, arc_ipc::ERROR_INVALID_REQUEST),
                )
                .unwrap();
                let stripped: Request = arc_ipc::read_message(stream).unwrap();
                assert_eq!(stripped.payload["context"]["facts"], json!([]));
                arc_ipc::write_response(
                    stream,
                    &arc_ipc::Response::ok(
                        &stripped.request_id,
                        json!({"classification":"UNKNOWN", "locality":"UNKNOWN",
                    "reason":"UNRESOLVED_SCRIPT", "uncertainty":"OBSERVE"}),
                    ),
                )
                .unwrap();
            },
        )
        .unwrap();
        assert_eq!(
            (result.classification, result.locality, result.uncertainty),
            (JobClass::Unknown, Locality::Unknown, Uncertainty::Observe),
            "{kind}"
        );
    }
}

#[test]
fn context_expiring_during_revalidation_cannot_certify_the_peer_result() {
    use std::cell::Cell;
    let context = gh();
    let input = shell("gh workflow run build.yml");
    let remote = arc_detector::classify_with_context(&input, &context, CAPTURED);
    let calls = Cell::new(0);
    let clock = || {
        let count = calls.get();
        calls.set(count + 1);
        Some(if count == 0 {
            CAPTURED
        } else {
            CAPTURED + arc_detector::MAX_CONTEXT_AGE_MS + 1
        })
    };
    let result = verified_result(&input, Some(&context), &clock, remote);
    assert_eq!(
        (result.classification, result.locality, result.uncertainty),
        (JobClass::Unknown, Locality::Unknown, Uncertainty::Observe)
    );
    assert_eq!(calls.get(), 2);
}

#[test]
fn malformed_invalid_response_does_not_trigger_schema_probe() {
    let input = shell("git push");
    let captured = context(
        json!([{ "kind":"GIT_PUSH_DEFAULT", "selector":"@bare-push", "endpoints":["https://github.com/fixture/repository.git"] }]),
        json!({}),
    );
    for bad_correlation in [false, true] {
        let result = peer(
            &input,
            Some(captured.clone()),
            Some(CAPTURED),
            move |request, stream| {
                let mut response =
                    arc_ipc::Response::error(&request.request_id, arc_ipc::ERROR_INVALID_REQUEST);
                if bad_correlation {
                    response.request_id = "wrong-correlation".into();
                } else {
                    response.result = Some(json!({"classification":"UNKNOWN"}));
                }
                arc_ipc::write_response(stream, &response).unwrap();
                assert_eq!(
                    stream.read(&mut [0u8; 1]).unwrap(),
                    0,
                    "malformed error triggered a probe"
                );
            },
        );
        assert_eq!(result, Err(crate::IPC_ERROR));
    }
}

#[test]
fn failed_capture_preserves_definitive_parser_diagnostics() {
    let mut cases = vec![
        (
            format!("cargo test{}", " a".repeat(1023)),
            Reason::TokenLimit,
        ),
        (vec!["cargo test"; 129].join(";"), Reason::InvocationLimit),
        (
            format!("sh -c 'cargo test {}'", "x".repeat(12000)),
            Reason::WorkLimit,
        ),
        (
            format!("{}cargo test{}", "(".repeat(9), ")".repeat(9)),
            Reason::NestingLimit,
        ),
        ("cargo test 'unterminated".into(), Reason::UnsupportedSyntax),
        (
            "cargo test $(never-run)".into(),
            Reason::UnresolvedExpansion,
        ),
        ("".into(), Reason::EmptyCommand),
        ("./verify.sh".into(), Reason::UnresolvedScript),
        ("npm run verify".into(), Reason::UnresolvedScript),
        (
            "cargo test && gh run view && ./deploy.sh".into(),
            Reason::UnresolvedScript,
        ),
        (
            "printf '%s' 'cargo build && gh run watch'".into(),
            Reason::UnresolvedCommand,
        ),
        ("--json".into(), Reason::UnresolvedCommand),
    ];
    for (command, reason) in cases.drain(..) {
        let input = shell(&command);
        assert_eq!(arc_detector::classify(&input).reason, reason);
        let result = peer(&input, None, Some(CAPTURED), |request, stream| {
            assert_eq!(request.operation, OPERATION_CLASSIFY);
            let response = arc_ipc::locality::classify_response_at(&request, CAPTURED);
            arc_ipc::write_response(stream, &response).unwrap();
        })
        .unwrap();
        assert_eq!(
            (result.classification, result.reason, result.uncertainty),
            (JobClass::Unknown, reason, Uncertainty::Observe)
        );
    }
}

#[test]
fn failed_capture_never_promotes_an_unresolved_script_or_endpoint() {
    for (command, reason) in [
        ("./verify.sh", Reason::UnresolvedScript),
        ("gh workflow run build.yml", Reason::UnresolvedCommand),
    ] {
        let result = peer(&shell(command), None, Some(CAPTURED), |request, stream| {
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
        })
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
                reason,
                Uncertainty::Observe
            ),
            "{command}"
        );
    }
}

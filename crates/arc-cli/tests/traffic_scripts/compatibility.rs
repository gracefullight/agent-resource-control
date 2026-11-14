use super::execution::{command, setup, unsupported};
use crate::support::*;
use arc_ipc::{
    governor::*,
    traffic::{self, JobRequest},
};
use serde_json::json;
use std::{fs, thread};

#[test]
fn unsupported_before_submission_projects_observation_without_changing_original_invocation() {
    for unsupported_at_enqueue in [false, true] {
        let socket = Socket::new();
        setup(&socket, "vite build");
        let listener = socket.listener.try_clone().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, request) = next(&listener);
            assert_eq!(
                request.operation,
                traffic::OPERATION_BUN_SCRIPTS_CAPABILITIES
            );
            let mut original_id = None;
            if unsupported_at_enqueue {
                reply(
                    &mut stream,
                    &request,
                    json!(traffic::Capabilities::scripts_supported()),
                );
                let (mut stream, request) = next(&listener);
                let GovernorJobRequest::Traffic {
                    request: JobRequest::Enqueue { job_id, .. },
                } = traffic::scripts::decode_job(&request.operation, &request.payload).unwrap()
                else {
                    panic!("script enqueue")
                };
                original_id = Some(job_id);
                unsupported(&mut stream, &request);
            } else {
                unsupported(&mut stream, &request);
            }
            let mut identity = None;
            // Deliberately lose legacy Register ACKs. The unchanged bounded
            // transport retries metadata using the same identity, then runs.
            for attempt in 0..3 {
                let (stream, request) = next(&listener);
                assert_eq!(
                    request.operation,
                    arc_ipc::jobs::OPERATION_JOB_START,
                    "unsupported_at_enqueue={unsupported_at_enqueue} attempt={attempt}"
                );
                let arc_ipc::jobs::StartRequest::Register {
                    job_id,
                    observer,
                    classification,
                } = serde_json::from_value(request.payload).unwrap()
                else {
                    panic!("legacy registration")
                };
                assert_eq!(classification.classification, arc_ipc::JobClass::Unknown);
                assert_eq!(classification.reason, arc_ipc::Reason::UnresolvedCommand);
                if let Some(id) = &original_id {
                    assert_eq!(&job_id, id);
                }
                if let Some(prior) = &identity {
                    assert_eq!(prior, &(job_id.clone(), observer.clone()));
                }
                identity = Some((job_id, observer));
                drop(stream);
            }
            // Native accounting is independently asserted by the synchronized
            // governed cases. This deliberately unavailable legacy peer makes
            // no promise of a terminal report for a fast, untracked child.
        });
        let result = output(command(&socket));
        server.join().unwrap_or_else(|_| {
            panic!(
                "unsupported_at_enqueue={unsupported_at_enqueue} status={} stderr={}",
                result.status,
                String::from_utf8_lossy(&result.stderr)
            )
        });
        assert_eq!(
            result.status.code(),
            Some(17),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            fs::read_to_string(socket.root.join("starts"))
                .unwrap()
                .lines()
                .count(),
            1
        );
        assert!(String::from_utf8_lossy(&result.stderr).contains("TRACKING_UNCERTAIN"));
    }
}

#[test]
fn lost_enqueue_ack_or_later_unsupported_never_erases_job_identity_witness_or_uncertainty() {
    for queued_first in [false, true] {
        let socket = Socket::new();
        setup(&socket, "tsc --noEmit");
        let listener = socket.listener.try_clone().unwrap();
        let control =
            std::os::unix::net::UnixListener::bind(socket.root.join("target-control")).unwrap();
        let root = socket.root.clone();
        let server = thread::spawn(move || {
            let (mut stream, request) = next(&listener);
            reply(
                &mut stream,
                &request,
                json!(traffic::Capabilities::scripts_supported()),
            );
            let (mut stream, request) = next(&listener);
            let GovernorJobRequest::Traffic {
                request:
                    JobRequest::Enqueue {
                        job_id, observer, ..
                    },
            } = traffic::scripts::decode_job(&request.operation, &request.payload).unwrap()
            else {
                panic!("enqueue")
            };
            if queued_first {
                reply(
                    &mut stream,
                    &request,
                    json!({"state":"TRAFFIC","result":{"state":"QUEUED","sequence":1,"queued_at_ms":100,"reason":"CLASS_WINDOW"}}),
                );
                let (mut stream, request) = next(&listener);
                assert_eq!(request.payload["request"]["action"], "POLL");
                unsupported(&mut stream, &request);
            }
            let initial = request.clone();
            drop(stream);
            if !queued_first {
                let (mut stream, retry) = next(&listener);
                assert_eq!(
                    retry, initial,
                    "Reconcile the possibly committed original enqueue before fail-open"
                );
                unsupported(&mut stream, &retry);
            }
            for completed in [false, true] {
                let (mut stream, request) = next(&listener);
                assert_eq!(
                    request.operation,
                    traffic::OPERATION_BUN_SCRIPTS_JOB,
                    "No legacy registration after possible commit"
                );
                let GovernorJobRequest::Traffic {
                    request:
                        JobRequest::Fallback {
                            job_id: id,
                            observer: owner,
                            outcome,
                            ..
                        },
                } = traffic::scripts::decode_job(&request.operation, &request.payload).unwrap()
                else {
                    panic!("same-attempt fallback: {}", request.payload)
                };
                assert_eq!(id, job_id);
                assert_eq!(owner, observer);
                assert_eq!(outcome.is_some(), completed);
                unsupported(&mut stream, &request);
                if !completed {
                    use std::io::Write;
                    let mut target = accept(&control);
                    arc_ipc::read_frame(&mut target).unwrap();
                    target.write_all(b"x").unwrap();
                }
            }
            // Unconfirmed registration cannot claim a persisted Meter binding.
            assert!(root
                .join(format!(".traffic-witness-{}", job_id.as_str()))
                .is_file());
            job_id
        });
        let mut command = command(&socket);
        command.env("ARC_FIXTURE_CONTROL", socket.root.join("target-control"));
        let result = output(command);
        let id = server.join().unwrap();
        assert_eq!(
            result.status.code(),
            Some(17),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            fs::read_to_string(socket.root.join("starts"))
                .unwrap()
                .lines()
                .count(),
            1
        );
        assert!(String::from_utf8_lossy(&result.stderr).contains("TRACKING_UNCERTAIN"));
        assert!(socket
            .root
            .join(format!(".traffic-witness-{}", id.as_str()))
            .exists());
    }
}

#[test]
fn queued_cancellation_cleans_up_the_same_attempt_without_native_execution() {
    let socket = Socket::new();
    setup(&socket, "bun test");
    let listener = socket.listener.try_clone().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, request) = next(&listener);
        reply(
            &mut stream,
            &request,
            json!(traffic::Capabilities::scripts_supported()),
        );
        let (mut stream, request) = next(&listener);
        let GovernorJobRequest::Traffic {
            request: JobRequest::Enqueue {
                job_id, observer, ..
            },
        } = traffic::scripts::decode_job(&request.operation, &request.payload).unwrap()
        else {
            panic!("enqueue")
        };
        reply(
            &mut stream,
            &request,
            json!({"state":"TRAFFIC","result":{"state":"QUEUED","sequence":1,"queued_at_ms":100,"reason":"CLASS_WINDOW"}}),
        );
        assert_eq!(unsafe { libc::kill(observer.pid as i32, libc::SIGTERM) }, 0);
        let (mut stream, request) = next(&listener);
        let GovernorJobRequest::Traffic {
            request:
                JobRequest::Cancel {
                    job_id: id,
                    observer: owner,
                    ..
                },
        } = traffic::scripts::decode_job(&request.operation, &request.payload).unwrap()
        else {
            panic!("cancel")
        };
        assert_eq!(id, job_id);
        assert_eq!(owner, observer);
        reply(
            &mut stream,
            &request,
            json!({"state":"TRAFFIC","result":{"state":"CANCELLED"}}),
        );
    });
    let result = output(command(&socket));
    server.join().unwrap();
    assert!(!result.status.success());
    assert!(!socket.root.join("starts").exists());
    assert!(String::from_utf8_lossy(&result.stderr).contains("NOT_STARTED"));
}

#[test]
fn unavailable_coordinator_is_bounded_and_original_opaque_arguments_still_run_once() {
    let socket = Socket::new();
    setup(&socket, "vite build");
    fs::remove_file(socket.root.join("s")).unwrap();
    let mut command = command(&socket);
    command.args(["", "two words", "--", "$(touch poison)"]);
    let start = std::time::Instant::now();
    let result = output(command);
    assert!(start.elapsed() < std::time::Duration::from_secs(10));
    assert_eq!(result.status.code(), Some(17));
    let data: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(
        data["args"],
        json!(
            ["run", "surprise", "", "two words", "--", "$(touch poison)"]
                .map(|s| s.as_bytes().to_vec())
        )
    );
    assert_eq!(
        fs::read_to_string(socket.root.join("starts"))
            .unwrap()
            .lines()
            .count(),
        1
    );
    assert!(!socket.root.join("poison").exists());
}

#[test]
fn invalid_capability_reply_stops_before_spawn_and_does_not_approve_scripts_from_governor_support()
{
    let socket = Socket::new();
    setup(&socket, "vite build");
    let listener = socket.listener.try_clone().unwrap();
    let server = thread::spawn(move || {
        let (mut stream, request) = next(&listener);
        assert_eq!(
            request.operation,
            traffic::OPERATION_BUN_SCRIPTS_CAPABILITIES
        );
        reply(
            &mut stream,
            &request,
            json!(GovernorCapabilities::supported()),
        );
    });
    let result = output(command(&socket));
    server.join().unwrap();
    assert!(!result.status.success());
    assert!(!socket.root.join("starts").exists());
    assert!(String::from_utf8_lossy(&result.stderr).contains("IPC_ERROR"));
}

#[test]
fn remote_only_and_unsupported_graphs_execute_as_observations_without_local_admission() {
    for (body, locality) in [
        ("gh workflow run ci.yml", "REMOTE"),
        ("ssh remote.example 'bun run build'", "REMOTE"),
        ("vite build --watch", "UNKNOWN"),
        ("vite build && biome check .", "UNKNOWN"),
        ("bun run surprise", "UNKNOWN"),
    ] {
        let socket = Socket::new();
        setup(&socket, body);
        let listener = socket.listener.try_clone().unwrap();
        let server = thread::spawn(move || {
            for _ in 0..3 {
                let (stream, request) = next(&listener);
                assert_eq!(
                    request.operation,
                    arc_ipc::jobs::OPERATION_JOB_START,
                    "Bypass graphs must never enqueue or negotiate a reservation"
                );
                assert_eq!(request.payload["classification"]["locality"], locality);
                assert_ne!(request.payload["classification"]["classification"], "BUILD");
                drop(stream);
            }
        });
        let result = output(command(&socket));
        server.join().unwrap();
        assert_eq!(result.status.code(), Some(17));
        assert_eq!(
            fs::read_to_string(socket.root.join("starts"))
                .unwrap()
                .lines()
                .count(),
            1
        );
    }
}

fn assert_typecheck_observation(body: &str, forwarded: &[&str]) {
    let socket = Socket::new();
    setup(&socket, body);
    // Even a present response file is outside bounded package metadata. The
    // native fixture preserves argv; it never runs tsc or expands these files.
    fs::write(socket.root.join("nested/watch.flags"), "--watch\n").unwrap();
    fs::write(socket.root.join("nested/emit.flags"), "--noEmit false\n").unwrap();
    let listener = socket.listener.try_clone().unwrap();
    let server = thread::spawn(move || {
        for _ in 0..3 {
            let (stream, request) = next(&listener);
            assert_eq!(
                request.operation,
                arc_ipc::jobs::OPERATION_JOB_START,
                "Unsupported tsc arguments must never negotiate or request admission"
            );
            let classification = &request.payload["classification"];
            assert_eq!(classification["classification"], "UNKNOWN");
            assert_eq!(classification["locality"], "UNKNOWN");
            assert_eq!(classification["uncertainty"], "OBSERVE");
            drop(stream);
        }
    });
    let mut invocation = command(&socket);
    invocation.args(forwarded);
    let result = output(invocation);
    server.join().unwrap_or_else(|_| {
        panic!(
            "{body} {forwarded:?}: status={} stderr={}",
            result.status,
            String::from_utf8_lossy(&result.stderr)
        )
    });
    assert_eq!(result.status.code(), Some(17));
    let data: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    let mut original = vec!["run", "surprise"];
    original.extend_from_slice(forwarded);
    assert_eq!(
        data["args"],
        json!(original
            .iter()
            .map(|s| s.as_bytes().to_vec())
            .collect::<Vec<_>>())
    );
    assert_eq!(
        fs::read_to_string(socket.root.join("starts"))
            .unwrap()
            .lines()
            .count(),
        1
    );
    socket.listener.set_nonblocking(true).unwrap();
    assert_eq!(
        socket.listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock,
        "No additional admission request after the observation"
    );
}

#[test]
fn typecheck_response_files_bypass_admission_in_bodies_and_forwarded_args() {
    for body in [
        "tsc --noEmit @watch.flags",
        "tsc --noEmit @emit.flags",
        "tsc --noEmit -p @emit.flags",
    ] {
        assert_typecheck_observation(body, &[]);
    }
    for forwarded in [vec!["@watch.flags"], vec!["--", "@emit.flags"]] {
        assert_typecheck_observation("tsc --noEmit", &forwarded);
    }
}

#[test]
fn typecheck_invalid_option_spellings_and_arities_bypass_admission() {
    for body in [
        "tsc --noEmit=true",
        "tsc --noEmit --project=tsconfig.json",
        "tsc --noEmit --strict=false",
        "tsc --noEmit -p",
    ] {
        assert_typecheck_observation(body, &[]);
    }
    for forwarded in [
        vec!["--noEmit=true"],
        vec!["--", "--project=tsconfig.json"],
        vec!["--strict=false"],
    ] {
        assert_typecheck_observation("tsc --noEmit", &forwarded);
    }
}

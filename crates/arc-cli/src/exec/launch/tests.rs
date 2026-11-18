use super::*;
use crate::exec::policy::fixture;
use arc_ipc::{read_frame, write_frame, Response};
use std::{
    fs,
    os::unix::{
        fs::DirBuilderExt,
        net::{UnixListener, UnixStream},
    },
    path::PathBuf,
    thread,
};

#[path = "expiry_tests.rs"]
mod expiry;

#[path = "../../../../arc-daemon/tests/support/child_watchdog.rs"]
mod child_watchdog;
#[path = "../../../../arc-daemon/tests/support/child_watchdog_tests.rs"]
mod child_watchdog_tests;

#[path = "socket_fixture.rs"]
mod socket_fixture;
use socket_fixture::Socket;

fn read(stream: &mut UnixStream) -> Request {
    Request::decode(&read_frame(stream).unwrap()).unwrap()
}
fn reply(stream: &mut UnixStream, request: &Request, value: serde_json::Value) {
    write_frame(
        stream,
        &serde_json::to_vec(&Response::ok(&request.request_id, value)).unwrap(),
    )
    .unwrap();
}
fn input() -> LaunchRequest {
    LaunchRequest {
        job_id: JobId::try_from("a".repeat(32)).unwrap(),
        observer: ProcessIdentity::capture(std::process::id()).unwrap(),
        permit_generation: 1,
    }
}

#[path = "release_tests.rs"]
mod release;

use release::wait_for_release_loss;

#[test]
fn release_loss_watchdog_distinguishes_a_closed_socket_from_a_hang_or_data() {
    use std::{io::Write, os::fd::AsRawFd, time::Duration};

    let (reader, mut peer) = UnixStream::pair().unwrap();
    assert_eq!(
        wait_for_release_loss(reader.as_raw_fd(), Duration::ZERO)
            .unwrap_err()
            .kind(),
        io::ErrorKind::TimedOut
    );
    peer.write_all(b"x").unwrap();
    assert_eq!(
        wait_for_release_loss(reader.as_raw_fd(), Duration::from_secs(60))
            .unwrap_err()
            .kind(),
        io::ErrorKind::InvalidData
    );
    drop(peer);
    let (reader, peer) = UnixStream::pair().unwrap();
    drop(peer);
    wait_for_release_loss(reader.as_raw_fd(), Duration::from_secs(60)).unwrap();
}

#[test]
fn held_is_not_outage_and_one_recovery_exec_preserves_the_native_exit() {
    let socket = Socket::new();
    let path = socket.path();
    let owner = input();
    let held = {
        let mut p = fixture::fallback();
        p.mode = arc_ipc::policy::PolicyMode::MemoryHold;
        p.reason = arc_ipc::policy::PolicyReason::MemoryCritical;
        p.observation = critical();
        p
    };
    let server = thread::spawn(move || {
        for n in 0..4 {
            let (mut stream, _) = socket.listener.accept().unwrap();
            let request = read(&mut stream);
            assert_eq!(request.operation, OPERATION_TRAFFIC_LAUNCH);
            assert_eq!(decode_launch_request(&request.payload).unwrap(), owner);
            if n < 2 {
                reply(
                    &mut stream,
                    &request,
                    json!(LaunchResult::Held {
                        policy: held.clone()
                    }),
                );
            } else if n == 2 {
                write_frame(
                    &mut stream,
                    &serde_json::to_vec(&Response::error(
                        &request.request_id,
                        arc_ipc::ERROR_DB_BUSY,
                    ))
                    .unwrap(),
                )
                .unwrap();
            } else {
                reply(
                    &mut stream,
                    &request,
                    json!(LaunchResult::Ready {
                        permit_generation: 1,
                        policy: fixture::fallback()
                    }),
                );
                let ack = read(&mut stream);
                assert_eq!(ack.operation, OPERATION_TRAFFIC_LAUNCH_ACK);
                assert!(matches!(
                    decode_launch_ack(&ack.payload, &owner).unwrap(),
                    LaunchAck::Started { .. }
                ));
                reply(&mut stream, &ack, json!({}));
            }
        }
    });
    let mut notices = 0;
    let Coordination::Fenced(mut gate) =
        wait(&path, input(), &Continuity::default(), &|| false, || {
            notices += 1
        })
        .unwrap()
    else {
        panic!("fenced release required");
    };
    assert_eq!(notices, 1);
    let marker = PathBuf::from(format!(
        "/tmp/arc-once-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    let mut command = Command::new("/bin/sh");
    command
        .args(["-c", "printf x >> \"$ARC_TEST_MARKER\"; exit 17"])
        .env("ARC_TEST_MARKER", &marker);
    gate.install(&mut command);
    let mut child = child_watchdog::Owned::spawn(&mut command).unwrap();
    assert!(gate.acknowledge(Some(ProcessIdentity::capture(child.child.id()).unwrap())));
    assert_eq!(child.wait("held launch target").unwrap().code(), Some(17));
    assert_eq!(fs::read(&marker).unwrap(), b"x");
    fs::remove_file(marker).unwrap();
    server.join().unwrap();
}

fn critical() -> arc_ipc::policy::ObservationSummary {
    use arc_ipc::policy::*;
    ObservationSummary {
        availability: ObservationAvailability::Available,
        pressure: Some(arc_core::meter::MemoryPressure::Critical),
        age_ms: Some(0),
        stale_after_ms: Some(3000),
        sampler: Some(SamplerIdentity {
            daemon_instance: ProcessIdentity::capture(std::process::id()).unwrap(),
            generation: 1,
            monotonic_epoch: 1,
        }),
        sequence: Some(1),
        observed_at_ms: Some(1000),
        observed_monotonic_ms: Some(1000),
    }
}

#[test]
fn explicit_old_daemon_unsupported_is_distinct_from_rejection_and_missing_endpoint() {
    for (code, expected) in [
        (arc_ipc::ERROR_UNSUPPORTED_CAPABILITY, None),
        (arc_ipc::ERROR_INVALID_REQUEST, Some(Failure::Rejected)),
    ] {
        let socket = Socket::new();
        let path = socket.path();
        let server = thread::spawn(move || {
            let (mut stream, _) = socket.listener.accept().unwrap();
            let request = read(&mut stream);
            write_frame(
                &mut stream,
                &serde_json::to_vec(&Response::error(&request.request_id, code)).unwrap(),
            )
            .unwrap();
        });
        let result = wait(&path, input(), &Continuity::default(), &|| false, || {});
        if let Some(expected) = expected {
            assert!(matches!(result, Err(e) if e == expected));
        } else {
            assert!(matches!(result, Ok(Coordination::Legacy)));
        }
        server.join().unwrap();
    }
    let socket = Socket::new();
    let path = socket.path();
    drop(socket);
    assert!(matches!(
        wait(&path, input(), &Continuity::default(), &|| false, || {}),
        Ok(Coordination::Outage)
    ));
}

#[test]
fn cancelled_held_launch_never_falls_open() {
    use std::cell::Cell;
    let socket = Socket::new();
    let path = socket.path();
    let server = thread::spawn(move || {
        let (mut stream, _) = socket.listener.accept().unwrap();
        let request = read(&mut stream);
        let mut p = fixture::fallback();
        p.observation = critical();
        p.mode = arc_ipc::policy::PolicyMode::MemoryHold;
        p.reason = arc_ipc::policy::PolicyReason::MemoryCritical;
        reply(
            &mut stream,
            &request,
            json!(LaunchResult::Held { policy: p }),
        );
    });
    let cancelled = Cell::new(false);
    let result = wait(
        &path,
        input(),
        &Continuity::default(),
        &|| cancelled.get(),
        || cancelled.set(true),
    );
    assert!(matches!(result, Err(Failure::Cancelled)));
    server.join().unwrap();
}

#[test]
fn malformed_or_generation_mismatched_release_cannot_spawn() {
    for value in [
        json!({ "state": "READY", "permit_generation": 2, "policy": fixture::fallback() }),
        json!({ "state": "READY", "policy": [] }),
        json!({ "state": "NOT_FOUND" }),
    ] {
        let socket = Socket::new();
        let path = socket.path();
        let server = thread::spawn(move || {
            let (mut stream, _) = socket.listener.accept().unwrap();
            let request = read(&mut stream);
            reply(&mut stream, &request, value);
        });
        assert!(matches!(
            wait(&path, input(), &Continuity::default(), &|| false, || {}),
            Err(Failure::InvalidReply | Failure::Rejected)
        ));
        server.join().unwrap();
    }
}

#[test]
fn expired_or_lost_release_is_rejected_by_native_pre_exec_before_the_target() {
    for lost in [false, true] {
        let socket = Socket::new();
        let path = socket.path();
        let marker = PathBuf::from(format!(
            "/tmp/arc-no-exec-{}",
            arc_core::process_identity::random_job_id().unwrap()
        ));
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let (closed_tx, closed_rx) = std::sync::mpsc::channel();
        let server = thread::spawn(move || {
            let (mut stream, _) = socket.listener.accept().unwrap();
            let request = read(&mut stream);
            reply(
                &mut stream,
                &request,
                json!(LaunchResult::Ready {
                    permit_generation: 1,
                    policy: fixture::fallback()
                }),
            );
            if lost {
                // A concurrent fork can temporarily inherit this descriptor
                // despite CLOEXEC. Shutdown makes the peer loss visible even
                // while an unrelated short-lived child holds a duplicate.
                stream.shutdown(std::net::Shutdown::Both).unwrap();
                drop(stream);
                closed_tx.send(()).unwrap();
            } else {
                closed_tx.send(()).unwrap();
                done_rx
                    .recv_timeout(std::time::Duration::from_secs(60))
                    .unwrap();
            }
        });
        let Coordination::Fenced(mut gate) =
            wait(&path, input(), &Continuity::default(), &|| false, || {}).unwrap()
        else {
            panic!("expected gate");
        };
        closed_rx
            .recv_timeout(std::time::Duration::from_secs(60))
            .unwrap();
        if lost {
            wait_for_release_loss(gate.session.raw_fd(), std::time::Duration::from_secs(60))
                .expect("release socket readiness watchdog");
        } else {
            gate.deadline = Instant::now();
        }
        let mut command = Command::new("/bin/sh");
        command
            .args(["-c", "printf x >> \"$ARC_TEST_MARKER\""])
            .env("ARC_TEST_MARKER", &marker);
        gate.install(&mut command);
        match command.spawn() {
            Err(error) => assert_eq!(
                error.raw_os_error(),
                Some(if lost {
                    libc::ECONNABORTED
                } else {
                    libc::ETIMEDOUT
                })
            ),
            Ok(mut child) => {
                let _ = child.kill();
                let deadline = Instant::now() + std::time::Duration::from_secs(60);
                let status = loop {
                    if let Some(status) = child.try_wait().unwrap() {
                        break status;
                    }
                    assert!(
                        Instant::now() < deadline,
                        "unexpected target could not be reaped"
                    );
                    thread::yield_now();
                };
                panic!("pre-exec gate admitted a child after release loss: {status:?}");
            }
        }
        assert!(
            !marker.exists(),
            "pre-exec helper is not a target execution"
        );
        let _ = done_tx.send(());
        server.join().unwrap();
    }
}

#[test]
fn a_healthy_hold_followed_by_connection_loss_is_not_outage_fail_open() {
    let socket = Socket::new();
    let path = socket.path();
    let server = thread::spawn(move || {
        let (mut stream, _) = socket.listener.accept().unwrap();
        let request = read(&mut stream);
        let mut p = fixture::fallback();
        p.observation = critical();
        p.mode = arc_ipc::policy::PolicyMode::MemoryHold;
        p.reason = arc_ipc::policy::PolicyReason::MemoryCritical;
        reply(
            &mut stream,
            &request,
            json!(LaunchResult::Held { policy: p }),
        );
    });
    assert!(matches!(
        wait(&path, input(), &Continuity::default(), &|| false, || {}),
        Err(Failure::Unavailable)
    ));
    server.join().unwrap();
}

#[test]
fn exhausted_fresh_observation_is_reevaluated_before_any_native_spawn() {
    let socket = Socket::new();
    let path = socket.path();
    let server = thread::spawn(move || {
        for fresh in [true, false] {
            let (mut stream, _) = socket.listener.accept().unwrap();
            let request = read(&mut stream);
            let mut p = fixture::fallback();
            if fresh {
                p.observation = critical();
                p.observation.pressure = Some(arc_core::meter::MemoryPressure::Normal);
                p.observation.age_ms = p.observation.stale_after_ms;
                p.reason = arc_ipc::policy::PolicyReason::PressureNormal;
            }
            reply(
                &mut stream,
                &request,
                json!(LaunchResult::Ready {
                    permit_generation: 1,
                    policy: p
                }),
            );
            let ack = read(&mut stream);
            assert!(matches!(
                decode_launch_ack(&ack.payload, &input()).unwrap(),
                LaunchAck::Unspawned { .. }
            ));
            reply(&mut stream, &ack, json!({}));
        }
    });
    let Coordination::Fenced(mut gate) =
        wait(&path, input(), &Continuity::default(), &|| false, || {}).unwrap()
    else {
        panic!("L0 fallback release expected");
    };
    assert!(gate.acknowledge(None));
    server.join().unwrap();
}

#[test]
fn lost_launch_ack_never_replays_a_native_execution_or_changes_its_exit() {
    let socket = Socket::new();
    let path = socket.path();
    let server = thread::spawn(move || {
        let (mut stream, _) = socket.listener.accept().unwrap();
        let request = read(&mut stream);
        reply(
            &mut stream,
            &request,
            json!(LaunchResult::Ready {
                permit_generation: 1,
                policy: fixture::fallback()
            }),
        );
        let ack = read(&mut stream);
        assert!(matches!(
            decode_launch_ack(&ack.payload, &input()).unwrap(),
            LaunchAck::Started { .. }
        ));
        // Deliberately lose the acknowledgement after recording the attempt.
    });
    let Coordination::Fenced(mut gate) =
        wait(&path, input(), &Continuity::default(), &|| false, || {}).unwrap()
    else {
        panic!("expected gate");
    };
    let marker = PathBuf::from(format!(
        "/tmp/arc-ack-once-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    let mut command = Command::new("/bin/sh");
    command
        .args(["-c", "printf x >> \"$ARC_TEST_MARKER\"; exit 23"])
        .env("ARC_TEST_MARKER", &marker);
    gate.install(&mut command);
    let mut child = child_watchdog::Owned::spawn(&mut command).unwrap();
    assert!(!gate.acknowledge(Some(ProcessIdentity::capture(child.child.id()).unwrap())));
    assert_eq!(
        child
            .wait("lost launch acknowledgement target")
            .unwrap()
            .code(),
        Some(23)
    );
    assert_eq!(fs::read(&marker).unwrap(), b"x");
    fs::remove_file(marker).unwrap();
    server.join().unwrap();
}

#[test]
fn launch_envelope_identity_and_protocol_are_checked_before_native_admission() {
    for protocol in [true, false] {
        let socket = Socket::new();
        let path = socket.path();
        let server = thread::spawn(move || {
            let (mut stream, _) = socket.listener.accept().unwrap();
            let request = read(&mut stream);
            let mut response = Response::ok(
                &request.request_id,
                json!(LaunchResult::Ready {
                    permit_generation: 1,
                    policy: fixture::fallback()
                }),
            );
            if protocol {
                response.protocol_version = 2;
            } else {
                response.request_id = "another-launch".into();
            }
            write_frame(&mut stream, &serde_json::to_vec(&response).unwrap()).unwrap();
        });
        assert!(matches!(
            wait(&path, input(), &Continuity::default(), &|| false, || {}),
            Err(Failure::Protocol | Failure::InvalidReply)
        ));
        server.join().unwrap();
    }
}

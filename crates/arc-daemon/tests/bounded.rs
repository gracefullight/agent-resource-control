//! Actual daemon and CLI checks; native observations are separate from cap proofs.
#[path = "support/measurements.rs"]
mod measurements;
#[path = "support/native.rs"]
mod native;
#[path = "support/runtime.rs"]
mod runtime;
mod support;

use runtime::{assert_closed, receipt, send, Fixture};
use std::{
    io::Write,
    time::{Duration, Instant},
};

#[test]
fn malformed_and_stalled_frames_recover_actual_cli() {
    let _serial = support::serial();
    let fixture = Fixture::new();
    let mut child = fixture.spawn("test");
    let mut idle = fixture.connect();
    idle.write_all(&[0]).unwrap();
    // Closure is observed, never inferred from a sleep. Paused-time units prove
    // the exact deadline; this subprocess allows 250 ms native scheduling margin.
    assert_closed(idle, Duration::from_millis(400));
    let mut body = fixture.connect();
    body.write_all(&1024u32.to_be_bytes()).unwrap();
    body.write_all(b"{").unwrap();
    assert_closed(body, Duration::from_millis(550));

    let mut stream = fixture.connect();
    stream.write_all(&1u32.to_be_bytes()).unwrap();
    stream.write_all(b"{").unwrap();
    let invalid: arc_ipc::Response = arc_ipc::read_message(&mut stream).unwrap();
    assert_eq!(invalid.error.unwrap().code, arc_ipc::ERROR_INVALID_REQUEST);
    let mut version = arc_ipc::Request::new("wrong", arc_ipc::OPERATION_DOCTOR);
    version.protocol_version = 99;
    let response = arc_ipc::exchange(&mut stream, &version).unwrap();
    assert_eq!(
        response.error.unwrap().code,
        arc_ipc::ERROR_PROTOCOL_INCOMPATIBLE
    );
    let response = arc_ipc::exchange(
        &mut stream,
        &arc_ipc::Request::new("ok", arc_ipc::OPERATION_HELLO),
    )
    .unwrap();
    assert_eq!(response.status, arc_ipc::STATUS_OK);
    drop(stream);
    for (length, code) in [
        (0, arc_ipc::ERROR_INVALID_REQUEST),
        (arc_ipc::MAX_FRAME_BYTES + 1, arc_ipc::ERROR_FRAME_TOO_LARGE),
    ] {
        let mut stream = fixture.connect();
        stream.write_all(&length.to_be_bytes()).unwrap();
        let response: arc_ipc::Response = arc_ipc::read_message(&mut stream).unwrap();
        assert_eq!(response.error.unwrap().code, code);
        assert_closed(stream, Duration::from_millis(250));
    }
    fixture.assert_cli();
    let (exit_us, peak) = child.stop(libc::SIGTERM);
    let report = fixture.report();
    assert_eq!(report["metrics"]["header_timeouts"], 1);
    assert_eq!(report["metrics"]["frame_timeouts"], 1);
    receipt(
        "frames",
        &serde_json::json!({"report":report,"signal_to_exit_us":exit_us,"native_peak_rss_bytes":peak}),
    );
}

#[test]
fn nonreading_peer_releases_real_daemon_capacity() {
    let _serial = support::serial();
    let fixture = Fixture::new();
    let mut child = fixture.spawn("test");
    let mut peer = fixture.connect();
    let size: libc::c_int = 1024;
    use std::os::fd::AsRawFd;
    assert_eq!(
        unsafe {
            libc::setsockopt(
                peer.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_RCVBUF,
                &size as *const _ as *const libc::c_void,
                std::mem::size_of_val(&size) as libc::socklen_t,
            )
        },
        0
    );
    send(
        &mut peer,
        &arc_ipc::Request::new("x".repeat(512 * 1024), arc_ipc::OPERATION_HELLO),
    );
    // Leave the peer unread until its kernel reports EOF/HUP via poll. No bytes
    // are drained to create writable space while the server deadline expires.
    let started = Instant::now();
    loop {
        let mut poll = libc::pollfd {
            fd: peer.as_raw_fd(),
            events: libc::POLLHUP,
            revents: 0,
        };
        assert!(unsafe { libc::poll(&mut poll, 1, 10) } >= 0);
        if poll.revents & libc::POLLHUP != 0 {
            break;
        }
        assert!(
            started.elapsed() < Duration::from_millis(800),
            "nonreading socket not closed"
        );
    }
    let closure_us = started.elapsed().as_micros();
    drop(peer);
    fixture.assert_cli();
    let (exit_us, _) = child.stop(libc::SIGTERM);
    let report = fixture.report();
    assert_eq!(
        report["metrics"]["write_timeouts"], 1,
        "actual backpressure required: {report}"
    );
    receipt(
        "nonreader",
        &serde_json::json!({"report":report,"closure_us":closure_us,"signal_to_exit_us":exit_us}),
    );
}

#[test]
fn native_probe_then_full_100000_attempt_workload() {
    measurements::run();
}

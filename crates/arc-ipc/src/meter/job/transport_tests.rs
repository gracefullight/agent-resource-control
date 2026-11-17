//! Real same-user Unix transport; durable job binding/persistence is not mocked
//! as accepted product evidence by these client-only exchange tests.
use super::*;
use crate::client::Failure;
use std::{
    os::{
        fd::AsRawFd,
        unix::{
            fs::PermissionsExt,
            net::{UnixListener, UnixStream},
        },
    },
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

struct Endpoint {
    root: PathBuf,
    socket: PathBuf,
    listener: UnixListener,
}
impl Endpoint {
    fn new() -> Self {
        let root = PathBuf::from("/tmp").join(format!(
            "arc-jm-{}",
            arc_core::process_identity::random_job_id().unwrap()
        ));
        std::fs::create_dir(&root).unwrap();
        std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        let socket = root.join("s");
        let listener = UnixListener::bind(&socket).unwrap();
        listener.set_nonblocking(true).unwrap();
        std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600)).unwrap();
        Self {
            root,
            socket,
            listener,
        }
    }
    fn accept(&self) -> UnixStream {
        let end = Instant::now() + Duration::from_secs(60);
        loop {
            match self.listener.accept() {
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
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => (),
                Err(error) => panic!("Meter peer accept failed: {error}"),
            }
            let remaining = end.saturating_duration_since(Instant::now());
            assert!(
                !remaining.is_zero(),
                "Meter peer never received a connection"
            );
            let mut fd = libc::pollfd {
                fd: self.listener.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            let timeout = remaining.as_millis().max(1).min(i32::MAX as u128) as i32;
            let ready = unsafe { libc::poll(&mut fd, 1, timeout) };
            if ready < 0 {
                let error = std::io::Error::last_os_error();
                if error.kind() != std::io::ErrorKind::Interrupted {
                    panic!("Meter peer poll failed: {error}");
                }
            } else if ready == 0 {
                panic!("Meter peer never received a connection");
            } else if fd.revents & (libc::POLLERR | libc::POLLNVAL) != 0 {
                panic!("Meter peer listener failed: {}", fd.revents);
            }
        }
    }
}
impl Drop for Endpoint {
    fn drop(&mut self) {
        std::fs::remove_file(&self.socket).unwrap();
        std::fs::remove_dir(&self.root).unwrap();
    }
}

#[test]
fn accepted_transport_peer_supports_blocking_framed_io() {
    let endpoint = Endpoint::new();
    let mut client = UnixStream::connect(&endpoint.socket).unwrap();
    let mut peer = endpoint.accept();
    // macOS inherits O_NONBLOCK from the listener. Framing uses read_exact
    // and write_all, so accepted fixture peers must restore blocking I/O.
    let flags = unsafe { libc::fcntl(peer.as_raw_fd(), libc::F_GETFL) };
    assert!(flags >= 0);
    assert_eq!(flags & libc::O_NONBLOCK, 0);
    let request = summary_request("framed", &fixture().job_id).unwrap();
    crate::write_message(&mut client, &request).unwrap();
    let received: Request = crate::read_message(&mut peer).unwrap();
    assert_eq!(received, request);
}

#[test]
fn lost_ack_retries_the_exact_report_without_any_execution_path() {
    let endpoint = Endpoint::new();
    let socket = endpoint.socket.clone();
    let server = thread::spawn(move || {
        let mut first = endpoint.accept();
        let original: Request = crate::read_message(&mut first).unwrap();
        drop(first); // simulate commit followed by a lost acknowledgement
        let mut second = endpoint.accept();
        let retried: Request = crate::read_message(&mut second).unwrap();
        assert_eq!(original, retried);
        let mut unchanged = receipt();
        unchanged.disposition = ReportDisposition::Unchanged;
        crate::write_response(&mut second, &report_response(&retried, &unchanged)).unwrap();
    });
    let mut possibly_committed = false;
    let now = Instant::now();
    let result = report_until_with_clock(
        &socket,
        "stable-report",
        &fixture(),
        now + Duration::from_secs(1),
        &|| false,
        &mut possibly_committed,
        &|| now,
    )
    .unwrap();
    assert_eq!(result.disposition, ReportDisposition::Unchanged);
    assert!(possibly_committed);
    server.join().unwrap();
}

#[test]
fn stalled_reporting_obeys_its_clamped_post_termination_budget() {
    let endpoint = Endpoint::new();
    let socket = endpoint.socket.clone();
    let expired = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let reached = expired.clone();
    let (release, hold) = std::sync::mpsc::channel();
    let server = thread::spawn(move || {
        let mut peer = endpoint.accept();
        let request: Request = crate::read_message(&mut peer).unwrap();
        assert_eq!(request.operation, OPERATION_METER_REPORT);
        reached.store(true, std::sync::atomic::Ordering::SeqCst);
        // Keep the socket open and send no acknowledgement. A disconnect cannot
        // explain the client's outcome. The watchdog only diagnoses a hang.
        hold.recv_timeout(Duration::from_secs(60))
            .expect("clamped report did not finish while peer stalled");
        drop(peer);
    });
    let start = Instant::now();
    let deadline = start + Duration::from_millis(METER_REPORT_MAX_BUDGET_MS);
    let now = || {
        if expired.load(std::sync::atomic::Ordering::SeqCst) {
            deadline
        } else {
            start
        }
    };
    let mut uncertain = false;
    let report = fixture();
    assert_eq!(
        report_until_with_clock(
            &socket,
            "stalled",
            &report,
            start + Duration::from_secs(10),
            &|| false,
            &mut uncertain,
            &now,
        ),
        Err(Failure::Uncertain)
    );
    assert!(uncertain);
    assert!(expired.load(std::sync::atomic::Ordering::SeqCst));
    release.send(()).unwrap();
    server.join().unwrap();
}

#[test]
fn report_passes_the_clamped_absolute_deadline_to_transport() {
    let report = fixture();
    let request = report_request("deadline", &report).unwrap();
    let now = Instant::now();
    let max = Duration::from_millis(METER_REPORT_MAX_BUDGET_MS);
    for original in [now + max / 2, now + max * 4] {
        let mut committed = false;
        let result = report_exchange(
            &request,
            &report,
            original,
            now,
            &mut committed,
            |actual_request, deadline, _| {
                assert_eq!(actual_request, &request);
                assert_eq!(deadline, original.min(now + max));
                Err(Failure::Unavailable)
            },
        );
        assert_eq!(result, Err(Failure::Unavailable));
        assert!(!committed);
    }
}

#[test]
fn unsupported_and_unavailable_stay_distinct_on_authenticated_transport() {
    for (operation, code, expected) in [
        (
            OPERATION_METER_REPORT,
            crate::ERROR_UNSUPPORTED_CAPABILITY,
            Failure::Unsupported,
        ),
        (
            OPERATION_METER_SUMMARY,
            crate::ERROR_UNSUPPORTED_CAPABILITY,
            Failure::Unsupported,
        ),
        (
            OPERATION_METER_REPORT,
            super::super::super::ERROR_METER_UNAVAILABLE,
            Failure::Unavailable,
        ),
        (
            OPERATION_METER_SUMMARY,
            super::super::super::ERROR_METER_UNAVAILABLE,
            Failure::Unavailable,
        ),
        (
            OPERATION_METER_REPORT,
            ERROR_METER_REPORT_CONFLICT,
            Failure::Rejected,
        ),
    ] {
        let endpoint = Endpoint::new();
        let socket = endpoint.socket.clone();
        let server = thread::spawn(move || {
            let mut peer = endpoint.accept();
            let request: Request = crate::read_message(&mut peer).unwrap();
            assert_eq!(request.operation, operation);
            crate::write_response(&mut peer, &Response::error(request.request_id, code)).unwrap();
        });
        let now = Instant::now();
        let end = now + Duration::from_secs(1);
        let failure = if operation == OPERATION_METER_REPORT {
            report_until_with_clock(
                &socket,
                "unsupported",
                &fixture(),
                end,
                &|| false,
                &mut false,
                &|| now,
            )
            .unwrap_err()
        } else {
            summary_until_with_clock(
                &socket,
                "unsupported",
                &fixture().job_id,
                end,
                &|| false,
                &|| now,
            )
            .unwrap_err()
        };
        assert_eq!(failure, expected);
        server.join().unwrap();
    }
}

#[test]
fn deadlines_cancellation_and_invalid_local_input_are_bounded() {
    let path = std::path::Path::new("/unused-job-meter.sock");
    let start = Instant::now();
    assert_eq!(
        report_until(
            path,
            "c",
            &fixture(),
            start + Duration::from_secs(1),
            &|| true,
            &mut false
        ),
        Err(Failure::Cancelled)
    );
    assert_eq!(
        summary_until(
            path,
            "c",
            &fixture().job_id,
            start + Duration::from_secs(1),
            &|| true
        ),
        Err(Failure::Cancelled)
    );
    assert_eq!(
        report_until(path, "e", &fixture(), start, &|| false, &mut false),
        Err(Failure::Unavailable)
    );
    assert_eq!(
        report_until(path, "e", &fixture(), start, &|| false, &mut true),
        Err(Failure::Uncertain)
    );
    assert_eq!(
        summary_until(path, "e", &fixture().job_id, start, &|| false),
        Err(Failure::Unavailable)
    );
    assert_eq!(
        summary_until(path, "e", "bad", start, &|| false),
        Err(Failure::Local)
    );
}

#[test]
fn valid_summary_and_malformed_report_ack_are_correlated() {
    for report_case in [false, true] {
        let endpoint = Endpoint::new();
        let socket = endpoint.socket.clone();
        let server = thread::spawn(move || {
            let mut peer = endpoint.accept();
            let request: Request = crate::read_message(&mut peer).unwrap();
            let response = if report_case {
                Response::ok(&request.request_id, json!({}))
            } else {
                summary_response(&request, &summary())
            };
            crate::write_response(&mut peer, &response).unwrap();
        });
        let mut uncertain = false;
        let logical_now = Instant::now() + Duration::from_secs(3600);
        if report_case {
            assert_eq!(
                report_until_with_clock(
                    &socket,
                    "r",
                    &fixture(),
                    logical_now + Duration::from_secs(1),
                    &|| false,
                    &mut uncertain,
                    &|| logical_now,
                ),
                Err(Failure::InvalidReply)
            );
            assert!(uncertain);
        } else {
            assert_eq!(
                summary_until_with_clock(
                    &socket,
                    "s",
                    &fixture().job_id,
                    logical_now + Duration::from_secs(1),
                    &|| false,
                    &|| logical_now,
                ),
                Ok(summary())
            );
        }
        server.join().unwrap();
    }
}

#[test]
fn injected_now_clamps_report_and_summary_without_renewing_expired_budgets() {
    let now = Instant::now();
    for max_ms in [METER_REPORT_MAX_BUDGET_MS, METER_SUMMARY_MAX_BUDGET_MS] {
        for offset in [0, max_ms / 2, max_ms, max_ms + 10] {
            let original = now + Duration::from_millis(offset);
            assert_eq!(
                clamp_deadline(now, original, max_ms),
                now + Duration::from_millis(offset.min(max_ms))
            );
        }
        let expired = now - Duration::from_millis(1);
        assert_eq!(clamp_deadline(now, expired, max_ms), expired);
    }
    let socket = std::path::Path::new("/unused-job-meter-clock.sock");
    let expired_now = || now + Duration::from_secs(10);
    assert_eq!(
        report_until_with_clock(
            socket,
            "expired",
            &fixture(),
            now + Duration::from_secs(1),
            &|| false,
            &mut false,
            &expired_now
        ),
        Err(Failure::Unavailable)
    );
    assert_eq!(
        summary_until_with_clock(
            socket,
            "expired",
            &fixture().job_id,
            now + Duration::from_secs(1),
            &|| false,
            &expired_now
        ),
        Err(Failure::Unavailable)
    );
}

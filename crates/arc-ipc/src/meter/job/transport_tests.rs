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
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match self.listener.accept() {
                Ok((stream, _)) => {
                    stream.set_nonblocking(false).unwrap();
                    stream
                        .set_read_timeout(Some(Duration::from_secs(2)))
                        .unwrap();
                    stream
                        .set_write_timeout(Some(Duration::from_secs(2)))
                        .unwrap();
                    return stream;
                }
                Err(e)
                    if e.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < deadline =>
                {
                    let mut ready = libc::pollfd {
                        fd: self.listener.as_raw_fd(),
                        events: libc::POLLIN,
                        revents: 0,
                    };
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    let result = unsafe {
                        libc::poll(&mut ready, 1, remaining.as_millis().try_into().unwrap())
                    };
                    assert_ne!(result, 0, "accept timed out");
                    if result < 0 {
                        assert_eq!(
                            std::io::Error::last_os_error().kind(),
                            std::io::ErrorKind::Interrupted
                        );
                    }
                }
                Err(e) => panic!("accept failed or timed out: {e}"),
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
    let result = report_until(
        &socket,
        "stable-report",
        &fixture(),
        Instant::now() + Duration::from_secs(1),
        &|| false,
        &mut possibly_committed,
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
    let (done, release) = std::sync::mpsc::channel();
    let server = thread::spawn(move || {
        let mut peer = endpoint.accept();
        let request: Request = crate::read_message(&mut peer).unwrap();
        assert_eq!(request.operation, OPERATION_METER_REPORT);
        release.recv_timeout(Duration::from_secs(2)).unwrap();
    });
    let start = Instant::now();
    let mut uncertain = false;
    assert_eq!(
        report_until(
            &socket,
            "stalled",
            &fixture(),
            start + Duration::from_secs(10),
            &|| false,
            &mut uncertain
        ),
        Err(Failure::Uncertain)
    );
    let elapsed = start.elapsed();
    assert!(uncertain);
    assert!(elapsed >= Duration::from_millis(400) && elapsed < Duration::from_millis(1200));
    done.send(()).unwrap();
    server.join().unwrap();
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
        let (ready, started) = std::sync::mpsc::sync_channel(0);
        let server = thread::spawn(move || {
            ready.send(()).unwrap();
            let mut peer = endpoint.accept();
            let request: Request = crate::read_message(&mut peer).unwrap();
            assert_eq!(request.operation, operation);
            crate::write_response(&mut peer, &Response::error(request.request_id, code)).unwrap();
        });
        started.recv_timeout(Duration::from_secs(2)).unwrap();
        let end = Instant::now() + Duration::from_secs(1);
        let failure = if operation == OPERATION_METER_REPORT {
            report_until(
                &socket,
                "unsupported",
                &fixture(),
                end,
                &|| false,
                &mut false,
            )
            .unwrap_err()
        } else {
            summary_until(&socket, "unsupported", &fixture().job_id, end, &|| false).unwrap_err()
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
    assert!(start.elapsed() < Duration::from_millis(100));
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
        if report_case {
            assert_eq!(
                report_until(
                    &socket,
                    "r",
                    &fixture(),
                    Instant::now() + Duration::from_secs(1),
                    &|| false,
                    &mut uncertain
                ),
                Err(Failure::InvalidReply)
            );
            assert!(uncertain);
        } else {
            assert_eq!(
                summary_until(
                    &socket,
                    "s",
                    &fixture().job_id,
                    Instant::now() + Duration::from_secs(1),
                    &|| false
                ),
                Ok(summary())
            );
        }
        server.join().unwrap();
    }
}

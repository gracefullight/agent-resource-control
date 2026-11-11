use super::{call_until, Failure};
use arc_ipc::{Request, Response};
use serde_json::Value;
use std::{
    fs,
    os::fd::AsRawFd,
    os::unix::net::UnixListener,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc, Arc,
    },
    thread,
};
use std::{
    io::Write,
    os::unix::net::UnixStream,
    path::Path,
    time::{Duration, Instant},
};
static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Peer {
    path: PathBuf,
    join: Option<thread::JoinHandle<()>>,
    done: mpsc::Receiver<()>,
    deadline: Instant,
}
impl Peer {
    fn new(n: usize, reply: impl FnMut(&mut UnixStream, Request) + Send + 'static) -> Self {
        Self::with_timeout(n, Duration::from_secs(60), reply)
    }
    fn with_timeout(
        n: usize,
        timeout: Duration,
        reply: impl FnMut(&mut UnixStream, Request) + Send + 'static,
    ) -> Self {
        Self::with_setup(n, timeout, || {}, reply)
    }
    fn with_setup(
        n: usize,
        timeout: Duration,
        setup: impl FnOnce(),
        mut reply: impl FnMut(&mut UnixStream, Request) + Send + 'static,
    ) -> Self {
        // The first historical call verifies pinned sources. Finish that work
        // before a test creates the deadline for its exchange.
        super::super::historical_integrity::verify();
        setup();
        let path = std::env::temp_dir().join(format!(
            "arc-transport-{}-{}.sock",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let listener = UnixListener::bind(&path).unwrap();
        listener.set_nonblocking(true).unwrap();
        let deadline = Instant::now() + timeout;
        let (done_tx, done) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel();
        let join = thread::spawn(move || {
            ready_tx.send(()).unwrap();
            for _ in 0..n {
                let mut stream = loop {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    assert!(!remaining.is_zero(), "peer accept deadline exceeded");
                    let mut descriptor = libc::pollfd {
                        fd: listener.as_raw_fd(),
                        events: libc::POLLIN,
                        revents: 0,
                    };
                    // SAFETY: the descriptor is live while the listener is owned here.
                    let ready = unsafe {
                        libc::poll(
                            &mut descriptor,
                            1,
                            remaining.as_millis().clamp(1, i32::MAX as u128) as i32,
                        )
                    };
                    if ready < 0 {
                        let error = std::io::Error::last_os_error();
                        if error.kind() == std::io::ErrorKind::Interrupted {
                            continue;
                        }
                        panic!("peer poll failed: {error}");
                    }
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => continue,
                        Err(error) => panic!("peer accept failed: {error}"),
                    }
                };
                let remaining = deadline.saturating_duration_since(Instant::now());
                assert!(!remaining.is_zero(), "peer read deadline exceeded");
                stream.set_nonblocking(false).unwrap();
                stream.set_read_timeout(Some(remaining)).unwrap();
                stream.set_write_timeout(Some(remaining)).unwrap();
                let request = arc_ipc::read_message(&mut stream).unwrap();
                reply(&mut stream, request);
            }
            let _ = done_tx.send(());
        });
        ready_rx
            .recv_timeout(Duration::from_secs(30))
            .expect("peer did not start");
        Self {
            path,
            join: Some(join),
            done,
            deadline,
        }
    }
    fn call(&self) -> Result<Value, Failure> {
        call_until(
            &self.path,
            "fixture",
            Value::Null,
            self.deadline
                .min(Instant::now() + Duration::from_millis(1800)),
            &|| false,
        )
    }
}
impl Drop for Peer {
    fn drop(&mut self) {
        let result = self
            .done
            .recv_timeout(self.deadline.saturating_duration_since(Instant::now()));
        let _ = fs::remove_file(&self.path);
        let join = self.join.take().unwrap();
        match result {
            Ok(()) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                let result = join.join();
                if !thread::panicking() {
                    result.unwrap();
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if !thread::panicking() {
                    panic!("peer exceeded hard deadline");
                }
            }
        }
    }
}
#[test]
fn missing_peer_connection_fails_within_the_hard_deadline() {
    let start = Instant::now();
    let result = std::panic::catch_unwind(|| {
        let _peer = Peer::with_timeout(1, Duration::from_millis(100), |_, _| {});
    });
    assert!(result.is_err());
    assert!(start.elapsed() < Duration::from_secs(30));
}
#[test]
fn slow_setup_cannot_consume_the_exchange_deadline() {
    let (setup_started_tx, setup_started_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let (result_tx, result_rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        let peer = Peer::with_setup(
            1,
            Duration::from_secs(60),
            || {
                setup_started_tx.send(()).unwrap();
                release_rx.recv_timeout(Duration::from_secs(30)).unwrap();
            },
            |stream, request| {
                arc_ipc::write_response(
                    stream,
                    &Response::ok(request.request_id, Value::Bool(true)),
                )
                .unwrap();
            },
        );
        let deadline = Instant::now() + Duration::from_millis(500);
        result_tx
            .send(call_until(
                &peer.path,
                "REGISTER",
                Value::Null,
                deadline,
                &|| false,
            ))
            .unwrap();
    });
    setup_started_rx
        .recv_timeout(Duration::from_secs(30))
        .unwrap();
    // This controlled delay exceeds both the original and repaired deadlines.
    assert!(matches!(
        result_rx.recv_timeout(Duration::from_millis(750)),
        Err(mpsc::RecvTimeoutError::Timeout)
    ));
    release_tx.send(()).unwrap();
    assert_eq!(
        result_rx.recv_timeout(Duration::from_secs(30)).unwrap(),
        Ok(Value::Bool(true))
    );
    worker.join().unwrap();
}
#[test]
fn unavailable_uncertain_and_idempotent_retry_keep_distinct_decisions() {
    assert_eq!(
        call_until(
            Path::new("/missing/arc.sock"),
            "fixture",
            Value::Null,
            Instant::now() + Duration::from_secs(1),
            &|| false
        ),
        Err(Failure::Unavailable)
    );
    let peer = Peer::new(3, |_, _| {});
    assert_eq!(peer.call(), Err(Failure::Uncertain));
    let mut ids = Vec::new();
    let peer = Peer::new(3, move |stream, request| {
        ids.push(request.request_id.clone());
        assert!(ids.iter().all(|id| id == &ids[0]));
        if ids.len() == 3 {
            arc_ipc::write_response(stream, &Response::ok(request.request_id, Value::Bool(true)))
                .unwrap();
        }
    });
    assert_eq!(peer.call(), Ok(Value::Bool(true)));
}
#[test]
fn db_busy_exhaustion_is_uncertain_but_explicit_errors_are_nonlaunching() {
    let peer = Peer::new(3, |stream, r| {
        arc_ipc::write_response(
            stream,
            &Response::error(r.request_id, arc_ipc::ERROR_DB_BUSY),
        )
        .unwrap();
    });
    assert_eq!(peer.call(), Err(Failure::Busy));
    for code in [
        "INVALID_REQUEST",
        "AUTHENTICATION_FAILED",
        "UNKNOWN_REJECTION",
        "PROTOCOL_INCOMPATIBLE",
    ] {
        let peer = Peer::new(1, move |stream, r| {
            arc_ipc::write_response(stream, &Response::error(r.request_id, code)).unwrap();
        });
        assert_eq!(
            peer.call(),
            Err(if code == "PROTOCOL_INCOMPATIBLE" {
                Failure::Protocol
            } else {
                Failure::Rejected
            })
        );
    }
}
#[test]
fn complete_invalid_frames_and_envelopes_are_not_outages() {
    for mode in 0..8 {
        let peer = Peer::new(1, move |stream, r| {
            if mode == 0 {
                stream.write_all(&0u32.to_be_bytes()).unwrap();
                return;
            }
            if mode == 1 {
                stream
                    .write_all(&(arc_ipc::MAX_FRAME_BYTES + 1).to_be_bytes())
                    .unwrap();
                return;
            }
            if mode == 2 {
                arc_ipc::write_frame(stream, b"{malformed").unwrap();
                return;
            }
            let mut response = Response::ok(r.request_id, Value::Null);
            match mode {
                3 => response.protocol_version = 2,
                4 => response.request_id = "wrong".into(),
                5 => response.error = Response::error("x", "REJECT").error,
                6 => response.result = None,
                _ => response.status = "unknown".into(),
            }
            arc_ipc::write_response(stream, &response).unwrap();
        });
        assert_eq!(
            peer.call(),
            Err(if mode == 3 {
                Failure::Protocol
            } else {
                Failure::InvalidReply
            })
        );
    }
}
#[test]
fn absolute_deadline_is_shared_and_partial_reads_cannot_renew_it() {
    let (tx, rx) = mpsc::channel();
    let (received_tx, received_rx) = mpsc::channel();
    let peer = Peer::new(1, move |stream, _| {
        received_tx.send(()).unwrap();
        stream.write_all(&100u32.to_be_bytes()).unwrap();
        stream.write_all(b"{").unwrap();
        rx.recv_timeout(Duration::from_secs(60)).unwrap();
    });
    // Stay below the historical transport's 600 ms per-attempt bound.
    let deadline = Instant::now() + Duration::from_millis(500);
    assert_eq!(
        call_until(&peer.path, "REGISTER", Value::Null, deadline, &|| false),
        Err(Failure::Uncertain)
    );
    received_rx.recv_timeout(Duration::from_secs(30)).unwrap();
    assert!(Instant::now() >= deadline);
    // Reusing this deadline for CLAIM admits no exchange and no extra budget.
    assert_eq!(
        call_until(&peer.path, "CLAIM", Value::Null, deadline, &|| false),
        Err(Failure::Unavailable)
    );
    tx.send(()).unwrap();
}
#[test]
fn cancellation_is_observed_while_waiting_and_at_expired_admission() {
    let cancel = Arc::new(AtomicBool::new(false));
    let observed = cancel.clone();
    let (tx, rx) = mpsc::channel();
    let peer = Peer::new(1, move |_, _| {
        observed.store(true, Ordering::SeqCst);
        rx.recv_timeout(Duration::from_secs(5)).unwrap();
    });
    let start = Instant::now();
    assert_eq!(
        call_until(
            &peer.path,
            "REGISTER",
            Value::Null,
            start + Duration::from_secs(1),
            &|| cancel.load(Ordering::SeqCst)
        ),
        Err(Failure::Cancelled)
    );
    assert!(start.elapsed() < Duration::from_millis(250));
    assert_eq!(
        call_until(&peer.path, "CLAIM", Value::Null, start, &|| true),
        Err(Failure::Cancelled)
    );
    tx.send(()).unwrap();
}
#[test]
fn invalid_socket_path_is_local_failure() {
    let path = PathBuf::from("x".repeat(200));
    assert_eq!(
        call_until(
            &path,
            "REGISTER",
            Value::Null,
            Instant::now() + Duration::from_secs(1),
            &|| false
        ),
        Err(Failure::Local)
    );
}

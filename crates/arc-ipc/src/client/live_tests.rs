use super::{call_until, call_until_no_busy_retry, call_until_tracked, Failure};
use crate::{Request, Response};
use serde_json::Value;
use std::{
    fs,
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

#[test]
fn bun_unsupported_is_no_commit_only_without_an_earlier_possible_commit() {
    unsupported_after_send(crate::traffic::OPERATION_BUN_JOB);
}
#[test]
fn governor_unsupported_is_no_commit_only_without_an_earlier_possible_commit() {
    unsupported_after_send(crate::governor::OPERATION_JOB);
}
#[test]
fn governor_unsupported_after_unconfirmed_store_cannot_claim_rejection() {
    let peer = Peer::new(1, |stream, request| {
        crate::write_response(
            stream,
            &Response::error(request.request_id, crate::ERROR_UNSUPPORTED_CAPABILITY),
        )
        .unwrap();
    });
    let continuity = super::Continuity::default();
    continuity.record(None);
    let request = Request::new("prior-store", crate::governor::OPERATION_JOB);
    assert_eq!(
        super::exchange_until(
            &peer.path,
            &request,
            Instant::now() + Duration::from_secs(2),
            &|| false,
            &mut false,
            super::ExchangeOptions {
                continuity: Some(&continuity),
                retry_busy: true
            }
        ),
        Err(Failure::Uncertain)
    );
}
fn unsupported_after_send(operation: &'static str) {
    for mode in 0..4 {
        let calls = if mode == 1 || mode == 2 { 2 } else { 1 };
        let mut count = 0;
        let peer = Peer::new(calls, move |stream, request| {
            count += 1;
            assert_eq!(request.operation, operation);
            if count == 1 && mode == 1 {
                return;
            }
            let code = if count == 1 && mode == 2 {
                crate::ERROR_DB_BUSY
            } else {
                crate::ERROR_UNSUPPORTED_CAPABILITY
            };
            crate::write_response(stream, &Response::error(request.request_id, code)).unwrap();
        });
        let mut prior = mode == 3;
        let result = call_until_tracked(
            &peer.path,
            operation,
            serde_json::json!({"action":"ENQUEUE"}),
            Instant::now() + Duration::from_secs(2),
            &|| false,
            &mut prior,
        );
        assert_eq!(
            result,
            Err(if mode == 0 {
                Failure::Unsupported
            } else {
                Failure::Uncertain
            })
        );
        assert_eq!(prior, mode != 0);
    }
}
struct Peer {
    path: PathBuf,
    join: Option<thread::JoinHandle<()>>,
    done: mpsc::Receiver<()>,
    deadline: Instant,
}
impl Peer {
    fn new(n: usize, reply: impl FnMut(&mut UnixStream, Request) + Send + 'static) -> Self {
        Self::with_timeout(n, Duration::from_secs(10), reply)
    }
    fn with_timeout(
        n: usize,
        timeout: Duration,
        mut reply: impl FnMut(&mut UnixStream, Request) + Send + 'static,
    ) -> Self {
        let path = std::env::temp_dir().join(format!(
            "arc-transport-live-{}-{}.sock",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let listener = UnixListener::bind(&path).unwrap();
        listener.set_nonblocking(true).unwrap();
        let deadline = Instant::now() + timeout;
        let (done_tx, done) = mpsc::channel();
        let join = thread::spawn(move || {
            for _ in 0..n {
                let mut stream = loop {
                    assert!(Instant::now() < deadline, "peer accept deadline exceeded");
                    match listener.accept() {
                        Ok((stream, _)) => break stream,
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            thread::sleep(Duration::from_millis(2));
                        }
                        Err(error) => panic!("peer accept failed: {error}"),
                    }
                };
                let remaining = deadline.saturating_duration_since(Instant::now());
                assert!(!remaining.is_zero(), "peer read deadline exceeded");
                stream.set_nonblocking(false).unwrap();
                stream.set_read_timeout(Some(remaining)).unwrap();
                stream.set_write_timeout(Some(remaining)).unwrap();
                let request = crate::read_message(&mut stream).unwrap();
                reply(&mut stream, request);
            }
            let _ = done_tx.send(());
        });
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
fn live_unavailable_uncertain_and_idempotent_retry_keep_distinct_decisions() {
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
            crate::write_response(stream, &Response::ok(request.request_id, Value::Bool(true)))
                .unwrap();
        }
    });
    assert_eq!(peer.call(), Ok(Value::Bool(true)));
}
#[test]
fn live_db_busy_exhaustion_is_uncertain_but_explicit_errors_are_nonlaunching() {
    let peer = Peer::new(3, |stream, r| {
        crate::write_response(stream, &Response::error(r.request_id, crate::ERROR_DB_BUSY))
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
            crate::write_response(stream, &Response::error(r.request_id, code)).unwrap();
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
fn live_complete_invalid_frames_and_envelopes_are_not_outages() {
    for mode in 0..8 {
        let peer = Peer::new(1, move |stream, r| {
            if mode == 0 {
                stream.write_all(&0u32.to_be_bytes()).unwrap();
                return;
            }
            if mode == 1 {
                stream
                    .write_all(&(crate::MAX_FRAME_BYTES + 1).to_be_bytes())
                    .unwrap();
                return;
            }
            if mode == 2 {
                crate::write_frame(stream, b"{malformed").unwrap();
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
            crate::write_response(stream, &response).unwrap();
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
fn live_absolute_deadline_is_shared_and_partial_reads_cannot_renew_it() {
    let (tx, rx) = mpsc::channel();
    let peer = Peer::new(1, move |stream, _| {
        stream.write_all(&100u32.to_be_bytes()).unwrap();
        stream.write_all(b"{").unwrap();
        rx.recv_timeout(Duration::from_secs(5)).unwrap();
    });
    let deadline = Instant::now() + Duration::from_millis(60);
    assert_eq!(
        call_until(&peer.path, "REGISTER", Value::Null, deadline, &|| false),
        Err(Failure::Uncertain)
    );
    // Reusing this deadline for CLAIM admits no exchange and no extra budget.
    assert_eq!(
        call_until(&peer.path, "CLAIM", Value::Null, deadline, &|| false),
        Err(Failure::Unavailable)
    );
    tx.send(()).unwrap();
}
#[test]
fn live_cancellation_is_observed_while_waiting_and_at_expired_admission() {
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
fn transient_database_contention_retries_past_three_replies() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let peer = Peer::new(7, move |stream, request| {
        let number = observed.fetch_add(1, Ordering::SeqCst);
        let response = if number < 6 {
            Response::error(request.request_id, crate::ERROR_DB_BUSY)
        } else {
            Response::ok(request.request_id, Value::Bool(true))
        };
        crate::write_response(stream, &response).unwrap();
    });
    assert_eq!(peer.call(), Ok(Value::Bool(true)));
    assert_eq!(calls.load(Ordering::SeqCst), 7);
}

#[test]
fn per_row_busy_reply_leaves_the_next_exchange_available() {
    let calls = Arc::new(AtomicUsize::new(0));
    let observed = calls.clone();
    let peer = Peer::new(2, move |stream, request| {
        let response = if observed.fetch_add(1, Ordering::SeqCst) == 0 {
            Response::error(request.request_id, crate::ERROR_DB_BUSY)
        } else {
            Response::ok(request.request_id, Value::Bool(true))
        };
        crate::write_response(stream, &response).unwrap();
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    assert_eq!(
        call_until_no_busy_retry(&peer.path, "fixture", Value::Null, deadline, &|| false),
        Err(Failure::Busy)
    );
    assert_eq!(
        call_until(&peer.path, "fixture", Value::Null, deadline, &|| false),
        Ok(Value::Bool(true))
    );
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}

//! Real-daemon fault proxy: successful backend replies are commit barriers.
use super::runtime::Fixture;
use arc_ipc::{Request, Response};
use std::{
    collections::BTreeMap,
    io::{self, Read, Write},
    os::unix::net::{UnixListener, UnixStream},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};
pub const MODES: [&str; 13] = [
    "absent",
    "refused",
    "disconnect-before-read",
    "disconnect-register",
    "stall-register",
    "partial-register",
    "committed-register",
    "disconnect-claim",
    "stall-claim",
    "committed-claim",
    "shared-budget",
    "busy-register",
    "busy-claim",
];
#[derive(Default)]
pub struct FaultReport {
    pub counts: BTreeMap<String, usize>,
    pub events: Vec<serde_json::Value>,
}
fn event(events: &mut Vec<serde_json::Value>, origin: Instant, action: &str, kind: &str) {
    events.push(serde_json::json!({"order":events.len(),"relativeNs":origin.elapsed().as_nanos() as u64,
        "operation":action,"event":kind,"sendObservation":if kind == "accepted-before-read" { "prefix-peeked" } else { "request-body-read" },
        "metadataDeadline":null,"deadlineReason":"CLI absolute deadline is not visible to the proxy; shared-deadline unit and native budget tests verify it"}));
}

// Darwin can reject SO_RCVTIMEO on an accepted socket after its peer closes.
// A nonblocking reader retains the same two-second bound without discarding a
// request that the client sent before disconnecting.
struct BoundedReader<'a> {
    stream: &'a mut UnixStream,
    deadline: Instant,
}
impl Read for BoundedReader<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        loop {
            match self.stream.read(buffer) {
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    if Instant::now() >= self.deadline {
                        return Err(io::ErrorKind::TimedOut.into());
                    }
                    thread::sleep(Duration::from_millis(1));
                }
                result => return result,
            }
        }
    }
}
pub struct Fault {
    pub path: PathBuf,
    stop: Arc<AtomicBool>,
    reached: Arc<AtomicBool>,
    join: Option<thread::JoinHandle<FaultReport>>,
}
impl Fault {
    pub fn new(f: &Fixture, mode: &str) -> Self {
        let path = f.root.join("fault.sock");
        let stop = Arc::new(AtomicBool::new(false));
        let reached = Arc::new(AtomicBool::new(false));
        if mode == "absent" {
            return Self {
                path,
                stop,
                reached,
                join: None,
            };
        }
        let listener = UnixListener::bind(&path).unwrap();
        if mode == "refused" {
            return Self {
                path,
                stop,
                reached,
                join: None,
            };
        }
        listener.set_nonblocking(true).unwrap();
        let backend = f.socket.clone();
        let mode = mode.to_string();
        let stopped = stop.clone();
        let admitted = reached.clone();
        let join = thread::spawn(move || {
            let end = Instant::now() + Duration::from_secs(30);
            let origin = Instant::now();
            let mut events = Vec::new();
            let mut counts = BTreeMap::new();
            let mut held = Vec::new();
            'accept: while !stopped.load(Ordering::SeqCst) {
                assert!(Instant::now() < end, "fault proxy deadline");
                let (mut stream, _) = match listener.accept() {
                    Ok(s) => s,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(1));
                        continue;
                    }
                    Err(e) => panic!("{e}"),
                };
                stream.set_nonblocking(false).unwrap();
                let fallback = match stream.set_read_timeout(Some(Duration::from_secs(2))) {
                    Ok(()) => false,
                    Err(error) if error.raw_os_error() == Some(libc::EINVAL) => {
                        stream.set_nonblocking(true).unwrap();
                        true
                    }
                    Err(error) => panic!("read timeout: {error}"),
                };
                if mode == "disconnect-before-read" {
                    // Observe a prefix without consuming the request. This
                    // barrier proves a write happened before disconnection.
                    let mut prefix = [0u8; 1];
                    let deadline = Instant::now() + Duration::from_secs(2);
                    loop {
                        let size = unsafe {
                            libc::recv(
                                std::os::fd::AsRawFd::as_raw_fd(&stream),
                                prefix.as_mut_ptr().cast(),
                                1,
                                libc::MSG_PEEK,
                            )
                        };
                        if size == 1 {
                            break;
                        }
                        let error = io::Error::last_os_error();
                        if fallback && (size == 0 || error.kind() == io::ErrorKind::ConnectionReset)
                        {
                            event(&mut events, origin, "unknown", "disconnected-before-prefix");
                            continue 'accept;
                        }
                        assert!(
                            fallback
                                && size < 0
                                && error.kind() == io::ErrorKind::WouldBlock
                                && Instant::now() < deadline,
                            "prefix peek: size {size}, {error}"
                        );
                        thread::sleep(Duration::from_millis(1));
                    }
                    event(&mut events, origin, "unknown", "accepted-before-read");
                    continue;
                }
                let read = if fallback {
                    arc_ipc::read_message(&mut BoundedReader {
                        stream: &mut stream,
                        deadline: Instant::now() + Duration::from_secs(2),
                    })
                } else {
                    arc_ipc::read_message(&mut stream)
                };
                let r: Request = match read {
                    Ok(request) => request,
                    Err(arc_ipc::IpcError::Frame(
                        arc_ipc::FrameError::Closed | arc_ipc::FrameError::UnexpectedEof,
                    )) if fallback => {
                        event(&mut events, origin, "unknown", "disconnected-before-frame");
                        continue;
                    }
                    Err(error) => panic!("fault request: {error}"),
                };
                if fallback {
                    stream.set_nonblocking(false).unwrap();
                }
                let action = r.payload["action"]
                    .as_str()
                    .unwrap_or(&r.operation)
                    .to_string();
                if action
                    == if mode.contains("claim") {
                        "CLAIM"
                    } else {
                        "REGISTER"
                    }
                {
                    admitted.store(true, Ordering::SeqCst);
                }
                event(&mut events, origin, &action, "request-read");
                let n = counts.entry(action.clone()).or_insert(0);
                *n += 1;
                if mode == "disconnect-register" && action == "REGISTER"
                    || mode == "disconnect-claim" && action == "CLAIM"
                {
                    event(&mut events, origin, &action, "disconnected-before-commit");
                    continue;
                }
                if mode == "stall-register" && action == "REGISTER"
                    || mode == "stall-claim" && action == "CLAIM"
                    || mode == "shared-budget"
                        && (action == "CLAIM" || action == "REGISTER" && *n < 3)
                {
                    event(&mut events, origin, &action, "stalled-before-commit");
                    held.push(stream);
                    continue;
                }
                if mode == "partial-register" {
                    event(&mut events, origin, &action, "partial-reply-withheld");
                    stream.write_all(&100u32.to_be_bytes()).unwrap();
                    stream.write_all(b"{").unwrap();
                    held.push(stream);
                    continue;
                }
                if mode.starts_with("busy-")
                    && action
                        == if mode == "busy-register" {
                            "REGISTER"
                        } else {
                            "CLAIM"
                        }
                {
                    event(&mut events, origin, &action, "injected-db-busy");
                    let _ = arc_ipc::write_response(
                        &mut stream,
                        &Response::error(r.request_id, arc_ipc::ERROR_DB_BUSY),
                    );
                    continue;
                }
                if mode.starts_with("reject-")
                    && action
                        == if mode == "reject-register" {
                            "REGISTER"
                        } else {
                            "CLAIM"
                        }
                {
                    let _ = arc_ipc::write_response(
                        &mut stream,
                        &Response::error(r.request_id, "INVALID_REQUEST"),
                    );
                    continue;
                }
                if mode.starts_with("invalid-")
                    && action
                        == if mode == "invalid-register" {
                            "REGISTER"
                        } else {
                            "CLAIM"
                        }
                {
                    let _ = arc_ipc::write_response(
                        &mut stream,
                        &Response::ok(r.request_id, serde_json::json!({"secret":"peer-secret"})),
                    );
                    continue;
                }
                let mut target = UnixStream::connect(&backend).unwrap();
                target
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let response = arc_ipc::exchange(&mut target, &r).unwrap();
                assert_eq!(response.status, "ok", "real commit barrier");
                if mode == "committed-register" && action == "REGISTER"
                    || mode == "committed-claim" && action == "CLAIM"
                {
                    event(&mut events, origin, &action, "backend-ack-withheld");
                    *counts.entry(format!("{action}-committed")).or_insert(0) += 1;
                    continue;
                }
                let forwarded = arc_ipc::write_response(&mut stream, &response).is_ok();
                event(
                    &mut events,
                    origin,
                    &action,
                    if forwarded {
                        "ack-forwarded"
                    } else {
                        "ack-write-failed"
                    },
                );
            }
            FaultReport { counts, events }
        });
        Self {
            path,
            stop,
            reached,
            join: Some(join),
        }
    }
    pub fn wait_for_request(&self) {
        let end = Instant::now() + Duration::from_secs(5);
        while !self.reached.load(Ordering::SeqCst) {
            assert!(Instant::now() < end, "metadata admission barrier");
            thread::sleep(Duration::from_millis(1));
        }
    }
    pub fn finish(mut self) -> FaultReport {
        self.stop.store(true, Ordering::SeqCst);
        self.join
            .take()
            .map(|j| j.join().unwrap())
            .unwrap_or_default()
    }
}
impl Drop for Fault {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(j) = self.join.take() {
            j.join().unwrap();
        }
    }
}

#[test]
fn nonblocking_timeout_fallback_preserves_framed_requests_and_deadlines() {
    let (mut read, mut write) = UnixStream::pair().unwrap();
    read.set_nonblocking(true).unwrap();
    let sender = thread::spawn(move || {
        thread::sleep(Duration::from_millis(10));
        arc_ipc::write_message(&mut write, &Request::new("fixture", "job.start")).unwrap();
        thread::sleep(Duration::from_millis(50));
    });
    let request: Request = arc_ipc::read_message(&mut BoundedReader {
        stream: &mut read,
        deadline: Instant::now() + Duration::from_secs(1),
    })
    .unwrap();
    assert_eq!(request.request_id, "fixture");
    let error = BoundedReader {
        stream: &mut read,
        deadline: Instant::now() + Duration::from_millis(10),
    }
    .read(&mut [0u8; 1])
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    sender.join().unwrap();
}

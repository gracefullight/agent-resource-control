//! Owned fault listener and per-attempt EOF witnesses for native bridge tests.
use super::{binary, native, Fault};
use arc_ipc::{
    jobs::{JobSnapshot, StatusResult, OPERATION_JOB_STATUS},
    Request, Response,
};
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    os::unix::{
        fs::PermissionsExt,
        net::{UnixListener, UnixStream},
    },
    path::Path,
    thread,
    time::{Duration, Instant},
};

#[derive(Default)]
struct FaultConnections {
    cancelled: bool,
    streams: Vec<UnixStream>,
}

/// Completion is sent only after the client returns. A first EOF is an attempt
/// boundary, not permission to remove the listener used by transport retries.
pub struct FaultServer {
    completion: std::sync::mpsc::Sender<()>,
    connections: std::sync::Arc<std::sync::Mutex<FaultConnections>>,
    worker: Option<thread::JoinHandle<Result<Vec<Value>, String>>>,
}
impl FaultServer {
    pub(super) fn start(socket: &Path, fault: Fault, snapshot: Option<JobSnapshot>) -> Self {
        use std::sync::{mpsc, Arc, Mutex};
        let listener = UnixListener::bind(socket).unwrap();
        fs::set_permissions(socket, fs::Permissions::from_mode(0o600)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let (completion, returned) = mpsc::channel();
        let connections = Arc::new(Mutex::new(FaultConnections::default()));
        let owned = connections.clone();
        let worker = thread::spawn(move || {
            // Scoped workers are joined even if accepting or another worker
            // fails. The owner shuts every registered socket before unwinding.
            thread::scope(|scope| {
                let mut workers = Vec::new();
                let accepting = (|| -> Result<(), String> {
                    let end = Instant::now() + Duration::from_secs(60);
                    let mut complete = false;
                    loop {
                        let mut registry = owned.lock().unwrap();
                        if registry.cancelled {
                            break;
                        }
                        match listener.accept() {
                            Ok((stream, _)) => {
                                registry
                                    .streams
                                    .push(stream.try_clone().map_err(|e| e.to_string())?);
                                let snapshot = snapshot.clone();
                                workers.push(
                                    scope.spawn(move || fault_connection(stream, fault, snapshot)),
                                );
                            }
                            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                // Drain queued attempts before honoring completion.
                                if complete {
                                    break;
                                }
                                drop(registry);
                                match returned.recv_timeout(Duration::from_millis(1)) {
                                    Ok(()) => complete = true,
                                    Err(mpsc::RecvTimeoutError::Timeout) => (),
                                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                                }
                            }
                            Err(e) => return Err(e.to_string()),
                        }
                        if Instant::now() >= end {
                            return Err("fault server completion watchdog".into());
                        }
                    }
                    Ok(())
                })();
                if accepting.is_err() {
                    Self::cancel_connections(&owned);
                }
                let mut records = Vec::new();
                let mut failure = accepting.err();
                for worker in workers {
                    match worker.join() {
                        Ok(Ok(record)) => records.push(record),
                        Ok(Err(error)) => {
                            failure.get_or_insert(error);
                        }
                        Err(_) => {
                            failure.get_or_insert("fault connection panicked".into());
                        }
                    }
                }
                failure.map_or(Ok(records), Err)
            })
        });
        Self {
            completion,
            connections,
            worker: Some(worker),
        }
    }

    pub fn finish(mut self) -> Value {
        self.completion.send(()).unwrap();
        let records = self.worker.take().unwrap().join().unwrap().unwrap();
        assert!(!records.is_empty(), "client sent no request");
        for record in &records {
            assert_eq!(
                record["request"], records[0]["request"],
                "retry changed identity/query"
            );
            assert_eq!(record["observedEof"], true);
        }
        json!({"fault": records[0]["fault"], "acceptedRequests": records.len(),
            "observedEofs": records.len(), "attempts": records})
    }

    fn cancel_connections(connections: &std::sync::Mutex<FaultConnections>) {
        let mut registry = connections.lock().unwrap();
        registry.cancelled = true;
        for stream in &registry.streams {
            let _ = stream.shutdown(std::net::Shutdown::Both);
        }
    }
}
impl Drop for FaultServer {
    fn drop(&mut self) {
        Self::cancel_connections(&self.connections);
        // Shutdown unblocks even an incomplete request. Do not detach any worker
        // or let a fixture directory disappear while a server still uses it.
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        self.connections.lock().unwrap().streams.clear();
    }
}

// Accepted macOS sockets inherit O_NONBLOCK; SO_RCVTIMEO can also fail after a
// fast peer disconnect. One fixture watchdog covers framing, writes and EOF.
struct FaultStream(UnixStream, Instant);
impl FaultStream {
    fn io<T>(
        &mut self,
        mut operation: impl FnMut(&mut UnixStream) -> std::io::Result<T>,
    ) -> std::io::Result<T> {
        loop {
            match operation(&mut self.0) {
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= self.1 {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::TimedOut,
                            "fault peer watchdog",
                        ));
                    }
                    thread::sleep(Duration::from_millis(1));
                }
                result => return result,
            }
        }
    }
}
impl std::io::Read for FaultStream {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        self.io(|stream| std::io::Read::read(stream, buffer))
    }
}
impl Write for FaultStream {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.io(|stream| stream.write(buffer))
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.io(Write::flush)
    }
}
fn fault_connection(
    stream: UnixStream,
    fault: Fault,
    snapshot: Option<JobSnapshot>,
) -> Result<Value, String> {
    use std::io::Read;
    let run = (|| -> Result<Value, Box<dyn std::error::Error>> {
        stream.set_nonblocking(true)?;
        let mut stream = FaultStream(stream, Instant::now() + Duration::from_secs(60));
        let request = Request::decode(&arc_ipc::read_frame(&mut stream)?)?;
        if request.protocol_version != 1
            || request.operation != OPERATION_JOB_STATUS
            || request.payload["query"] != "BY_ID"
        {
            return Err("unexpected fault request".into());
        }
        match fault {
            Fault::Json => arc_ipc::write_frame(&mut stream, b"{broken-json")?,
            Fault::Oversized => stream.write_all(&(arc_ipc::MAX_FRAME_BYTES + 1).to_be_bytes())?,
            Fault::Shape => arc_ipc::write_response(
                &mut stream,
                &Response::ok(&request.request_id, json!({"kind":"FOUND","job":[]})),
            )?,
            Fault::Correlation => arc_ipc::write_response(
                &mut stream,
                &Response::ok("wrong-correlation", json!({"kind":"NOT_FOUND"})),
            )?,
            Fault::Protocol => {
                let mut reply = Response::ok(&request.request_id, json!({"kind":"NOT_FOUND"}));
                reply.protocol_version = 99;
                arc_ipc::write_response(&mut stream, &reply)?;
            }
            Fault::Selection => arc_ipc::write_response(
                &mut stream,
                &Response::ok(
                    &request.request_id,
                    json!(StatusResult::Found {
                        job: Box::new(snapshot.ok_or("missing selection snapshot")?)
                    }),
                ),
            )?,
            Fault::Stall => (),
        }
        // Real peer EOF, including every stalled attempt; completion never
        // manufactures this observation by shutting sockets on the normal path.
        loop {
            match stream.read(&mut [0u8]) {
                Ok(0) => break,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                result => return Err(format!("expected client EOF: {result:?}").into()),
            }
        }
        Ok(json!({"request": request, "fault": fault.name(), "observedEof": true}))
    })();
    run.map_err(|e| e.to_string())
}

#[cfg(test)]
mod fault_lifecycle_tests {
    use super::*;

    #[test]
    fn stalled_listener_survives_attempt_eofs_until_explicit_completion() {
        let fixture = native::Fixture::with_binary(binary("arc-daemon"));
        let socket = fixture.root.join("retry.sock");
        let server = FaultServer::start(&socket, Fault::Stall, None);
        let mut request = Request::new("same-request", OPERATION_JOB_STATUS);
        request.payload = json!({"query": "BY_ID", "job_id": "a".repeat(32)});
        for _ in 0..3 {
            let mut stream = UnixStream::connect(&socket).unwrap();
            arc_ipc::write_message(&mut stream, &request).unwrap();
            stream.shutdown(std::net::Shutdown::Write).unwrap();
        }
        let report = server.finish();
        assert_eq!(report["acceptedRequests"], 3);
        assert_eq!(report["observedEofs"], 3);
        assert!(fixture.root.exists());
    }

    #[test]
    fn fault_server_unwind_closes_incomplete_request_and_joins_workers() {
        use std::io::Read;
        let fixture = native::Fixture::with_binary(binary("arc-daemon"));
        let socket = fixture.root.join("unwind.sock");
        let server = FaultServer::start(&socket, Fault::Stall, None);
        let joined = server.connections.clone();
        let mut peer = UnixStream::connect(&socket).unwrap();
        let end = Instant::now() + Duration::from_secs(60);
        while joined.lock().unwrap().streams.is_empty() {
            assert!(Instant::now() < end, "fault connection not accepted");
            thread::yield_now();
        }
        let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let _owner = server;
            panic!("exercise server unwind before a complete request");
        }));
        assert!(failure.is_err());
        assert!(joined.lock().unwrap().streams.is_empty());
        assert_eq!(
            std::sync::Arc::strong_count(&joined),
            1,
            "server still running"
        );
        assert_eq!(peer.read(&mut [0u8]).unwrap(), 0);
        assert!(fixture.root.exists());
    }
}

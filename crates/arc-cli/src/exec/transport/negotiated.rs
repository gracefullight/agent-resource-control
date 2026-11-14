//! New negotiated vocabulary over the maintained authenticated connection.
//! Session's release-only exchange treats all frame failures as invalid. An
//! admission instead distinguishes a lost ACK from malformed data, retaining
//! the original attempt without replaying a release or changing shared codecs.
use super::*;
use arc_ipc::{client::Session, FrameError, IpcError, PROTOCOL_VERSION, STATUS_ERROR, STATUS_OK};
use std::{
    io::{self, Read, Write},
    os::{
        fd::{AsRawFd, BorrowedFd},
        unix::net::UnixStream,
    },
};

pub(crate) fn negotiated(
    socket: &Path,
    operation: &str,
    payload: Value,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
    possibly_committed: &mut bool,
    continuity: Option<&Continuity>,
) -> Result<Value, Failure> {
    let mut request = Request::new(crate::request_id(), operation);
    request.payload = payload;
    let mut attempts = 0;
    loop {
        if cancelled() {
            return Err(Failure::Cancelled);
        }
        if Instant::now() >= deadline {
            return Err(if *possibly_committed {
                Failure::Uncertain
            } else {
                Failure::Unavailable
            });
        }
        attempts += 1;
        let attempt_deadline = deadline.min(Instant::now() + Duration::from_millis(600));
        let session = match Session::connect_until(socket, attempt_deadline, cancelled, continuity)
        {
            Ok(session) => session,
            Err(Failure::Unavailable) if attempts < 3 => continue,
            Err(error) => {
                return Err(
                    if *possibly_committed
                        && matches!(error, Failure::Unavailable | Failure::InvalidEndpoint)
                    {
                        Failure::Uncertain
                    } else {
                        error
                    },
                )
            }
        };
        // Duplicate only this client's authenticated socket. The descriptor remains
        // nonblocking and CLOEXEC; no inherited stdio flags or process state change.
        let descriptor = unsafe { BorrowedFd::borrow_raw(session.raw_fd()) }
            .try_clone_to_owned()
            .map_err(|_| Failure::Local)?;
        let mut io = DeadlineIo {
            stream: descriptor.into(),
            deadline: attempt_deadline,
            cancelled,
            sent: false,
        };
        let response = arc_ipc::exchange(&mut io, &request);
        if cancelled() {
            *possibly_committed |= io.sent;
            return Err(Failure::Cancelled);
        }
        let response = match response {
            Ok(response) => response,
            Err(
                IpcError::Json(_)
                | IpcError::NotAnObject
                | IpcError::Frame(FrameError::EmptyFrame | FrameError::TooLarge { .. }),
            ) => {
                *possibly_committed |= io.sent;
                return Err(Failure::InvalidReply);
            }
            Err(_) => {
                *possibly_committed |= io.sent;
                if attempts < 3 && Instant::now() < deadline {
                    continue;
                }
                return Err(if *possibly_committed {
                    Failure::Uncertain
                } else {
                    Failure::Unavailable
                });
            }
        };
        if response.protocol_version != PROTOCOL_VERSION {
            *possibly_committed |= io.sent;
            return Err(Failure::Protocol);
        }
        if response.request_id != request.request_id {
            *possibly_committed |= io.sent;
            return Err(Failure::InvalidReply);
        }
        return match (response.status.as_str(), response.result, response.error) {
            (STATUS_OK, Some(value), None) => {
                *possibly_committed = true;
                Ok(value)
            }
            (STATUS_ERROR, None, Some(error)) => Err(match error.code.as_str() {
                arc_ipc::ERROR_UNSUPPORTED_CAPABILITY => {
                    if *possibly_committed || continuity.is_some_and(|c| !c.confirmed()) {
                        Failure::Uncertain
                    } else {
                        Failure::Unsupported
                    }
                }
                arc_ipc::ERROR_DB_BUSY => Failure::Busy,
                arc_ipc::ERROR_PROTOCOL_INCOMPATIBLE => Failure::Protocol,
                "" => {
                    *possibly_committed |= io.sent;
                    Failure::InvalidReply
                }
                _ => Failure::Rejected,
            }),
            _ => {
                *possibly_committed |= io.sent;
                Err(Failure::InvalidReply)
            }
        };
    }
}

struct DeadlineIo<'a> {
    stream: UnixStream,
    deadline: Instant,
    cancelled: &'a dyn Fn() -> bool,
    sent: bool,
}
impl DeadlineIo<'_> {
    fn ready(&self, events: i16) -> io::Result<()> {
        loop {
            if (self.cancelled)() {
                return Err(io::ErrorKind::ConnectionAborted.into());
            }
            let remaining = self.deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::ErrorKind::TimedOut.into());
            }
            let mut fd = libc::pollfd {
                fd: self.stream.as_raw_fd(),
                events,
                revents: 0,
            };
            let timeout = remaining.as_millis().clamp(1, 20) as i32;
            let result = unsafe { libc::poll(&mut fd, 1, timeout) };
            if result > 0 {
                return Ok(());
            }
            if result < 0 && io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
                return Err(io::Error::last_os_error());
            }
        }
    }
}
impl Read for DeadlineIo<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        loop {
            self.ready(libc::POLLIN)?;
            match self.stream.read(bytes) {
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) => {}
                result => return result,
            }
        }
    }
}
impl Write for DeadlineIo<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        loop {
            self.ready(libc::POLLOUT)?;
            match self.stream.write(bytes) {
                Ok(n) => {
                    self.sent |= n > 0;
                    return Ok(n);
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) => {}
                result => return result,
            }
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        os::unix::net::UnixListener,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        thread,
    };

    #[test]
    fn lost_or_malformed_ack_and_negative_evidence_remain_distinct_for_the_same_attempt() {
        for mode in [
            "unsupported",
            "previously-sent",
            "ambiguous-peer",
            "lost",
            "partial",
            "invalid-json",
            "wrong-id",
            "future-protocol",
            "busy",
            "cancelled",
        ] {
            let root = std::env::temp_dir().join(format!(
                "arc-script-peer-{}",
                arc_core::process_identity::random_job_id().unwrap()
            ));
            fs::create_dir(&root).unwrap();
            let path = root.join("s");
            let listener = UnixListener::bind(&path).unwrap();
            let cancelled = Arc::new(AtomicBool::new(false));
            let server_cancelled = cancelled.clone();
            let server = thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                let request: Request = arc_ipc::read_message(&mut stream).unwrap();
                assert_eq!(
                    request.operation,
                    arc_ipc::traffic::OPERATION_BUN_SCRIPTS_JOB
                );
                assert_eq!(request.payload["request"]["job_id"], "a".repeat(32));
                match mode {
                    "lost" => (),
                    "partial" => {
                        stream.write_all(&100u32.to_be_bytes()).unwrap();
                        stream.write_all(b"{").unwrap();
                    }
                    "invalid-json" => arc_ipc::write_frame(&mut stream, b"{").unwrap(),
                    "cancelled" => server_cancelled.store(true, Ordering::SeqCst),
                    _ => {
                        let mut response = arc_ipc::Response::error(
                            &request.request_id,
                            if mode == "busy" {
                                arc_ipc::ERROR_DB_BUSY
                            } else {
                                arc_ipc::ERROR_UNSUPPORTED_CAPABILITY
                            },
                        );
                        if mode == "wrong-id" {
                            response.request_id = "not-the-request".into();
                        }
                        if mode == "future-protocol" {
                            response.protocol_version = 2;
                        }
                        arc_ipc::write_response(&mut stream, &response).unwrap();
                    }
                }
            });
            let continuity = Continuity::default();
            if mode == "ambiguous-peer" {
                continuity.record(None);
            }
            let mut committed = mode == "previously-sent";
            let result = negotiated(
                &path,
                arc_ipc::traffic::OPERATION_BUN_SCRIPTS_JOB,
                serde_json::json!({"action":"TRAFFIC","request":{"action":"POLL","job_id":"a".repeat(32)}}),
                Instant::now() + Duration::from_secs(2),
                &|| cancelled.load(Ordering::SeqCst),
                &mut committed,
                Some(&continuity),
            );
            let expected = match mode {
                "unsupported" => Failure::Unsupported,
                "previously-sent" | "ambiguous-peer" | "lost" | "partial" => Failure::Uncertain,
                "invalid-json" | "wrong-id" => Failure::InvalidReply,
                "future-protocol" => Failure::Protocol,
                "busy" => Failure::Busy,
                "cancelled" => Failure::Cancelled,
                _ => unreachable!(),
            };
            assert_eq!(result, Err(expected), "{mode}");
            if matches!(
                mode,
                "previously-sent"
                    | "lost"
                    | "partial"
                    | "invalid-json"
                    | "wrong-id"
                    | "future-protocol"
                    | "cancelled"
            ) {
                assert!(committed, "{mode}");
            }
            server.join().unwrap();
            fs::remove_dir_all(root).unwrap();
        }
    }
}

//! Bounded exchanges; retries only record metadata and never launch a child.
use crate::{Request, PROTOCOL_VERSION, STATUS_OK};
use serde_json::Value;
use std::{
    io::{self, Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{ffi::OsStrExt, net::UnixStream},
    },
    path::Path,
    time::{Duration, Instant},
};

mod exchange_types;
pub use exchange_types::{ExchangeOptions, Failure};
mod endpoint;
use endpoint::{endpoint_failure, local_failure, ConnectFailure};

trait TrackedStream: Read + Write {
    fn sent(&self) -> bool;
    fn peer_identity(&self) -> Option<arc_core::process_identity::ProcessIdentity> {
        None
    }
}

mod continuity;
pub use continuity::Continuity;

/// Exchange an already constructed protocol-1 request. All retries retain the same
/// request identity, absolute deadline, commit tracking and peer authentication.
pub fn exchange_until(
    socket: &Path,
    request: &Request,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
    possibly_committed: &mut bool,
    options: ExchangeOptions<'_>,
) -> Result<Value, Failure> {
    exchange_with(
        request,
        deadline,
        cancelled,
        possibly_committed,
        options,
        |end| DeadlineStream::connect(socket, end, cancelled),
    )
}

fn exchange_with<S: TrackedStream>(
    request: &Request,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
    possibly_committed: &mut bool,
    options: ExchangeOptions<'_>,
    mut connect: impl FnMut(Instant) -> Result<S, ConnectFailure>,
) -> Result<Value, Failure> {
    let operation = request.operation.as_str();
    let mut uncertain = *possibly_committed;
    let mut busy = false;
    let mut attempts = 0;
    loop {
        if cancelled() {
            return Err(Failure::Cancelled);
        }
        if Instant::now() >= deadline {
            break;
        }
        attempts += 1;
        let attempt_deadline = deadline.min(Instant::now() + Duration::from_millis(600));
        let mut stream = match connect(attempt_deadline) {
            Ok(stream) => stream,
            Err(error) => {
                if cancelled() {
                    return Err(Failure::Cancelled);
                }
                match error {
                    ConnectFailure::Local => return Err(Failure::Local),
                    ConnectFailure::InvalidEndpoint => {
                        return Err(if uncertain {
                            Failure::Uncertain
                        } else {
                            Failure::InvalidEndpoint
                        });
                    }
                    ConnectFailure::Endpoint => (),
                }
                if attempts >= 3 {
                    break;
                }
                continue;
            }
        };
        let response = crate::exchange(&mut stream, request);
        if stream.sent() {
            if let Some(continuity) = options.continuity {
                continuity.record(stream.peer_identity());
            }
        }
        if cancelled() {
            *possibly_committed |= stream.sent();
            return Err(Failure::Cancelled);
        }
        match response {
            Ok(response) => {
                if response.protocol_version != PROTOCOL_VERSION {
                    *possibly_committed |= stream.sent();
                    return Err(Failure::Protocol);
                }
                if response.request_id != request.request_id {
                    *possibly_committed |= stream.sent();
                    return Err(Failure::InvalidReply);
                }
                match (response.status.as_str(), response.result, response.error) {
                    (STATUS_OK, Some(value), None) => return Ok(value),
                    (crate::STATUS_ERROR, None, Some(e)) if e.code == crate::ERROR_DB_BUSY => {
                        if !options.retry_busy {
                            return Err(Failure::Busy);
                        }
                        uncertain = true;
                        *possibly_committed = true;
                        busy = true;
                        // A responsive but saturated worker is not an outage.
                        // Back off and retry the same request within the shared
                        // deadline rather than failing open after three replies.
                        let remaining = deadline.saturating_duration_since(Instant::now());
                        std::thread::sleep(remaining.min(Duration::from_millis(20)));
                        continue;
                    }
                    (crate::STATUS_ERROR, None, Some(e))
                        if matches!(
                            operation,
                            crate::policy::OPERATION_POLICY_CAPABILITIES
                                | crate::policy::OPERATION_POLICY_CURRENT
                                | crate::policy::OPERATION_POLICY_JOB
                                | crate::traffic::OPERATION_TRAFFIC_CAPABILITIES
                                | crate::traffic::OPERATION_BUN_CAPABILITIES
                                | crate::meter::OPERATION_METER_SNAPSHOT
                                | crate::meter::OPERATION_METER_REPORT
                                | crate::meter::OPERATION_METER_SUMMARY
                        ) && e.code == crate::ERROR_UNSUPPORTED_CAPABILITY =>
                    {
                        return Err(Failure::Unsupported)
                    }
                    (crate::STATUS_ERROR, None, Some(e))
                        if matches!(
                            operation,
                            crate::policy::OPERATION_POLICY_CAPABILITIES
                                | crate::policy::OPERATION_POLICY_CURRENT
                                | crate::policy::OPERATION_POLICY_JOB
                        ) && e.code == crate::policy::ERROR_POLICY_UNAVAILABLE =>
                    {
                        return Err(Failure::Unavailable)
                    }
                    (crate::STATUS_ERROR, None, Some(e))
                        if matches!(
                            operation,
                            crate::meter::OPERATION_METER_SNAPSHOT
                                | crate::meter::OPERATION_METER_REPORT
                                | crate::meter::OPERATION_METER_SUMMARY
                        ) && e.code == crate::meter::ERROR_METER_UNAVAILABLE =>
                    {
                        return Err(Failure::Unavailable)
                    }
                    (crate::STATUS_ERROR, None, Some(e))
                        if matches!(
                            operation,
                            crate::traffic::OPERATION_BUN_JOB | crate::governor::OPERATION_JOB
                        ) && e.code == crate::ERROR_UNSUPPORTED_CAPABILITY =>
                    {
                        return Err(
                            if uncertain || options.continuity.is_some_and(|c| !c.confirmed()) {
                                Failure::Uncertain
                            } else {
                                Failure::Unsupported
                            },
                        );
                    }
                    (crate::STATUS_ERROR, None, Some(e))
                        if operation == crate::traffic::OPERATION_TRAFFIC_JOB
                            && request.payload.get("action").and_then(Value::as_str)
                                == Some("STATUS_V2")
                            && matches!(
                                e.code.as_str(),
                                crate::ERROR_INVALID_REQUEST | crate::ERROR_UNSUPPORTED_CAPABILITY
                            ) =>
                    {
                        return Err(Failure::Unsupported)
                    }
                    (crate::STATUS_ERROR, None, Some(e)) if !e.code.is_empty() => {
                        return Err(if e.code == crate::ERROR_PROTOCOL_INCOMPATIBLE {
                            Failure::Protocol
                        } else {
                            Failure::Rejected
                        });
                    }
                    _ => {
                        *possibly_committed |= stream.sent();
                        return Err(Failure::InvalidReply);
                    }
                }
            }
            Err(
                crate::IpcError::Json(_)
                | crate::IpcError::NotAnObject
                | crate::IpcError::Frame(
                    crate::FrameError::EmptyFrame | crate::FrameError::TooLarge { .. },
                ),
            ) => {
                *possibly_committed |= stream.sent();
                return Err(Failure::InvalidReply);
            }
            Err(_) => {
                // timeout, reset, EOF and partial disconnected frames
                uncertain |= stream.sent();
                *possibly_committed |= stream.sent();
            }
        }
        if attempts >= 3 {
            break;
        }
    }
    if cancelled() {
        return Err(Failure::Cancelled);
    }
    Err(if busy {
        Failure::Busy
    } else if uncertain {
        Failure::Uncertain
    } else {
        Failure::Unavailable
    })
}
mod session;
pub use session::Session;

struct DeadlineStream<'a> {
    stream: UnixStream,
    deadline: Instant,
    cancelled: &'a dyn Fn() -> bool,
    sent: bool,
    peer_identity: Option<arc_core::process_identity::ProcessIdentity>,
}
impl<'a> DeadlineStream<'a> {
    fn connect(
        path: &Path,
        deadline: Instant,
        cancelled: &'a dyn Fn() -> bool,
    ) -> Result<Self, ConnectFailure> {
        Self::connect_with_auth(path, deadline, cancelled, peer::ensure_same_user)
    }
    fn connect_with_auth(
        path: &Path,
        deadline: Instant,
        cancelled: &'a dyn Fn() -> bool,
        authenticate: impl FnOnce(&UnixStream) -> io::Result<()>,
    ) -> Result<Self, ConnectFailure> {
        let mut address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
        let bytes = path.as_os_str().as_bytes();
        if bytes.len() >= address.sun_path.len() || bytes.contains(&0) {
            return Err(ConnectFailure::InvalidEndpoint);
        }
        address.sun_family = libc::AF_UNIX as _;
        for (to, from) in address.sun_path.iter_mut().zip(bytes) {
            *to = *from as _;
        }
        let length = (std::mem::offset_of!(libc::sockaddr_un, sun_path) + bytes.len() + 1)
            as libc::socklen_t;
        #[cfg(target_os = "macos")]
        {
            address.sun_len = length as u8;
        }
        // SAFETY: adopt only a new live descriptor, before any fallible setup.
        let fd = unsafe { libc::socket(libc::AF_UNIX, libc::SOCK_STREAM, 0) };
        if fd < 0 {
            return Err(ConnectFailure::Local);
        }
        let stream = unsafe { UnixStream::from_raw_fd(fd) };
        stream.set_nonblocking(true).map_err(local_failure)?;
        if unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
            return Err(ConnectFailure::Local);
        }
        let connected =
            unsafe { libc::connect(fd, (&address as *const libc::sockaddr_un).cast(), length) };
        if connected < 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(libc::EINPROGRESS) {
                return Err(endpoint_failure(error));
            }
        }
        let result = Self {
            stream,
            deadline,
            cancelled,
            sent: false,
            peer_identity: None,
        };
        if connected < 0 {
            result.ready(libc::POLLOUT).map_err(|error| {
                if matches!(
                    error.kind(),
                    io::ErrorKind::TimedOut | io::ErrorKind::ConnectionAborted
                ) {
                    ConnectFailure::Endpoint
                } else {
                    ConnectFailure::Local
                }
            })?;
        }
        if let Some(error) = result.stream.take_error().map_err(local_failure)? {
            return Err(endpoint_failure(error));
        }
        result.stream.peer_addr().map_err(|error| {
            if error.kind() == io::ErrorKind::NotConnected {
                ConnectFailure::Endpoint
            } else {
                ConnectFailure::Local
            }
        })?;
        // The daemon also checks its accepted peer. Check the server before
        // sending any lifecycle bytes or accepting any response as definitive.
        authenticate(&result.stream).map_err(|_| ConnectFailure::Endpoint)?;
        let mut result = result;
        result.peer_identity = peer::identity(&result.stream).ok();
        Ok(result)
    }
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
            // SAFETY: poll receives one live descriptor and valid writable storage.
            let result =
                unsafe { libc::poll(&mut fd, 1, remaining.as_millis().clamp(1, 25) as i32) };
            if result > 0 {
                return Ok(());
            }
            if result < 0 && io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
                return Err(io::Error::last_os_error());
            }
        }
    }
}
impl TrackedStream for DeadlineStream<'_> {
    fn sent(&self) -> bool {
        self.sent
    }
    fn peer_identity(&self) -> Option<arc_core::process_identity::ProcessIdentity> {
        self.peer_identity.clone()
    }
}
impl Read for DeadlineStream<'_> {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        loop {
            self.ready(libc::POLLIN)?;
            match self.stream.read(bytes) {
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) => {}
                other => return other,
            }
        }
    }
}
impl Write for DeadlineStream<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        loop {
            self.ready(libc::POLLOUT)?;
            match self.stream.write(bytes) {
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) => {}
                other => {
                    if matches!(other, Ok(n) if n > 0) {
                        self.sent = true;
                    }
                    return other;
                }
            }
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

mod peer;
#[cfg(test)]
mod test_calls;
#[cfg(test)]
use test_calls::{call_until, call_until_no_busy_retry, call_until_tracked, call_with};
#[cfg(test)]
mod access_tests;
#[cfg(test)]
mod live_tests;
#[cfg(test)]
mod meter_tests;
#[cfg(test)]
mod repair_tests;

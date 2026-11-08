//! Bounded exchanges; retries only record metadata and never launch a child.
use arc_ipc::{Request, PROTOCOL_VERSION, STATUS_OK};
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

pub(crate) fn call(socket: &Path, operation: &str, payload: Value) -> Result<Value, ()> {
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut request = Request::new(super::super::request_id(), operation);
    request.payload = payload;
    for _ in 0..3 {
        if Instant::now() >= deadline {
            break;
        }
        let attempt_deadline = deadline.min(Instant::now() + Duration::from_millis(600));
        if let Ok(mut stream) = DeadlineStream::connect(socket, attempt_deadline) {
            if let Ok(response) = arc_ipc::exchange(&mut stream, &request) {
                if response.protocol_version != PROTOCOL_VERSION
                    || response.request_id != request.request_id
                {
                    return Err(());
                }
                if response.status == STATUS_OK && response.error.is_none() {
                    return response.result.ok_or(());
                }
                if response
                    .error
                    .as_ref()
                    .is_none_or(|e| e.code != arc_ipc::ERROR_DB_BUSY)
                {
                    return Err(());
                }
            }
        }
    }
    Err(())
}
struct DeadlineStream {
    stream: UnixStream,
    deadline: Instant,
}
impl DeadlineStream {
    fn connect(path: &Path, deadline: Instant) -> io::Result<Self> {
        let mut address: libc::sockaddr_un = unsafe { std::mem::zeroed() };
        let bytes = path.as_os_str().as_bytes();
        if bytes.len() >= address.sun_path.len() || bytes.contains(&0) {
            return Err(io::ErrorKind::InvalidInput.into());
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
            return Err(io::Error::last_os_error());
        }
        let stream = unsafe { UnixStream::from_raw_fd(fd) };
        stream.set_nonblocking(true)?;
        if unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
            return Err(io::Error::last_os_error());
        }
        let connected =
            unsafe { libc::connect(fd, (&address as *const libc::sockaddr_un).cast(), length) };
        if connected < 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(libc::EINPROGRESS) {
                return Err(error);
            }
        }
        let result = Self { stream, deadline };
        if connected < 0 {
            result.ready(libc::POLLOUT)?;
        }
        if let Some(error) = result.stream.take_error()? {
            return Err(error);
        }
        result.stream.peer_addr()?;
        Ok(result)
    }
    fn ready(&self, events: i16) -> io::Result<()> {
        loop {
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
            let result = unsafe { libc::poll(&mut fd, 1, remaining.as_millis().max(1) as i32) };
            if result > 0 {
                return Ok(());
            }
            if result < 0 && io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
                return Err(io::Error::last_os_error());
            }
        }
    }
}
impl Read for DeadlineStream {
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
impl Write for DeadlineStream {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        loop {
            self.ready(libc::POLLOUT)?;
            match self.stream.write(bytes) {
                Err(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                    ) => {}
                other => return other,
            }
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

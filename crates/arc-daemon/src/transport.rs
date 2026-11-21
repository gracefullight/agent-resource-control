//! Async protocol-1 I/O with deadlines fixed before any partial progress.

use arc_ipc::{FrameError, Response, MAX_FRAME_BYTES};
use std::{
    io::{self, Write},
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
};
use tokio::{
    net::UnixStream,
    time::{timeout_at, Instant},
};

pub(crate) async fn read_header(stream: &UnixStream, deadline: Instant) -> Result<u32, FrameError> {
    let (declared, relay) = read_header_with_peer(stream, deadline).await?;
    if relay.is_some() {
        return Err(
            io::Error::new(io::ErrorKind::InvalidData, "unexpected relay descriptor").into(),
        );
    }
    Ok(declared)
}

pub(crate) async fn read_header_with_peer(
    stream: &UnixStream,
    deadline: Instant,
) -> Result<(u32, Option<OwnedFd>), FrameError> {
    let mut header = [0; 4];
    let mut filled = 0;
    let mut relay = None;
    while filled < header.len() {
        check_deadline(deadline)?;
        timeout_at(deadline, stream.readable())
            .await
            .map_err(|_| timed_out())??;
        check_deadline(deadline)?;
        match stream.try_io(tokio::io::Interest::READABLE, || {
            receive_header(stream, &mut header[filled..])
        }) {
            Ok((0, _)) if filled == 0 => return Err(FrameError::Closed),
            Ok((0, _)) => return Err(FrameError::UnexpectedEof),
            Ok((n, received)) => {
                if relay.is_some() && received.is_some() {
                    return Err(
                        io::Error::new(io::ErrorKind::InvalidData, "multiple relay peers").into(),
                    );
                }
                relay = relay.or(received);
                filled += n;
            }
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) => {}
            Err(e) => return Err(e.into()),
        }
    }
    let declared = u32::from_be_bytes(header);
    validate_size(declared)?;
    Ok((declared, relay))
}

/// Receive an optional passed socket. Its credentials do not authenticate the
/// bytes on this connection; lifecycle relays require a separate request proof.
fn receive_header(stream: &UnixStream, bytes: &mut [u8]) -> io::Result<(usize, Option<OwnedFd>)> {
    let mut iov = libc::iovec {
        iov_base: bytes.as_mut_ptr().cast(),
        iov_len: bytes.len(),
    };
    // usize alignment is sufficient for cmsghdr; extra room lets us close all
    // delivered descriptors before rejecting an oversized ancillary message.
    let mut control = [0usize; 16];
    let mut message: libc::msghdr = unsafe { std::mem::zeroed() };
    message.msg_iov = &mut iov;
    message.msg_iovlen = 1;
    message.msg_control = control.as_mut_ptr().cast();
    message.msg_controllen = std::mem::size_of_val(&control) as _;
    let size = unsafe { libc::recvmsg(stream.as_raw_fd(), &mut message, 0) };
    if size < 0 {
        return Err(io::Error::last_os_error());
    }
    let mut received = Vec::new();
    let mut invalid = message.msg_flags & libc::MSG_CTRUNC != 0;
    let mut current = unsafe { libc::CMSG_FIRSTHDR(&message) };
    while !current.is_null() {
        let header = unsafe { &*current };
        let base = unsafe { libc::CMSG_LEN(0) } as usize;
        if header.cmsg_level != libc::SOL_SOCKET
            || header.cmsg_type != libc::SCM_RIGHTS
            || (header.cmsg_len as usize) < base
        {
            invalid = true;
        } else {
            let payload = header.cmsg_len as usize - base;
            if !payload.is_multiple_of(std::mem::size_of::<libc::c_int>()) {
                invalid = true;
            }
            let count = payload / std::mem::size_of::<libc::c_int>();
            let ptr = unsafe { libc::CMSG_DATA(current) }.cast::<libc::c_int>();
            for index in 0..count {
                let fd = unsafe { *ptr.add(index) };
                // SAFETY: SCM_RIGHTS transfers ownership of each received fd.
                let owned = unsafe { OwnedFd::from_raw_fd(fd) };
                if unsafe { libc::fcntl(owned.as_raw_fd(), libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
                    invalid = true;
                }
                received.push(owned);
            }
        }
        current = unsafe { libc::CMSG_NXTHDR(&message, current) };
    }
    if invalid || received.len() > 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid relay descriptor",
        ));
    }
    Ok((size as usize, received.pop()))
}

pub(crate) async fn read_body(
    stream: &UnixStream,
    declared: u32,
    deadline: Instant,
) -> Result<Vec<u8>, FrameError> {
    validate_size(declared)?;
    check_deadline(deadline)?;
    let mut body = vec![0; declared as usize];
    read_exact(stream, &mut body, deadline, false).await?;
    Ok(body)
}

fn validate_size(declared: u32) -> Result<(), FrameError> {
    match declared {
        0 => Err(FrameError::EmptyFrame),
        n if n > MAX_FRAME_BYTES => Err(FrameError::TooLarge { declared }),
        _ => Ok(()),
    }
}

async fn read_exact(
    stream: &UnixStream,
    buffer: &mut [u8],
    deadline: Instant,
    header: bool,
) -> Result<(), FrameError> {
    let mut filled = 0;
    while filled < buffer.len() {
        check_deadline(deadline)?;
        timeout_at(deadline, stream.readable())
            .await
            .map_err(|_| timed_out())??;
        check_deadline(deadline)?;
        match stream.try_read(&mut buffer[filled..]) {
            Ok(0) if header && filled == 0 => return Err(FrameError::Closed),
            Ok(0) => return Err(FrameError::UnexpectedEof),
            Ok(n) => filled += n,
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

pub(crate) async fn write_response(
    stream: &UnixStream,
    response: &Response,
    deadline: Instant,
) -> io::Result<()> {
    let mut writer = CappedWriter {
        body: Vec::new(),
        deadline,
        failure: None,
    };
    if serde_json::to_writer(&mut writer, response).is_err() {
        return Err(writer.failure.unwrap_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "response serialization failed")
        }));
    }
    check_deadline(deadline)?;
    write_all(stream, &(writer.body.len() as u32).to_be_bytes(), deadline).await?;
    write_all(stream, &writer.body, deadline).await
}

/// Ask the authenticated observer to confirm the exact forwarded request.
pub(crate) async fn confirm_request(
    stream: &UnixStream,
    request: &arc_ipc::Request,
    deadline: Instant,
) -> bool {
    let Ok(nonce) = arc_core::process_identity::random_job_id() else {
        return false;
    };
    let challenge =
        serde_json::json!({"kind":"arc.relay.challenge.v1", "nonce":nonce, "request":request});
    exchange_value(stream, &challenge, deadline).await.ok()
        == Some(serde_json::json!({"kind":"arc.relay.response.v1", "nonce":nonce}))
}

/// One framed JSON round trip with an already authenticated callback peer.
/// `Err(true)` means the request may have reached the peer.
pub(crate) async fn exchange_value(
    stream: &UnixStream,
    message: &serde_json::Value,
    deadline: Instant,
) -> Result<serde_json::Value, bool> {
    let body = serde_json::to_vec(message).map_err(|_| false)?;
    if body.is_empty() || body.len() > MAX_FRAME_BYTES as usize {
        return Err(false);
    }
    write_all(stream, &(body.len() as u32).to_be_bytes(), deadline)
        .await
        .map_err(|_| false)?;
    // A partial body write is indistinguishable from delivery to the caller.
    write_all(stream, &body, deadline).await.map_err(|_| true)?;
    let length = read_header(stream, deadline).await.map_err(|_| true)?;
    let body = read_body(stream, length, deadline)
        .await
        .map_err(|_| true)?;
    serde_json::from_slice(&body).map_err(|_| true)
}

async fn write_all(stream: &UnixStream, bytes: &[u8], deadline: Instant) -> io::Result<()> {
    let mut written = 0;
    while written < bytes.len() {
        check_deadline(deadline)?;
        timeout_at(deadline, stream.writable())
            .await
            .map_err(|_| timed_out())??;
        check_deadline(deadline)?;
        match stream.try_write(&bytes[written..]) {
            Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
            Ok(n) => written += n,
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

/// Exact checked growth bounds the Vec's retained capacity, not just its length.
struct CappedWriter {
    body: Vec<u8>,
    deadline: Instant,
    failure: Option<io::Error>,
}

impl Write for CappedWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let result = (|| {
            check_deadline(self.deadline)?;
            let required = self
                .body
                .len()
                .checked_add(bytes.len())
                .filter(|n| *n <= MAX_FRAME_BYTES as usize)
                .ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidData, "response exceeds frame cap")
                })?;
            if required > self.body.capacity() {
                // Geometric target avoids quadratic serialization while never reserving over the cap.
                let target = required.max(
                    self.body
                        .capacity()
                        .saturating_mul(2)
                        .min(MAX_FRAME_BYTES as usize),
                );
                self.body
                    .try_reserve_exact(target - self.body.len())
                    .map_err(|_| io::Error::other("response allocation failed"))?;
                if self.body.capacity() > MAX_FRAME_BYTES as usize {
                    self.body = Vec::new();
                    return Err(io::Error::other("response allocation exceeded cap"));
                }
            }
            self.body.extend_from_slice(bytes);
            Ok(bytes.len())
        })();
        if let Err(ref error) = result {
            self.failure = Some(io::Error::new(
                error.kind(),
                "bounded response serialization failed",
            ));
        }
        result
    }
    fn flush(&mut self) -> io::Result<()> {
        check_deadline(self.deadline)
    }
}

pub(crate) fn check_deadline(deadline: Instant) -> io::Result<()> {
    if Instant::now() >= deadline {
        Err(timed_out())
    } else {
        Ok(())
    }
}

fn timed_out() -> io::Error {
    io::Error::new(io::ErrorKind::TimedOut, "IPC absolute deadline expired")
}

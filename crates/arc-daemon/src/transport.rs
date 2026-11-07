//! Async protocol-1 I/O with deadlines fixed before any partial progress.

use arc_ipc::{FrameError, Response, MAX_FRAME_BYTES};
use std::io::{self, Write};
use tokio::{
    net::UnixStream,
    time::{timeout_at, Instant},
};

pub(crate) async fn read_header(stream: &UnixStream, deadline: Instant) -> Result<u32, FrameError> {
    let mut header = [0; 4];
    read_exact(stream, &mut header, deadline, true).await?;
    let declared = u32::from_be_bytes(header);
    validate_size(declared)?;
    Ok(declared)
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

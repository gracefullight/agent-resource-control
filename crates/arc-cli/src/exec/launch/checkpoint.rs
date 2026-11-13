//! Explicit delay-only native acceptance seam. It cannot authorize execution.
//! The installed CLI ignores it unless both acceptance variables are supplied.
use super::Failure;
use serde_json::Value;
use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    time::{Duration, Instant},
};

pub(super) fn pause(
    stage: &str,
    value: &Value,
    cancelled: &dyn Fn() -> bool,
) -> Result<(), Failure> {
    if !std::env::var("ARC_PRESSURE_ACCEPTANCE_STAGE")
        .is_ok_and(|stages| stages.split(',').any(|selected| selected == stage))
    {
        return Ok(());
    }
    let path = std::env::var_os("ARC_PRESSURE_ACCEPTANCE_BARRIER").ok_or(Failure::Local)?;
    let mut stream = UnixStream::connect(path).map_err(|_| Failure::Local)?;
    stream
        .set_write_timeout(Some(Duration::from_millis(100)))
        .map_err(|_| Failure::Local)?;
    stream
        .set_read_timeout(Some(Duration::from_millis(50)))
        .map_err(|_| Failure::Local)?;
    serde_json::to_writer(&mut stream, value).map_err(|_| Failure::Local)?;
    stream.write_all(b"\n").map_err(|_| Failure::Local)?;
    let end = Instant::now() + Duration::from_secs(10);
    let mut byte = [0u8];
    loop {
        if cancelled() {
            return Err(Failure::Cancelled);
        }
        if Instant::now() >= end {
            return Err(Failure::Local);
        }
        match stream.read_exact(&mut byte) {
            Ok(()) if byte == [b'x'] => return Ok(()),
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            _ => return Err(Failure::Local),
        }
    }
}

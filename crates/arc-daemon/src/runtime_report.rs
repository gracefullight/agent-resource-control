//! Optional final report: one safe open, bounded serialization, one final write.

use crate::{limits::RuntimeLimits, server};
use std::{
    ffi::{CString, OsStr},
    fs::File,
    io::{self, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{
            ffi::OsStrExt,
            fs::{MetadataExt, PermissionsExt},
        },
    },
    path::Path,
};
use tokio::time::Instant;

pub(crate) const MAX_REPORT_BYTES: usize = 4096;

pub(crate) struct RuntimeReport(Option<File>);

impl RuntimeReport {
    #[cfg(test)]
    pub(crate) fn read_only_for_test(path: &Path) -> Self {
        Self(Some(File::open(path).unwrap()))
    }

    pub(crate) fn from_env() -> io::Result<Self> {
        Self::open(
            std::env::var_os("ARC_RUNTIME_REPORT")
                .as_deref()
                .map(Path::new),
        )
    }

    pub(crate) fn open(path: Option<&Path>) -> io::Result<Self> {
        let Some(path) = path else {
            return Ok(Self(None));
        };
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "report needs a private parent directory",
                )
            })?;
        let name = path
            .file_name()
            .filter(|n| *n != OsStr::new(".") && *n != OsStr::new(".."))
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidInput, "report needs a file name")
            })?;
        let name = CString::new(name.as_bytes())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "report path contains NUL"))?;
        let directory = server::open_private_directory(parent)?;
        // SAFETY: the directory descriptor is live and name is NUL terminated.
        // EXCL refuses existing regular files, links and other entries atomically.
        let fd = unsafe {
            libc::openat(
                directory.as_raw_fd(),
                name.as_ptr(),
                libc::O_WRONLY
                    | libc::O_CREAT
                    | libc::O_EXCL
                    | libc::O_NOFOLLOW
                    | libc::O_CLOEXEC
                    | libc::O_NONBLOCK,
                0o600,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: openat returned a new owned descriptor.
        let file = unsafe { File::from_raw_fd(fd) };
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        let metadata = file.metadata()?;
        if !metadata.is_file()
            || metadata.uid() != unsafe { libc::geteuid() }
            || metadata.mode() & 0o7777 != 0o600
            || metadata.nlink() != 1
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid runtime report file",
            ));
        }
        Ok(Self(Some(file)))
    }

    pub(crate) fn finish(mut self, value: &serde_json::Value, deadline: Instant) -> io::Result<()> {
        let Some(file) = &mut self.0 else {
            return Ok(());
        };
        check_deadline(deadline)?;
        let bytes = encode(value)?;
        check_deadline(deadline)?;
        file.write_all(&bytes.bytes[..bytes.len])?;
        check_deadline(deadline)
    }
}

fn check_deadline(deadline: Instant) -> io::Result<()> {
    if Instant::now() >= deadline {
        Err(io::Error::new(
            io::ErrorKind::TimedOut,
            "runtime report deadline expired",
        ))
    } else {
        Ok(())
    }
}

struct Encoded {
    bytes: [u8; MAX_REPORT_BYTES],
    len: usize,
}
impl Write for Encoded {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_REPORT_BYTES - self.len {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "runtime report exceeds 4096 bytes",
            ));
        }
        self.bytes[self.len..self.len + bytes.len()].copy_from_slice(bytes);
        self.len += bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn encode(value: &serde_json::Value) -> io::Result<Encoded> {
    let mut result = Encoded {
        bytes: [0; MAX_REPORT_BYTES],
        len: 0,
    };
    serde_json::to_writer(&mut result, value).map_err(io::Error::other)?;
    Ok(result)
}

pub(crate) fn configuration(limits: &RuntimeLimits) -> serde_json::Value {
    serde_json::json!({
        "connections": limits.connections.get(), "requests": limits.requests.get(),
        "db_queue": limits.db_queue.get(), "db_executing": 1,
        "max_frame_bytes": arc_ipc::MAX_FRAME_BYTES,
        "header_timeout_ms": limits.header_timeout.as_millis(),
        "frame_timeout_ms": limits.frame_timeout.as_millis(),
        "write_timeout_ms": limits.write_timeout.as_millis(),
        "db_response_timeout_ms": limits.db_response_timeout.as_millis(),
        "db_execution_timeout_ms": limits.db_execution_timeout.as_millis(),
        "db_busy_timeout_ms": limits.db_busy_timeout.as_millis(),
        "shutdown_timeout_ms": limits.shutdown_timeout.as_millis(),
    })
}

#[cfg(test)]
#[path = "runtime_report_tests.rs"]
mod tests;

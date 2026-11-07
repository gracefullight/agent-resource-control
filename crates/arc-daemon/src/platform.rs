//! Platform checks for the resident daemon.
//!
//! The daemon is single-user: IPC peers must belong to the same account and a
//! lock file records a pid plus process start time so a stale lock is
//! recoverable without killing any process.

use std::{io, os::unix::net::UnixStream};

/// Verify the peer of an accepted Unix socket connection is this user.
pub fn ensure_same_user(stream: &UnixStream) -> io::Result<()> {
    #[cfg(target_os = "linux")]
    {
        use std::os::fd::AsRawFd;
        let mut credentials = libc::ucred {
            pid: 0,
            uid: 0,
            gid: 0,
        };
        let mut length = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
        let result = unsafe {
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                &mut credentials as *mut _ as *mut libc::c_void,
                &mut length,
            )
        };
        if result != 0 {
            return Err(io::Error::last_os_error());
        }
        if credentials.uid != unsafe { libc::geteuid() } {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "IPC peer belongs to another user",
            ));
        }
        Ok(())
    }

    #[cfg(target_os = "macos")]
    {
        use std::os::fd::AsRawFd;
        let mut uid: libc::uid_t = 0;
        let mut gid: libc::gid_t = 0;
        let result = unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) };
        if result != 0 {
            return Err(io::Error::last_os_error());
        }
        if uid != unsafe { libc::geteuid() } {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "IPC peer belongs to another user",
            ));
        }
        Ok(())
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = stream;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "same-user IPC checks are not implemented on this platform",
        ))
    }
}

/// Whether a process id currently exists (permission errors count as alive).
pub fn process_is_alive(pid: u32) -> bool {
    let result = unsafe { libc::kill(pid as libc::pid_t, 0) };
    if result == 0 {
        return true;
    }
    io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
}

/// Platform process-start token, stable across pid reuse (microseconds on macOS,
/// clock ticks since boot on Linux).
pub fn process_start_time(pid: u32) -> io::Result<u64> {
    #[cfg(target_os = "macos")]
    {
        let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
        let size = std::mem::size_of::<libc::proc_bsdinfo>() as libc::c_int;
        let written = unsafe {
            libc::proc_pidinfo(
                pid as libc::c_int,
                libc::PROC_PIDTBSDINFO,
                0,
                &mut info as *mut _ as *mut libc::c_void,
                size,
            )
        };
        if written != size {
            return Err(io::Error::last_os_error());
        }
        Ok(info.pbi_start_tvsec * 1_000_000 + info.pbi_start_tvusec)
    }

    #[cfg(target_os = "linux")]
    {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat"))?;
        let closing = stat
            .rfind(')')
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "malformed process stat"))?;
        let fields: Vec<&str> = stat[closing + 1..].split_whitespace().collect();
        let start = fields.get(19).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "missing process start time")
        })?;
        start
            .parse::<u64>()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid process start time"))
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = pid;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "process start time is not implemented on this platform",
        ))
    }
}

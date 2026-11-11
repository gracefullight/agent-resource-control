//! Verify the connected server's effective user, as the daemon does for clients.
use std::{io, os::fd::AsRawFd};

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub(super) fn check_uid(uid: libc::uid_t, effective_uid: libc::uid_t) -> io::Result<()> {
    if uid != effective_uid {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "IPC peer belongs to another user",
        ));
    }
    Ok(())
}

pub(super) fn ensure_same_user(stream: &(impl AsRawFd + ?Sized)) -> io::Result<()> {
    #[cfg(target_os = "macos")]
    let uid = {
        let mut uid = 0;
        let mut gid = 0;
        if unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) } != 0 {
            return Err(io::Error::last_os_error());
        }
        uid
    };
    #[cfg(target_os = "linux")]
    let uid = {
        let mut credentials: libc::ucred = unsafe { std::mem::zeroed() };
        let mut len = std::mem::size_of::<libc::ucred>() as libc::socklen_t;
        if unsafe {
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                (&mut credentials as *mut libc::ucred).cast(),
                &mut len,
            )
        } != 0
        {
            return Err(io::Error::last_os_error());
        }
        if len as usize != std::mem::size_of::<libc::ucred>() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid peer credentials",
            ));
        }
        credentials.uid
    };
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        check_uid(uid, unsafe { libc::geteuid() })
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

/// Capture the process behind the authenticated socket. A PID alone is not
/// enough: it may have been recycled between separate exchanges.
pub(super) fn identity(
    stream: &(impl AsRawFd + ?Sized),
) -> io::Result<arc_core::process_identity::ProcessIdentity> {
    #[cfg(target_os = "macos")]
    let pid = {
        let mut pid: libc::pid_t = 0;
        let mut len = std::mem::size_of_val(&pid) as libc::socklen_t;
        if unsafe {
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_LOCAL,
                libc::LOCAL_PEERPID,
                (&mut pid as *mut libc::pid_t).cast(),
                &mut len,
            )
        } != 0
            || len as usize != std::mem::size_of_val(&pid)
            || pid <= 0
        {
            return Err(io::Error::last_os_error());
        }
        pid as u32
    };
    #[cfg(target_os = "linux")]
    let pid = {
        let mut credentials: libc::ucred = unsafe { std::mem::zeroed() };
        let mut len = std::mem::size_of_val(&credentials) as libc::socklen_t;
        if unsafe {
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_PEERCRED,
                (&mut credentials as *mut libc::ucred).cast(),
                &mut len,
            )
        } != 0
            || len as usize != std::mem::size_of_val(&credentials)
            || credentials.pid <= 0
        {
            return Err(io::Error::last_os_error());
        }
        credentials.pid as u32
    };
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        arc_core::process_identity::ProcessIdentity::capture(pid)
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = stream;
        Err(io::ErrorKind::Unsupported.into())
    }
}

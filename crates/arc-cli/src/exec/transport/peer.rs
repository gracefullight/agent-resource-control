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

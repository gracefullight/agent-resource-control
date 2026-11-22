//! The process holding a daemon capability advertisement beside a socket.
//!
//! The daemon advertises a capability by holding a POSIX write lock on a
//! file beside its socket for its lifetime (`arc-daemon
//! capability_advertisement.rs`); the kernel names the holder. A file nobody
//! locks, or one owned by another user, advertises nothing. Included by the
//! hook, `hook-exec` and the Meter reads of lane B, so one definition serves
//! every gate of `meter.usage-summary.v1` and `meter.snapshot-traffic.v1`.
use std::{
    fs::OpenOptions,
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, OpenOptionsExt},
    },
    path::Path,
};

/// The PID holding the write lock on `advertisement` beside `socket`.
pub(super) fn holder(socket: &Path, advertisement: &str) -> Option<u32> {
    let directory = socket.parent().filter(|p| !p.as_os_str().is_empty())?;
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
        .open(directory.join(advertisement))
        .ok()?;
    // SAFETY: geteuid has no preconditions.
    if !file
        .metadata()
        .is_ok_and(|m| m.is_file() && m.uid() == unsafe { libc::geteuid() })
    {
        return None;
    }
    // SAFETY: an all-zero flock is valid; fcntl receives a live descriptor
    // and writable storage. F_GETLK takes no lock of its own.
    let pid = unsafe {
        let mut lock: libc::flock = std::mem::zeroed();
        lock.l_type = libc::F_RDLCK as _;
        lock.l_whence = libc::SEEK_SET as _;
        if libc::fcntl(file.as_raw_fd(), libc::F_GETLK, &mut lock) != 0
            || lock.l_type == libc::F_UNLCK as libc::c_short
        {
            return None;
        }
        lock.l_pid
    };
    u32::try_from(pid).ok().filter(|pid| *pid > 0)
}

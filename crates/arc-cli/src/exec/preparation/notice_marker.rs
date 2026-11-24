//! Distinguish a reaped helper that never attempted stderr from an uncertain write.
use super::NoticeWrite;
use std::{
    io,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
};

/// The helper records intent before touching stderr. If it is reaped without
/// that byte, a retry cannot duplicate or interleave an earlier notice.
pub(super) struct NoticeMarker {
    pub(super) read: OwnedFd,
    pub(super) write: OwnedFd,
}

impl NoticeMarker {
    pub(super) fn new() -> Option<Self> {
        let mut fds = [-1; 2];
        if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
            return None;
        }
        // SAFETY: pipe returned two new, distinct owned descriptors.
        let marker = unsafe {
            Self {
                read: OwnedFd::from_raw_fd(fds[0]),
                write: OwnedFd::from_raw_fd(fds[1]),
            }
        };
        for fd in [marker.read.as_raw_fd(), marker.write.as_raw_fd()] {
            if unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
                return None;
            }
        }
        let flags = unsafe { libc::fcntl(marker.read.as_raw_fd(), libc::F_GETFL) };
        if flags < 0
            || unsafe {
                libc::fcntl(
                    marker.read.as_raw_fd(),
                    libc::F_SETFL,
                    flags | libc::O_NONBLOCK,
                )
            } < 0
        {
            return None;
        }
        Some(marker)
    }

    pub(super) fn intent_seen(&self) -> Option<bool> {
        let mut byte = 0_u8;
        loop {
            let n = unsafe { libc::read(self.read.as_raw_fd(), (&mut byte as *mut u8).cast(), 1) };
            if n == 1 {
                return Some(true);
            }
            if n == 0 {
                return Some(false);
            }
            match io::Error::last_os_error().kind() {
                io::ErrorKind::Interrupted => continue,
                io::ErrorKind::WouldBlock => return Some(false),
                _ => return None,
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct NoticeWait {
    pub(super) outcome: NoticeWrite,
    pub(super) reaped: bool,
}

pub(super) fn resolve_notice(result: NoticeWait, intent_seen: Option<bool>) -> NoticeWrite {
    if result.reaped && result.outcome == NoticeWrite::Uncertain && intent_seen == Some(false) {
        NoticeWrite::NotWritten
    } else {
        result.outcome
    }
}

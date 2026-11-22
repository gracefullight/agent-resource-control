//! Poll cadence of one queued wrapper.
//!
//! A `FIFO` reply says this job is not the queue head: no poll of its own can
//! admit it before another job is admitted. Two rules keep a deep queue from
//! saturating the daemon's bounded SQLite queue, which at 101 waiters used to
//! return `DB_BUSY` until confirmed waiters failed open.
//!
//! - A waiter behind the head doubles its delay from 40 ms up to a ceiling.
//!   The head, and a waiter held for any other reason, keeps the 40 ms
//!   cadence, so the job that can start next still starts promptly.
//! - Waiters behind the head poll one at a time. Each takes an advisory lock
//!   on one file beside the socket for the duration of its poll, so however
//!   many wait and however long the daemon stalls, they add one request.
//!
//! The lock is an optimisation only: when the file cannot be used the waiter
//! polls on its cadence without it. Every delay and every lock attempt is
//! taken in 40 ms slices, so a signal cancels within one slice as before.
use arc_ipc::traffic::WaitReason;
use std::{
    fs::{File, OpenOptions},
    os::{
        fd::AsRawFd,
        unix::fs::{MetadataExt, OpenOptionsExt},
    },
    path::{Path, PathBuf},
    time::Duration,
};

const STEP: Duration = Duration::from_millis(40);
const DOUBLINGS: u32 = 6;
const TURN: &str = ".traffic-wait-turn";

pub(in crate::exec) struct Cadence {
    behind: u32,
    turn: Option<PathBuf>,
    held: Option<File>,
}

impl Cadence {
    pub(in crate::exec) fn new(socket: &Path) -> Self {
        Self {
            behind: 0,
            turn: socket
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .map(|parent| parent.join(TURN)),
            held: None,
        }
    }

    /// The delay before the next poll after a `QUEUED` reply.
    fn delay(&mut self, reason: WaitReason) -> Duration {
        if reason != WaitReason::Fifo {
            self.behind = 0;
            return STEP;
        }
        let delay = STEP * (1 << self.behind.min(DOUBLINGS));
        self.behind = self.behind.saturating_add(1);
        delay
    }

    /// Wait until this wrapper may poll again. The previous poll has been
    /// answered, so its turn ends here. Returns early once `cancelled` reports
    /// a signal; the caller's own cancellation check then ends the wait.
    pub(in crate::exec) fn pause(
        &mut self,
        reason: WaitReason,
        sleep: &dyn Fn(Duration),
        cancelled: &dyn Fn() -> bool,
    ) {
        self.held = None;
        let mut remaining = self.delay(reason);
        while !remaining.is_zero() {
            let slice = remaining.min(STEP);
            sleep(slice);
            remaining -= slice;
            if cancelled() {
                return;
            }
        }
        if reason != WaitReason::Fifo {
            return;
        }
        let Some(file) = self.turn.as_deref().and_then(open) else {
            return;
        };
        // SAFETY: flock receives a live descriptor owned by `file`.
        while unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
            if std::io::Error::last_os_error().kind() != std::io::ErrorKind::WouldBlock {
                return;
            }
            sleep(STEP);
            if cancelled() {
                return;
            }
        }
        // Closing the descriptor, here or at exit, releases the lock.
        self.held = Some(file);
    }
}

/// The shared turn file: a regular file of this user, never followed or
/// created through a link. It holds no data and is never removed.
fn open(path: &Path) -> Option<File> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK)
        .open(path)
        .ok()?;
    let metadata = file.metadata().ok()?;
    // SAFETY: geteuid has no preconditions.
    (metadata.is_file() && metadata.uid() == unsafe { libc::geteuid() }).then_some(file)
}

#[cfg(test)]
mod tests;

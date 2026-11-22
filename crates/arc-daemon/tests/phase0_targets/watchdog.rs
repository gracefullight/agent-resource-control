//! A diagnostic deadline for a child that is awaited with blocking calls, so
//! the wait adds no polling delay to a timed sample.
use std::{sync::mpsc, thread, time::Duration};

pub const LIMIT: Duration = Duration::from_secs(60);

pub struct Watchdog {
    disarm: mpsc::Sender<()>,
    thread: thread::JoinHandle<bool>,
    label: &'static str,
}

impl Watchdog {
    /// Kill the not yet reaped child `pid` if it is still awaited after the limit.
    pub fn arm(pid: u32, label: &'static str) -> Self {
        let (disarm, expired) = mpsc::channel::<()>();
        let thread = thread::spawn(move || {
            let fired = matches!(
                expired.recv_timeout(LIMIT),
                Err(mpsc::RecvTimeoutError::Timeout)
            );
            if fired {
                // SAFETY: the owner has not reaped the child, so the pid is its own.
                unsafe {
                    libc::kill(pid as libc::pid_t, libc::SIGKILL);
                }
            }
            fired
        });
        Self {
            disarm,
            thread,
            label,
        }
    }

    /// Call once the child has been reaped; fails the test if the deadline fired.
    #[track_caller]
    pub fn disarm(self) {
        drop(self.disarm);
        assert!(
            !self.thread.join().unwrap(),
            "{}: child exceeded the {LIMIT:?} watchdog",
            self.label
        );
    }
}

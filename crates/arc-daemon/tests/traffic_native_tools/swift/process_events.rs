//! Event-driven waiting and output draining for the owned Swift fixture.
use super::TreeChild;
use std::{
    io::{self, Read},
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    time::{Duration, Instant},
};

pub(super) fn is_terminal_event(info: &libc::siginfo_t) -> bool {
    info.si_pid != 0
        && matches!(
            info.si_code,
            libc::CLD_EXITED | libc::CLD_KILLED | libc::CLD_DUMPED
        )
}

pub(super) fn wait_for_group_leader_stop(pid: u32) -> io::Result<()> {
    let end = Instant::now() + Duration::from_secs(60);
    loop {
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        let result = unsafe {
            libc::waitid(
                libc::P_PID,
                pid,
                &mut info,
                libc::WSTOPPED | libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        if result != 0 {
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error);
            }
        } else if info.si_pid != 0
            && (info.si_code == libc::CLD_STOPPED || is_terminal_event(&info))
        {
            return Ok(());
        }
        if Instant::now() >= end {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("Swift group leader {pid} never stopped or exited"),
            ));
        }
        std::thread::yield_now();
    }
}

pub(super) fn nonblocking(pipe: Option<&impl AsRawFd>) -> io::Result<()> {
    if let Some(pipe) = pipe {
        let fd = pipe.as_raw_fd();
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

// A process-exit notification and both output pipes wake the drain loop. This
// keeps a slow compiler from consuming a core in an elapsed-time poll loop.
pub(super) struct OutputEvents(OwnedFd);
impl OutputEvents {
    pub(super) fn new(
        pid: u32,
        out: Option<&impl AsRawFd>,
        err: Option<&impl AsRawFd>,
        child: &TreeChild,
    ) -> io::Result<Self> {
        let fd = unsafe { libc::kqueue() };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let queue = Self(unsafe { OwnedFd::from_raw_fd(fd) });
        for pipe in [out.map(AsRawFd::as_raw_fd), err.map(AsRawFd::as_raw_fd)]
            .into_iter()
            .flatten()
        {
            queue.register(pipe as usize, libc::EVFILT_READ, 0)?;
        }
        if let Err(error) = queue.register(pid as usize, libc::EVFILT_PROC, libc::NOTE_EXIT) {
            // Darwin may stop accepting a process filter just before waitid
            // exposes its terminal state. Keep the unreaped owned child and
            // observe that state directly; a missing event is not its exit.
            recover_process_registration(error, Duration::from_secs(60), || {
                child.exited_without_reaping()
            })?;
        }
        Ok(queue)
    }

    fn register(&self, ident: usize, filter: i16, flags: u32) -> io::Result<()> {
        let mut change: libc::kevent = unsafe { std::mem::zeroed() };
        change.ident = ident as _;
        change.filter = filter;
        change.flags = libc::EV_ADD | libc::EV_ENABLE;
        change.fflags = flags;
        let result = unsafe {
            libc::kevent(
                self.0.as_raw_fd(),
                &change,
                1,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
            )
        };
        if result < 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    pub(super) fn wait(&self, deadline: Option<Instant>) -> io::Result<()> {
        let timeout = deadline.map(|end| {
            let duration = end.saturating_duration_since(Instant::now());
            libc::timespec {
                tv_sec: duration.as_secs() as _,
                tv_nsec: duration.subsec_nanos() as _,
            }
        });
        let mut event: libc::kevent = unsafe { std::mem::zeroed() };
        let result = unsafe {
            libc::kevent(
                self.0.as_raw_fd(),
                std::ptr::null(),
                0,
                &mut event,
                1,
                timeout
                    .as_ref()
                    .map_or(std::ptr::null(), |value| value as *const _),
            )
        };
        if result < 0 && io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

pub(super) fn recover_process_registration(
    error: io::Error,
    timeout: Duration,
    exited: impl FnMut() -> io::Result<bool>,
) -> io::Result<()> {
    wait_for_exit_after_missing(error, timeout, "process-event registration", exited)
}

fn wait_for_exit_after_missing(
    error: io::Error,
    timeout: Duration,
    context: &str,
    mut exited: impl FnMut() -> io::Result<bool>,
) -> io::Result<()> {
    if error.raw_os_error() != Some(libc::ESRCH) {
        return Err(error);
    }
    let end = Instant::now() + timeout;
    loop {
        if exited()? {
            return Ok(());
        }
        if Instant::now() >= end {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("Swift direct child never exposed exit after {context} disappeared"),
            ));
        }
        std::thread::yield_now();
    }
}

pub(super) fn signal_owned_group(
    root: u32,
    mut exited: impl FnMut() -> io::Result<bool>,
    group: impl FnOnce() -> io::Result<i32>,
    signal: impl FnOnce() -> io::Result<()>,
) -> io::Result<()> {
    if exited()? {
        return Ok(());
    }
    match group() {
        Ok(group) if group == root as i32 => (),
        Ok(group) => {
            return Err(io::Error::other(format!(
                "Swift child {root} lost its owned process group (current group {group})"
            )));
        }
        Err(error) if error.raw_os_error() == Some(libc::ESRCH) => {
            // waitid(WNOWAIT) can lag behind getpgid during the live-to-zombie
            // transition. Never infer exit from a failed lookup alone.
            return wait_for_exit_after_missing(
                error,
                Duration::from_secs(60),
                "process-group lookup",
                exited,
            );
        }
        Err(error) => {
            return Err(io::Error::new(
                error.kind(),
                format!("cannot observe Swift child {root} process group: {error}"),
            ))
        }
    }
    match signal() {
        Err(error) if error.raw_os_error() == Some(libc::ESRCH) => wait_for_exit_after_missing(
            error,
            Duration::from_secs(60),
            "process-group signal",
            exited,
        ),
        result => result,
    }
}

pub(super) fn drain(pipe: &mut Option<impl Read>, bytes: &mut Vec<u8>) -> io::Result<()> {
    let mut buffer = [0; 16384];
    for _ in 0..16 {
        let Some(reader) = pipe.as_mut() else { break };
        match reader.read(&mut buffer) {
            Ok(0) => {
                *pipe = None;
                break;
            }
            Ok(n) => bytes.extend_from_slice(&buffer[..n]),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

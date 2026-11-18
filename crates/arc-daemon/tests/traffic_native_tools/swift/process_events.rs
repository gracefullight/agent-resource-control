//! Event-driven waiting and output draining for the owned Swift fixture.
use super::TreeChild;
use arc_core::process_identity::ProcessIdentity;
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

// A process can become a zombie between identity capture and proc_pidinfo.
// Check the native process record, including its start token, before deciding
// that a failed parent or descendant query is a completed exit transition.
pub(super) fn terminal_or_replaced(identity: &ProcessIdentity) -> io::Result<bool> {
    if crate::cleanup::capture(identity.pid)?.as_ref() != Some(identity) {
        return Ok(true);
    }
    #[repr(C)]
    struct Prefix {
        start: libc::timeval,
        vmspace: *mut libc::c_void,
        sigacts: *mut libc::c_void,
        flags: libc::c_int,
        status: libc::c_char,
        pid: libc::pid_t,
    }
    let mut mib = [
        libc::CTL_KERN,
        libc::KERN_PROC,
        libc::KERN_PROC_PID,
        identity.pid as i32,
    ];
    let mut bytes = [0u8; 4096];
    let mut length = bytes.len();
    let result = unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            mib.len() as u32,
            bytes.as_mut_ptr().cast(),
            &mut length,
            std::ptr::null_mut(),
            0,
        )
    };
    if result != 0 {
        return Err(io::Error::last_os_error());
    }
    if length == 0 {
        return Ok(true);
    }
    if length < std::mem::size_of::<Prefix>() || length > bytes.len() {
        return Err(io::Error::other("incomplete Swift process state"));
    }
    let info = unsafe { std::ptr::read_unaligned(bytes.as_ptr().cast::<Prefix>()) };
    if info.start.tv_sec <= 0 || !(0..1_000_000).contains(&info.start.tv_usec) {
        return Err(io::Error::other("invalid Swift process start time"));
    }
    let start = (info.start.tv_sec as u64)
        .checked_mul(1_000_000)
        .and_then(|seconds| seconds.checked_add(info.start.tv_usec as u64));
    if info.pid != identity.pid as i32
        || start.map(|value| value.to_string()).as_ref() != Some(&identity.start_token)
    {
        return Ok(true);
    }
    Ok(info.status as u32 == libc::SZOMB)
}

fn query_after_transition<T>(
    mut query: impl FnMut() -> io::Result<T>,
    mut terminal: impl FnMut() -> io::Result<bool>,
    mut retry: impl FnMut() -> bool,
) -> io::Result<Option<T>> {
    loop {
        match query() {
            Ok(value) => return Ok(Some(value)),
            Err(error) => {
                if terminal()? {
                    return Ok(None);
                }
                if error.raw_os_error() == Some(libc::ESRCH) && retry() {
                    continue;
                }
                return Err(error);
            }
        }
    }
}

pub(super) fn child_parent(identity: &ProcessIdentity) -> io::Result<Option<u32>> {
    let end = Instant::now() + Duration::from_secs(60);
    query_after_transition(
        || {
            let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
            let size = std::mem::size_of_val(&info) as i32;
            let written = unsafe {
                libc::proc_pidinfo(
                    identity.pid as i32,
                    libc::PROC_PIDTBSDINFO,
                    0,
                    (&mut info as *mut libc::proc_bsdinfo).cast(),
                    size,
                )
            };
            if written == size {
                Ok(info.pbi_ppid)
            } else {
                Err(io::Error::last_os_error())
            }
        },
        || terminal_or_replaced(identity),
        || {
            std::thread::yield_now();
            Instant::now() < end
        },
    )
}

pub(super) fn owned_children(identity: &ProcessIdentity) -> io::Result<Vec<u32>> {
    match crate::cleanup::children(identity.pid) {
        Ok(children) => Ok(children),
        Err(_) if terminal_or_replaced(identity)? => Ok(Vec::new()),
        Err(error) => Err(error),
    }
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

#[cfg(test)]
mod transition_tests {
    use super::*;

    #[test]
    fn missing_parent_observation_recovers_only_after_verified_exit() {
        let mut attempts = 0;
        let result = query_after_transition(
            || {
                attempts += 1;
                Err::<u32, _>(io::Error::from_raw_os_error(libc::ESRCH))
            },
            || Ok(true),
            || panic!("verified exit must not be retried"),
        )
        .unwrap();
        assert_eq!(result, None);
        assert_eq!(attempts, 1);
    }

    #[test]
    fn transient_missing_parent_is_retried_but_denial_stays_an_error() {
        let mut attempts = 0;
        let result = query_after_transition(
            || {
                attempts += 1;
                if attempts == 1 {
                    Err(io::Error::from_raw_os_error(libc::ESRCH))
                } else {
                    Ok(17)
                }
            },
            || Ok(false),
            || true,
        )
        .unwrap();
        assert_eq!(result, Some(17));
        let error = query_after_transition(
            || Err::<u32, _>(io::Error::from_raw_os_error(libc::EACCES)),
            || Ok(false),
            || panic!("denied observation must not be retried"),
        )
        .unwrap_err();
        assert_eq!(error.raw_os_error(), Some(libc::EACCES));
    }
}

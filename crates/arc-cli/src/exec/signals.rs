//! Signal ownership is synchronized on the single-threaded exec path.
use std::{
    io,
    os::unix::process::ExitStatusExt,
    sync::atomic::{AtomicI32, AtomicU32, Ordering},
};
// SPAWNING retains cancellation until fork; the child checks its inherited copy.
const PREPARING: i32 = 0;
const SPAWNING: i32 = -2;
static CHILD: AtomicI32 = AtomicI32::new(-1);
static PENDING: AtomicU32 = AtomicU32::new(0);
static CONTINUE_CHILD: AtomicI32 = AtomicI32::new(0);
const NONTERMINAL: [i32; 4] = [libc::SIGTERM, libc::SIGHUP, libc::SIGUSR1, libc::SIGUSR2];
const TERMINAL: [i32; 2] = [libc::SIGINT, libc::SIGQUIT];
extern "C" fn receive(signal: i32) {
    #[cfg(target_os = "macos")]
    let errno = unsafe { libc::__error() };
    #[cfg(target_os = "linux")]
    let errno = unsafe { libc::__errno_location() };
    let saved = unsafe { *errno };
    let child = CHILD.load(Ordering::SeqCst);
    if signal == libc::SIGCONT {
        if child > 0 && CONTINUE_CHILD.swap(0, Ordering::SeqCst) != 0 {
            // A target-only SIGSTOP needs PID-only continuation. Shell job
            // stop/continue is delivered by the OS, never forwarded as a group.
            unsafe { libc::kill(child, libc::SIGCONT) };
        }
    } else if signal != libc::SIGCHLD {
        if child > 0 {
            unsafe { libc::kill(child, signal) };
        } else if child == PREPARING || child == SPAWNING {
            PENDING.fetch_or(1u32 << signal, Ordering::SeqCst);
        }
    }
    unsafe { *errno = saved };
}
struct Mask(libc::sigset_t);
impl Mask {
    fn block() -> Self {
        unsafe {
            let mut set = std::mem::zeroed();
            let mut old = std::mem::zeroed();
            libc::sigfillset(&mut set);
            assert_eq!(libc::sigprocmask(libc::SIG_BLOCK, &set, &mut old), 0);
            Self(old)
        }
    }
}
impl Drop for Mask {
    fn drop(&mut self) {
        unsafe { libc::sigprocmask(libc::SIG_SETMASK, &self.0, std::ptr::null_mut()) };
    }
}
// New diagnostic threads inherit this blocked mask, leaving the supervising
// thread as the sole recipient of child and forwarding notifications.
pub(super) fn with_blocked_signals<T>(f: impl FnOnce() -> T) -> io::Result<T> {
    struct ThreadMask(libc::sigset_t);
    impl Drop for ThreadMask {
        fn drop(&mut self) {
            unsafe { libc::pthread_sigmask(libc::SIG_SETMASK, &self.0, std::ptr::null_mut()) };
        }
    }
    let mut set = unsafe { std::mem::zeroed() };
    let mut old = unsafe { std::mem::zeroed() };
    unsafe { libc::sigfillset(&mut set) };
    let error = unsafe { libc::pthread_sigmask(libc::SIG_BLOCK, &set, &mut old) };
    if error != 0 {
        return Err(io::Error::from_raw_os_error(error));
    }
    let _mask = ThreadMask(old);
    Ok(f())
}
pub(crate) struct Signals {
    prior: Vec<(i32, libc::sigaction)>,
    shared_job: bool,
}
impl Signals {
    pub(crate) fn install(shared_job: bool) -> io::Result<Self> {
        let _mask = Mask::block();
        CHILD.store(PREPARING, Ordering::SeqCst);
        PENDING.store(0, Ordering::SeqCst);
        CONTINUE_CHILD.store(PREPARING, Ordering::SeqCst);
        let mut guard = Self {
            prior: Vec::new(),
            shared_job,
        };
        for signal in NONTERMINAL
            .into_iter()
            .chain(TERMINAL)
            .chain([libc::SIGCONT, libc::SIGCHLD])
        {
            unsafe {
                let mut action: libc::sigaction = std::mem::zeroed();
                let mut old = std::mem::zeroed();
                if libc::sigaction(signal, std::ptr::null(), &mut old) != 0 {
                    return Err(io::Error::last_os_error());
                }
                // CHLD and CONT are private to supervision: inherited ignore
                // must neither discard our wait result nor strand a target
                // stopped separately from its wrapper. Restore both in exec.
                if ![libc::SIGCHLD, libc::SIGCONT].contains(&signal)
                    && old.sa_sigaction == libc::SIG_IGN
                {
                    continue;
                }
                // Terminal signals already reach the target in a shared job.
                action.sa_sigaction = if shared_job && TERMINAL.contains(&signal) {
                    quiet as *const () as usize
                } else {
                    receive as *const () as usize
                };
                action.sa_flags = libc::SA_RESTART;
                // No nested handler can interleave attachment/ownership state.
                libc::sigfillset(&mut action.sa_mask);
                if libc::sigaction(signal, &action, &mut old) != 0 {
                    return Err(io::Error::last_os_error());
                }
                guard.prior.push((signal, old));
            }
        }
        Ok(guard)
    }
    pub(crate) fn prepare_child(&self, command: &mut std::process::Command) {
        use std::os::unix::process::CommandExt;
        let actions = self.prior.clone();
        // SAFETY: only lock-free atomics, signal syscalls and errno inspection
        // run after fork. No allocation or parent synchronization is needed.
        unsafe {
            command.pre_exec(move || {
                let mut all = std::mem::zeroed();
                let mut original = std::mem::zeroed();
                libc::sigfillset(&mut all);
                if libc::sigprocmask(libc::SIG_BLOCK, &all, &mut original) != 0 {
                    return Err(io::Error::last_os_error());
                }
                // A signal handled before fork is copied into the child. Reject
                // execution even if it arrived after the parent's admission.
                if PENDING.load(Ordering::SeqCst) != 0 {
                    return Err(io::Error::from_raw_os_error(libc::EINTR));
                }
                // Once a child exists, shared-group terminal delivery belongs
                // to it. Restore native dispositions before unblocking, closing
                // the quiet-handler interval before exec resets caught actions.
                for (signal, action) in &actions {
                    if libc::sigaction(*signal, action, std::ptr::null_mut()) != 0 {
                        return Err(io::Error::last_os_error());
                    }
                }
                if libc::sigprocmask(libc::SIG_SETMASK, &original, std::ptr::null_mut()) != 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
    pub(crate) fn interrupted(&self) -> bool {
        PENDING.load(Ordering::SeqCst) != 0
    }
    pub(crate) fn admit_spawn(&self) -> bool {
        let _mask = Mask::block();
        if self.interrupted() {
            return false;
        }
        // Keep recording through fork: admission is not proof of child creation.
        // The child checks its snapshot; attachment discards shared terminal
        // sightings because an existing group member received them from the OS.
        CHILD.store(SPAWNING, Ordering::SeqCst);
        true
    }
    pub(crate) fn attach(&self, pid: u32) {
        let _mask = Mask::block();
        CHILD.store(pid as i32, Ordering::SeqCst);
        let pending = PENDING.swap(0, Ordering::SeqCst);
        for signal in NONTERMINAL
            .into_iter()
            .chain(TERMINAL.into_iter().filter(|_| !self.shared_job))
        {
            if pending & (1u32 << signal) != 0 {
                unsafe { libc::kill(pid as i32, signal) };
            }
        }
    }
    pub(crate) fn wait(
        &self,
        child: &mut std::process::Child,
    ) -> io::Result<std::process::ExitStatus> {
        self.wait_with(child, || {})
    }
    fn wait_with(
        &self,
        child: &mut std::process::Child,
        mut before_suspend: impl FnMut(),
    ) -> io::Result<std::process::ExitStatus> {
        let mask = Mask::block();
        let mut wake_mask = mask.0;
        // These notifications belong to our waiter, even if the caller blocked
        // them. The original mask still reaches exec and is restored on return.
        unsafe {
            libc::sigdelset(&mut wake_mask, libc::SIGCHLD);
            libc::sigdelset(&mut wake_mask, libc::SIGCONT);
        }
        loop {
            let mut stopped = None;
            loop {
                let mut status = 0;
                // Consume every event with handlers blocked. Clearing ownership
                // before unmasking closes the native reap/recycled-PID interval.
                let pid = unsafe {
                    libc::waitpid(
                        child.id() as i32,
                        &mut status,
                        libc::WNOHANG | libc::WUNTRACED | libc::WCONTINUED,
                    )
                };
                if pid < 0 {
                    self.reaped();
                    return Err(io::Error::last_os_error());
                }
                if pid == 0 {
                    break;
                }
                if libc::WIFEXITED(status) || libc::WIFSIGNALED(status) {
                    self.reaped();
                    return Ok(std::process::ExitStatus::from_raw(status));
                }
                if libc::WIFSTOPPED(status) {
                    stopped = Some(libc::WSTOPSIG(status));
                }
                if libc::WIFCONTINUED(status) {
                    stopped = None;
                }
            }
            if let Some(signal) = stopped {
                CONTINUE_CHILD.store(
                    i32::from(!self.shared_job || signal == libc::SIGSTOP),
                    Ordering::SeqCst,
                );
                // All available stop/continue events have been consumed. A
                // pending continuation wins over an earlier stop notification.
                let mut pending = unsafe { std::mem::zeroed() };
                unsafe { libc::sigpending(&mut pending) };
                if unsafe { libc::sigismember(&pending, libc::SIGCONT) } == 0 {
                    unsafe { libc::raise(libc::SIGSTOP) };
                }
            }
            // Atomically restore the caller mask and sleep, allowing forwarding
            // and SIGCHLD/SIGCONT wakeups without a check-to-sleep race.
            before_suspend();
            unsafe { libc::sigsuspend(&wake_mask) };
        }
    }
    pub(crate) fn reaped(&self) {
        let _mask = Mask::block();
        CHILD.store(-1, Ordering::SeqCst);
        CONTINUE_CHILD.store(PREPARING, Ordering::SeqCst);
        PENDING.store(0, Ordering::SeqCst);
    }
}
extern "C" fn quiet(signal: i32) {
    // Before fork cancellation must survive admission. After fork, the child's
    // copy is independent and group signals have native single delivery there.
    if matches!(CHILD.load(Ordering::SeqCst), PREPARING | SPAWNING) {
        PENDING.fetch_or(1u32 << signal, Ordering::SeqCst);
    }
}
impl Drop for Signals {
    fn drop(&mut self) {
        let _mask = Mask::block();
        self.reaped();
        for (signal, old) in &self.prior {
            unsafe { libc::sigaction(*signal, old, std::ptr::null_mut()) };
        }
    }
}
pub(crate) fn reraise(signal: i32) {
    // Keep existing native signal status compatibility, including 128+signal
    // shell reporting, after lifecycle completion has recorded the observation.
    unsafe {
        libc::signal(signal, libc::SIG_DFL);
        let mut set = std::mem::zeroed();
        libc::sigemptyset(&mut set);
        libc::sigaddset(&mut set, signal);
        libc::sigprocmask(libc::SIG_UNBLOCK, &set, std::ptr::null_mut());
        libc::raise(signal);
        libc::_exit(128 + signal);
    }
}

#[cfg(test)]
mod tests;

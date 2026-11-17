//! Direct Go children use the maintained identity-aware tree cleanup on every exit.
use arc_core::process_identity::ProcessIdentity;
use std::{
    io::{self, Read},
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    process::{Child, Command, ExitStatus, Output, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

/// Capture a short fixture command with the same owned-child watchdog as Go.
pub fn fixture_output(command: &mut Command) -> io::Result<Output> {
    let description = format!("{command:?}");
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    GoChild::spawn(command)
        .and_then(|mut child| child.wait_with_output())
        .map_err(|error| {
            io::Error::new(
                error.kind(),
                format!("fixture command {description}: {error}"),
            )
        })
}

#[derive(Default)]
struct Counts {
    started: AtomicU64,
    cleaned: AtomicU64,
}
thread_local! {
    static COUNTS: Arc<Counts> = Arc::new(Counts::default());
}
pub fn counts() -> (u64, u64) {
    COUNTS.with(|counts| {
        (
            counts.started.load(Ordering::SeqCst),
            counts.cleaned.load(Ordering::SeqCst),
        )
    })
}

pub struct GoChild {
    pub child: Child,
    owned: Vec<ProcessIdentity>,
    reaped: bool,
    clean: bool,
    counts: Arc<Counts>,
}
impl GoChild {
    pub fn spawn(command: &mut Command) -> io::Result<Self> {
        let child = command.spawn()?;
        let counts = COUNTS.with(Arc::clone);
        counts.started.fetch_add(1, Ordering::SeqCst);
        let mut guard = Self {
            child,
            owned: Vec::new(),
            reaped: false,
            clean: false,
            counts,
        };
        if let Some(identity) = crate::cleanup::capture(guard.child.id())? {
            guard.owned.push(identity);
        }
        Ok(guard)
    }

    pub fn observe(&mut self) -> io::Result<()> {
        self.reaped |= self.child.try_wait()?.is_some();
        let mut at = 0;
        while at < self.owned.len() {
            let parent = self.owned[at].clone();
            if self.current(&parent)? {
                let children = match crate::cleanup::children(parent.pid) {
                    Ok(children) => children,
                    Err(_) if !self.current(&parent)? => Vec::new(),
                    Err(error) => return Err(error),
                };
                for pid in children {
                    if let Some(identity) = child_identity(parent.pid, pid)? {
                        if !self.owned.contains(&identity) {
                            self.owned.push(identity);
                        }
                    }
                }
            }
            at += 1;
            if self.owned.len() > 16_384 {
                return Err(io::Error::other("Go fixture descendant limit"));
            }
        }
        Ok(())
    }

    fn current(&mut self, identity: &ProcessIdentity) -> io::Result<bool> {
        if identity.pid == self.child.id() {
            self.reaped |= self.child.try_wait()?.is_some();
            if self.reaped {
                return Ok(false);
            }
        }
        Ok(crate::cleanup::capture(identity.pid)?.as_ref() == Some(identity))
    }

    pub fn identities(&self) -> Vec<ProcessIdentity> {
        self.owned.clone()
    }

    /// Stop children before their parent; retain exact native identities and
    /// require disappearance, including zombie reaping, before reporting success.
    pub fn stop(&mut self) -> io::Result<()> {
        self.stop_with(Self::observe, crate::cleanup::kill_tree)
    }

    fn stop_with(
        &mut self,
        observe: impl FnOnce(&mut Self) -> io::Result<()>,
        mut kill_tree: impl FnMut(&ProcessIdentity, usize) -> Vec<String>,
    ) -> io::Result<()> {
        if self.clean {
            return Ok(());
        }
        let mut errors = observe(self)
            .err()
            .map(|e| e.to_string())
            .into_iter()
            .collect::<Vec<_>>();
        for identity in self.owned.clone().iter().rev() {
            match self.current(identity) {
                Ok(true) => {
                    let tree_errors = kill_tree(identity, 0);
                    let termination_failed = !tree_errors.is_empty();
                    errors.extend(tree_errors);
                    if identity.pid == self.child.id() || termination_failed {
                        // The owned Child handle can terminate the root even if
                        // tree observation or termination was unsuccessful.
                        continue;
                    }
                    // Keep its parent alive long enough to reap this child.
                    // Otherwise a second tree walk can race its zombie transition.
                    let end = Instant::now() + Duration::from_secs(60);
                    loop {
                        match self.current(identity) {
                            Ok(false) => break,
                            Ok(true) => (),
                            Err(error) => {
                                errors.push(error.to_string());
                                break;
                            }
                        }
                        if Instant::now() >= end {
                            errors.push(format!("Go descendant not reaped: {}", identity.pid));
                            break;
                        }
                        thread::yield_now();
                    }
                }
                Ok(false) => (),
                Err(error) => errors.push(error.to_string()),
            }
        }
        if !self.reaped {
            match self.child.try_wait() {
                Ok(Some(_)) => self.reaped = true,
                Ok(None) => {
                    if self.owned.is_empty() {
                        errors.push("Go fixture root identity was unavailable".into());
                    }
                }
                Err(error) => errors.push(format!("cannot observe Go fixture root: {error}")),
            }
            if !self.reaped {
                if let Err(error) = self.child.kill() {
                    // A concurrent exit may make kill fail after try_wait.
                    match self.child.try_wait() {
                        Ok(Some(_)) => self.reaped = true,
                        _ => errors.push(format!("cannot terminate Go fixture root: {error}")),
                    }
                }
            }
            let end = Instant::now() + Duration::from_secs(60);
            while !self.reaped {
                match self.child.try_wait() {
                    Ok(Some(_)) => self.reaped = true,
                    Ok(None) => (),
                    Err(error) => {
                        errors.push(format!("cannot reap Go fixture root: {error}"));
                        break;
                    }
                }
                if !self.reaped && Instant::now() >= end {
                    errors.push(format!(
                        "Go fixture root {} did not exit after termination",
                        self.child.id()
                    ));
                    break;
                }
                thread::yield_now();
            }
        }
        let end = Instant::now() + Duration::from_secs(60);
        loop {
            let mut remaining = Vec::new();
            for identity in &self.owned {
                match crate::cleanup::capture(identity.pid) {
                    Ok(current) if current.as_ref() != Some(identity) => (),
                    _ => remaining.push(identity.pid),
                }
            }
            if remaining.is_empty() {
                break;
            }
            if Instant::now() >= end {
                errors.push(format!("Go fixture descendants not reaped: {remaining:?}"));
                break;
            }
            thread::yield_now();
        }
        if !errors.is_empty() {
            return Err(io::Error::other(errors.join("; ")));
        }
        self.clean = true;
        self.counts.cleaned.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    pub fn wait_with_output(&mut self) -> io::Result<Output> {
        self.wait_until(Instant::now() + Duration::from_secs(60))
    }

    pub fn wait_until(&mut self, end: Instant) -> io::Result<Output> {
        drop(self.child.stdin.take());
        let mut out = self.child.stdout.take();
        let mut err = self.child.stderr.take();
        nonblocking(out.as_ref())?;
        nonblocking(err.as_ref())?;
        let mut events = OutputEvents::new(out.as_ref(), err.as_ref())?;
        let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
        let mut status: Option<ExitStatus> = None;
        loop {
            self.observe()?;
            let registration_pending = events.watch(&self.owned)?;
            drain(&mut out, &mut stdout)?;
            drain(&mut err, &mut stderr)?;
            if status.is_none() {
                status = self.child.try_wait()?;
                self.reaped = status.is_some();
            }
            if let (Some(status), None, None) = (status, &out, &err) {
                self.stop()?;
                return Ok(Output {
                    status,
                    stdout,
                    stderr,
                });
            }
            if Instant::now() >= end {
                self.stop()?;
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!(
                        "fixture child {} did not exit or close its output pipes",
                        self.child.id()
                    ),
                ));
            }
            if registration_pending {
                // Darwin can reject a process filter just before waitpid
                // exposes exit. Re-observe that state rather than missing it.
                thread::yield_now();
            } else {
                events.wait(end)?;
            }
        }
    }
}

fn child_identity(parent: u32, pid: u32) -> io::Result<Option<ProcessIdentity>> {
    crate::cleanup::child_identity(parent, pid)
}
impl Drop for GoChild {
    fn drop(&mut self) {
        if let Err(error) = self.stop() {
            crate::cleanup::report_errors(vec![error.to_string()]);
        }
    }
}

// Pipe readiness and native process fork/exit notifications wake the owner;
// no fixed polling interval is part of a successful fixture run.
struct OutputEvents {
    queue: OwnedFd,
    watched: Vec<ProcessIdentity>,
}
impl OutputEvents {
    fn new(out: Option<&impl AsRawFd>, err: Option<&impl AsRawFd>) -> io::Result<Self> {
        let fd = unsafe { libc::kqueue() };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let mut events = Self {
            queue: unsafe { OwnedFd::from_raw_fd(fd) },
            watched: Vec::new(),
        };
        for fd in [out.map(AsRawFd::as_raw_fd), err.map(AsRawFd::as_raw_fd)]
            .into_iter()
            .flatten()
        {
            events.register(fd as usize, libc::EVFILT_READ, 0)?;
        }
        Ok(events)
    }

    fn register(&mut self, id: usize, filter: i16, flags: u32) -> io::Result<()> {
        let mut event: libc::kevent = unsafe { std::mem::zeroed() };
        event.ident = id as _;
        event.filter = filter;
        event.flags = libc::EV_ADD | libc::EV_ENABLE | libc::EV_CLEAR;
        event.fflags = flags;
        if unsafe {
            libc::kevent(
                self.queue.as_raw_fd(),
                &event,
                1,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
            )
        } < 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    fn watch(&mut self, identities: &[ProcessIdentity]) -> io::Result<bool> {
        let mut pending = false;
        for identity in identities {
            if self.watched.contains(identity) {
                continue;
            }
            if let Err(error) = self.register(
                identity.pid as usize,
                libc::EVFILT_PROC,
                libc::NOTE_FORK | libc::NOTE_EXIT,
            ) {
                // A process can exit between identity capture and registration.
                // The next root/pipe event still drives observation and cleanup.
                if error.raw_os_error() != Some(libc::ESRCH) {
                    return Err(error);
                }
                if crate::cleanup::capture(identity.pid)?.as_ref() == Some(identity) {
                    pending = true;
                }
                continue;
            }
            self.watched.push(identity.clone());
        }
        Ok(pending)
    }

    fn wait(&self, deadline: Instant) -> io::Result<()> {
        let duration = deadline.saturating_duration_since(Instant::now());
        let timeout = libc::timespec {
            tv_sec: duration.as_secs() as _,
            tv_nsec: duration.subsec_nanos() as _,
        };
        let mut event: libc::kevent = unsafe { std::mem::zeroed() };
        let result = unsafe {
            libc::kevent(
                self.queue.as_raw_fd(),
                std::ptr::null(),
                0,
                &mut event,
                1,
                &timeout,
            )
        };
        if result < 0 && io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }
}

fn nonblocking(pipe: Option<&impl AsRawFd>) -> io::Result<()> {
    if let Some(pipe) = pipe {
        let fd = pipe.as_raw_fd();
        // SAFETY: only this fixture's captured output descriptors are changed.
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}
fn drain(pipe: &mut Option<impl Read>, bytes: &mut Vec<u8>) -> io::Result<()> {
    let mut buffer = [0; 16384];
    for _ in 0..16 {
        let Some(reader) = pipe.as_mut() else {
            break;
        };
        match reader.read(&mut buffer) {
            Ok(0) => {
                *pipe = None;
                break;
            }
            Ok(n) => bytes.extend_from_slice(&buffer[..n]),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "owned_tests.rs"]
mod tests;

//! Direct Go children use the maintained identity-aware tree cleanup on every exit.
use arc_core::process_identity::ProcessIdentity;
use std::{
    io::{self, Read},
    os::fd::AsRawFd,
    process::{Child, Command, ExitStatus, Output},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

static STARTED: AtomicU64 = AtomicU64::new(0);
static CLEANED: AtomicU64 = AtomicU64::new(0);
pub fn counts() -> (u64, u64) {
    (
        STARTED.load(Ordering::SeqCst),
        CLEANED.load(Ordering::SeqCst),
    )
}

pub struct GoChild {
    pub child: Child,
    owned: Vec<ProcessIdentity>,
    reaped: bool,
    clean: bool,
}
impl GoChild {
    pub fn spawn(command: &mut Command) -> io::Result<Self> {
        let child = command.spawn()?;
        STARTED.fetch_add(1, Ordering::SeqCst);
        let mut guard = Self {
            child,
            owned: Vec::new(),
            reaped: false,
            clean: false,
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
        let end = Instant::now() + Duration::from_millis(100);
        loop {
            if identity.pid == self.child.id() {
                self.reaped |= self.child.try_wait()?.is_some();
                if self.reaped {
                    return Ok(false);
                }
            }
            match crate::cleanup::capture(identity.pid) {
                Ok(current) => return Ok(current.as_ref() == Some(identity)),
                Err(_) if Instant::now() < end => thread::sleep(Duration::from_millis(1)),
                Err(error) => return Err(error),
            }
        }
    }

    pub fn identities(&self) -> Vec<ProcessIdentity> {
        self.owned.clone()
    }

    /// Stop children before their parent; retain exact native identities and
    /// require disappearance, including zombie reaping, before reporting success.
    pub fn stop(&mut self) -> io::Result<()> {
        if self.clean {
            return Ok(());
        }
        let mut errors = self
            .observe()
            .err()
            .map(|e| e.to_string())
            .into_iter()
            .collect::<Vec<_>>();
        for identity in self.owned.clone().iter().rev() {
            match self.current(identity) {
                Ok(true) => {
                    errors.extend(crate::cleanup::kill_tree(identity, 0));
                    // Keep its parent alive long enough to reap this child.
                    // Otherwise a second tree walk can race its zombie transition.
                    let end = Instant::now() + Duration::from_secs(10);
                    while self.current(identity)? {
                        if Instant::now() >= end {
                            errors.push(format!("Go descendant not reaped: {}", identity.pid));
                            break;
                        }
                        thread::sleep(Duration::from_millis(1));
                    }
                }
                Ok(false) => (),
                Err(error) => errors.push(error.to_string()),
            }
        }
        if !self.reaped {
            if self.owned.is_empty() && self.child.try_wait()?.is_none() {
                self.child.kill()?;
                errors.push("Go fixture root identity was unavailable".into());
            }
            self.child.wait()?;
            self.reaped = true;
        }
        let end = Instant::now() + Duration::from_secs(10);
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
            thread::sleep(Duration::from_millis(1));
        }
        if !errors.is_empty() {
            return Err(io::Error::other(errors.join("; ")));
        }
        self.clean = true;
        CLEANED.fetch_add(1, Ordering::SeqCst);
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
        let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
        let mut status: Option<ExitStatus> = None;
        loop {
            self.observe()?;
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
                    "Go fixture exit/pipe watchdog",
                ));
            }
            thread::sleep(Duration::from_millis(1));
        }
    }
}

fn child_identity(parent: u32, pid: u32) -> io::Result<Option<ProcessIdentity>> {
    // A compiler can cross Darwin's live/zombie boundary between the two
    // identity reads. Wait for stable evidence; an error is never absence.
    let end = Instant::now() + Duration::from_millis(100);
    loop {
        match crate::cleanup::child_identity(parent, pid) {
            Ok(identity) => return Ok(identity),
            Err(_) if Instant::now() < end => thread::sleep(Duration::from_millis(1)),
            Err(error) => return Err(error),
        }
    }
}
impl Drop for GoChild {
    fn drop(&mut self) {
        if let Err(error) = self.stop() {
            crate::cleanup::report_errors(vec![error.to_string()]);
        }
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

//! Direct Go children use the maintained identity-aware tree cleanup on every exit.
use arc_core::process_identity::ProcessIdentity;
use std::{
    io::{self, Read},
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::process::CommandExt,
    },
    process::{Child, Command, Output, Stdio},
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
impl Counts {
    fn snapshot(&self) -> (u64, u64) {
        (
            self.started.load(Ordering::SeqCst),
            self.cleaned.load(Ordering::SeqCst),
        )
    }
}
thread_local! {
    static COUNTS: Arc<Counts> = Arc::new(Counts::default());
}
pub fn counts() -> (u64, u64) {
    COUNTS.with(|counts| counts.snapshot())
}

pub struct GoChild {
    pub child: Child,
    marker: marker::Marker,
    owned: Vec<ProcessIdentity>,
    reaped: bool,
    clean: bool,
    counts: Arc<Counts>,
}
impl GoChild {
    pub fn spawn(command: &mut Command) -> io::Result<Self> {
        Self::spawn_counted(command, COUNTS.with(Arc::clone))
    }

    fn spawn_counted(command: &mut Command, counts: Arc<Counts>) -> io::Result<Self> {
        // Reserve a private group before exec so an orphaned pipe holder can
        // still be identified while the direct child remains unreaped.
        command.process_group(0);
        let mut marker = marker::Marker::attach(command)?;
        let child = command.spawn()?;
        marker.close_parent();
        counts.started.fetch_add(1, Ordering::SeqCst);
        let mut guard = Self {
            child,
            marker,
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
        if self.exited_without_reaping()? {
            self.observe_marker()?;
        }
        self.observe_tree()
    }

    fn observe_marker(&mut self) -> io::Result<()> {
        for identity in self.marker.identities()? {
            if !self.owned.contains(&identity) {
                self.owned.push(identity);
            }
        }
        Ok(())
    }

    fn observe_all(&mut self) -> io::Result<()> {
        self.observe_marker()?;
        self.observe_tree()
    }

    fn observe_tree(&mut self) -> io::Result<()> {
        self.observe_group()?;
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
            if self.reaped {
                return Ok(false);
            }
            // Do not reap the group anchor until the final group observation.
            return Ok(!self.exited_without_reaping()?);
        }
        Ok(crate::cleanup::capture(identity.pid)?.as_ref() == Some(identity))
    }

    fn exited_without_reaping(&self) -> io::Result<bool> {
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        let result = unsafe {
            libc::waitid(
                libc::P_PID,
                self.child.id(),
                &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        if result != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(info.si_pid != 0
            && matches!(
                info.si_code,
                libc::CLD_EXITED | libc::CLD_KILLED | libc::CLD_DUMPED
            ))
    }

    fn observe_group(&mut self) -> io::Result<()> {
        if self.reaped {
            return Ok(());
        }
        // waitid confirms this is still our unreaped child, even after exit;
        // its reserved PID prevents the private group ID from being reused.
        match self.exited_without_reaping() {
            Ok(_) => (),
            Err(error) if error.raw_os_error() == Some(libc::ECHILD) => {
                self.reaped = self.child.try_wait()?.is_some();
                return Ok(());
            }
            Err(error) => return Err(error),
        }
        let mut ids = vec![0i32; 128];
        loop {
            let bytes = unsafe {
                *libc::__error() = 0;
                libc::proc_listpids(
                    2,
                    self.child.id(),
                    ids.as_mut_ptr().cast(),
                    std::mem::size_of_val(ids.as_slice()) as i32,
                )
            };
            if bytes < 0 || (bytes == 0 && io::Error::last_os_error().raw_os_error() != Some(0)) {
                return Err(io::Error::last_os_error());
            }
            let count = bytes as usize / std::mem::size_of::<i32>();
            if count == ids.len() {
                if ids.len() >= 16_384 {
                    return Err(io::Error::other("Go fixture process group limit"));
                }
                ids.resize(ids.len() * 2, 0);
                continue;
            }
            for pid in ids[..count].iter().copied().filter(|pid| *pid > 0) {
                if pid as u32 == self.child.id() {
                    continue;
                }
                let Some(identity) = crate::cleanup::capture(pid as u32)? else {
                    continue;
                };
                if unsafe { libc::getpgid(pid) } == self.child.id() as i32
                    && !self.owned.contains(&identity)
                {
                    self.owned.push(identity);
                }
            }
            return Ok(());
        }
    }

    pub fn identities(&self) -> Vec<ProcessIdentity> {
        self.owned.clone()
    }

    /// Stop children before their parent; retain exact native identities and
    /// require disappearance, including zombie reaping, before reporting success.
    pub fn stop(&mut self) -> io::Result<()> {
        self.stop_with(Self::observe_all, crate::cleanup::kill_tree)
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
        if !errors.is_empty() && !self.reaped {
            // Visit recorded identities before the group fallback: signalling
            // the group first can make the root exit before its tree is visited.
            // The unreaped child still reserves the group until it is reaped.
            match self.exited_without_reaping() {
                Ok(_) => {
                    if unsafe { libc::kill(-(self.child.id() as i32), libc::SIGKILL) } != 0 {
                        let error = io::Error::last_os_error();
                        if error.raw_os_error() != Some(libc::ESRCH) {
                            errors.push(format!("cannot terminate Go fixture group: {error}"));
                        }
                    }
                }
                Err(error) => errors.push(format!("cannot verify Go fixture group: {error}")),
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
            self.marker.retain_on_drop();
            return Err(io::Error::other(errors.join("; ")));
        }
        self.marker.remove_on_drop();
        self.clean = true;
        self.counts.cleaned.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }

    pub fn wait_with_output(&mut self) -> io::Result<Output> {
        self.wait_until(Instant::now() + Duration::from_secs(60))
    }

    pub fn wait_until(&mut self, end: Instant) -> io::Result<Output> {
        self.wait_with_clock(end, Instant::now, OutputEvents::wait)
    }

    fn wait_with_clock(
        &mut self,
        end: Instant,
        now: impl Fn() -> Instant,
        mut wait: impl FnMut(&OutputEvents, Instant) -> io::Result<()>,
    ) -> io::Result<Output> {
        drop(self.child.stdin.take());
        let mut out = self.child.stdout.take();
        let mut err = self.child.stderr.take();
        nonblocking(out.as_ref())?;
        nonblocking(err.as_ref())?;
        let mut events = OutputEvents::new(out.as_ref(), err.as_ref())?;
        let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
        loop {
            self.observe()?;
            drain(&mut out, &mut stdout)?;
            drain(&mut err, &mut stderr)?;
            if self.exited_without_reaping()? {
                // Keep the root unreaped as a group identity anchor until every
                // owned descendant finishes and both output streams reach EOF.
                self.observe_group()?;
                let mut descendants_running = false;
                for identity in self.owned.clone() {
                    if identity.pid != self.child.id() && self.current(&identity)? {
                        descendants_running = true;
                    }
                }
                if !descendants_running && out.is_none() && err.is_none() {
                    let status = self.child.try_wait()?.ok_or_else(|| {
                        io::Error::other("fixture root exit was not available to reap")
                    })?;
                    self.reaped = true;
                    self.stop()?;
                    return Ok(Output {
                        status,
                        stdout,
                        stderr,
                    });
                }
            }
            let current = now();
            if current >= end {
                let cleanup = self.stop().err();
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!(
                        "fixture child {} or its descendants did not exit and close output pipes; cleanup: {cleanup:?}",
                        self.child.id(),
                    ),
                ));
            }
            let registration_pending = events.watch(&self.owned)?;
            if registration_pending {
                // Darwin can reject a process filter just before waitpid
                // exposes exit. Re-observe that state rather than missing it.
                thread::yield_now();
            } else {
                // A child can exit while native notifications are being
                // registered. Recheck owned identities and pipe state even
                // if that notification was missed; keep the original
                // absolute failure watchdog unchanged.
                let recheck = current
                    .checked_add(Duration::from_millis(100))
                    .unwrap_or(end)
                    .min(end);
                wait(&events, recheck)?;
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

#[path = "owned_events.rs"]
mod events;
use events::{drain, nonblocking, OutputEvents};

#[path = "../../support/owned_marker.rs"]
mod marker;

#[cfg(test)]
#[path = "owned_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "owned_descriptor_tests.rs"]
mod descriptor_tests;

#[cfg(test)]
#[path = "owned_race_tests.rs"]
mod race_tests;

//! Own the complete native fixture tree from spawn, including before markers.
//! The macOS CLI puts each native target in a separate process group, so a
//! wrapper-only guard or a group kill cannot cover nested invocations.
#[path = "owned_tree_cleanup.rs"]
mod owned_tree_cleanup;
#[path = "owned_tree_monitor.rs"]
mod owned_tree_monitor;
#[path = "owned_tree_process.rs"]
mod owned_tree_process;

use arc_core::process_identity::ProcessIdentity;
pub(super) use owned_tree_cleanup::info;
use owned_tree_cleanup::{
    children, cleanup_uncaptured_root, confirmed_missing, freeze_session, freeze_tree_with,
    report_cleanup, terminate,
};
use owned_tree_monitor::Monitor;
use std::{
    fs,
    io::{self, Read},
    os::{
        fd::AsRawFd,
        unix::{fs::DirBuilderExt, process::CommandExt},
    },
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Output},
    thread,
    time::{Duration, Instant},
};

pub(super) const REGISTRY_ENV: &str = "ARC_TEST_OWNER_REGISTRY";
pub(super) const REGISTER_ENV: &str = "ARC_TEST_OWNER_REGISTER";

pub(super) struct OwnedTree {
    child: Child,
    root: ProcessIdentity,
    fixture: PathBuf,
    registry: PathBuf,
    status: Option<ExitStatus>,
    cleaned: bool,
    monitor: Option<Monitor>,
}

// A capture or observer setup panic must not discard the unreaped Child.
struct PendingChild {
    child: Option<Child>,
    fixture: PathBuf,
    registry: PathBuf,
}

impl Drop for PendingChild {
    fn drop(&mut self) {
        if let Some(child) = &mut self.child {
            let result = cleanup_uncaptured_root(child, &self.fixture, &ProcessIdentity::capture);
            if result.is_err() && !thread::panicking() {
                panic!("fixture tree cleanup incomplete");
            }
        }
        if !self.fixture.join("owned-cleanup-incomplete").exists() {
            let _ = fs::remove_dir_all(&self.registry);
        }
    }
}

impl OwnedTree {
    pub fn spawn(command: Command, fixture: &Path) -> Self {
        Self::spawn_with_mode(command, fixture, ProcessIdentity::capture, true)
    }

    pub(super) fn spawn_with_initial_capture(
        command: Command,
        fixture: &Path,
        capture: impl Fn(u32) -> io::Result<ProcessIdentity>,
    ) -> Self {
        Self::spawn_with_mode(command, fixture, capture, true)
    }

    pub(super) fn spawn_with_mode(
        mut command: Command,
        fixture: &Path,
        capture: impl Fn(u32) -> io::Result<ProcessIdentity>,
        observe_ancestry: bool,
    ) -> Self {
        let registry = command
            .get_envs()
            .find_map(|(key, value)| (key == REGISTRY_ENV).then_some(value).flatten())
            .map(PathBuf::from)
            .expect("fixture command requires an ownership registry");
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&registry)
            .expect("create private fixture ownership registry");
        command.env(REGISTER_ENV, std::env::current_exe().unwrap());
        // Establish a private session before the fixture can fork. Native ARC
        // children create separate process groups, but the controlled test
        // tools stay in this session even if their original parent exits.
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() < 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                let _ = fs::remove_dir_all(&registry);
                panic!("spawn owned fixture tree: {error}");
            }
        };
        let mut pending = PendingChild {
            child: Some(child),
            fixture: fixture.to_owned(),
            registry: registry.clone(),
        };
        let root = match capture(pending.child.as_ref().unwrap().id()) {
            Ok(root) => root,
            Err(error) => {
                let cleanup =
                    cleanup_uncaptured_root(pending.child.as_mut().unwrap(), fixture, &capture);
                pending.child.take();
                panic!("capture owned root identity: {error}; cleanup: {cleanup:?}");
            }
        };
        let mut tree = Self {
            child: pending.child.take().unwrap(),
            root: root.clone(),
            fixture: fixture.to_owned(),
            registry,
            status: None,
            cleaned: false,
            monitor: None,
        };
        // Construct the guard first, so a thread-creation failure also cleans up.
        tree.monitor = Some(
            Monitor::start(root, tree.registry.clone(), observe_ancestry)
                .expect("start fixture ownership observer"),
        );
        // Ownership of the registry moves to the completed guard.
        pending.registry = PathBuf::new();
        tree
    }

    /// Return identities retained by the independent observer, including any
    /// still-live descendants whose original parent has since exited.
    pub fn snapshot(&self) -> io::Result<Vec<ProcessIdentity>> {
        Ok(self
            .monitor
            .as_ref()
            .map_or_else(|| vec![self.root.clone()], Monitor::snapshot))
    }

    pub(super) fn recorded(&self) -> Vec<ProcessIdentity> {
        self.monitor
            .as_ref()
            .map_or_else(Vec::new, Monitor::recorded)
    }

    pub(super) fn root_id(&self) -> u32 {
        self.child.id()
    }

    pub(super) fn kill_root(&mut self) -> io::Result<()> {
        self.child.kill()
    }

    // Observe exit without reaping: the unreaped direct child reserves the
    // session ID even when its leader has exited and descendants are orphaned.
    pub(super) fn has_exited(&self) -> io::Result<bool> {
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        let result = unsafe {
            libc::waitid(
                libc::P_PID,
                self.child.id() as libc::id_t,
                &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        if result != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(unsafe { info.si_pid() } == self.child.id() as libc::pid_t)
    }

    pub fn wait_with_output(&mut self) -> io::Result<Output> {
        self.wait_with_output_for(Duration::from_secs(60))
    }

    pub(super) fn wait_with_output_for(&mut self, timeout: Duration) -> io::Result<Output> {
        let result = self.wait_inner(timeout);
        match (result, self.cleanup()) {
            (Ok(output), Ok(())) => Ok(output),
            (Err(error), Ok(())) => Err(error),
            (result, Err(cleanup)) => Err(io::Error::other(format!(
                "fixture cleanup incomplete: {cleanup}; execution={result:?}"
            ))),
        }
    }

    fn wait_inner(&mut self, timeout: Duration) -> io::Result<Output> {
        fn nonblocking(pipe: &Option<impl AsRawFd>) -> io::Result<()> {
            if let Some(pipe) = pipe {
                let fd = pipe.as_raw_fd();
                let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
                if flags < 0
                    || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0
                {
                    return Err(io::Error::last_os_error());
                }
            }
            Ok(())
        }
        fn drain(pipe: &mut Option<impl Read>, output: &mut Vec<u8>) -> io::Result<()> {
            let mut buf = [0; 8192];
            for _ in 0..16 {
                let Some(reader) = pipe.as_mut() else { break };
                match reader.read(&mut buf) {
                    Ok(0) => {
                        *pipe = None;
                        break;
                    }
                    Ok(n) => output.extend_from_slice(&buf[..n]),
                    Err(e)
                        if matches!(
                            e.kind(),
                            io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                        ) =>
                    {
                        break
                    }
                    Err(e) => return Err(e),
                }
            }
            Ok(())
        }
        drop(self.child.stdin.take());
        let (mut out, mut err) = (self.child.stdout.take(), self.child.stderr.take());
        nonblocking(&out)?;
        nonblocking(&err)?;
        let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
        let deadline = Instant::now() + timeout;
        loop {
            drain(&mut out, &mut stdout)?;
            drain(&mut err, &mut stderr)?;
            if self.has_exited()? && out.is_none() && err.is_none() {
                self.cleanup()?;
                return Ok(Output {
                    status: self.status.expect("cleanup reaped the exited fixture"),
                    stdout,
                    stderr,
                });
            }
            if Instant::now() >= deadline {
                // The wrapper may already have exited while an orphan still
                // owns a pipe. Keep the read ends until cleanup and verify EOF
                // so an unobserved pipe holder cannot be reported as cleaned.
                let cleanup = self.cleanup();
                let eof_deadline = Instant::now() + Duration::from_secs(1);
                while (out.is_some() || err.is_some()) && Instant::now() < eof_deadline {
                    drain(&mut out, &mut stdout)?;
                    drain(&mut err, &mut stderr)?;
                    if out.is_some() || err.is_some() {
                        thread::sleep(Duration::from_millis(5));
                    }
                }
                let pipes = if out.is_some() || err.is_some() {
                    report_cleanup(
                        &self.fixture,
                        &["output pipe remains open after fixture cleanup".into()],
                        &[],
                    )
                } else {
                    Ok(())
                };
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!("owned native fixture timed out; cleanup={cleanup:?}; pipes={pipes:?}"),
                ));
            }
            thread::sleep(Duration::from_millis(5));
        }
    }
}

impl OwnedTree {
    fn cleanup(&mut self) -> io::Result<()> {
        self.cleanup_with(&ProcessIdentity::capture, &info)
    }

    pub(super) fn cleanup_with(
        &mut self,
        capture: &impl Fn(u32) -> io::Result<ProcessIdentity>,
        read_info: &impl Fn(u32) -> Option<libc::proc_bsdinfo>,
    ) -> io::Result<()> {
        if self.cleaned {
            return Ok(());
        }
        self.cleaned = true;
        let mut observations = self.monitor.take().map(Monitor::finish).unwrap_or_default();
        if self.status.is_none() && !observations.identities.contains(&self.root) {
            observations.identities.insert(0, self.root.clone());
        }
        let mut owned = Vec::new();
        let mut errors = Vec::new();
        for identity in observations.identities {
            if let Err(error) = freeze_tree_with(identity, &mut owned, capture, read_info) {
                errors.push(error.to_string());
            }
        }
        if let Err(error) = freeze_session(
            &self.child,
            Some(&self.root),
            &mut owned,
            capture,
            read_info,
        ) {
            errors.push(error.to_string());
        }
        for (pid, error) in observations.enumeration_errors {
            errors.push(format!("cannot enumerate pid {pid}: {error}"));
        }
        let (remaining, root_error) = terminate(&mut self.child, &mut self.status, &owned);
        errors.extend(root_error);
        for (pid, issue) in observations.unresolved {
            if !confirmed_missing(pid) {
                errors.push(format!("pid {pid}: {issue}"));
            }
        }
        report_cleanup(&self.fixture, &errors, &remaining)?;
        fs::remove_dir_all(&self.registry).inspect_err(|error| {
            let _ = report_cleanup(
                &self.fixture,
                &[format!("remove ownership registry: {error}")],
                &[],
            );
        })
    }
}

impl Drop for OwnedTree {
    fn drop(&mut self) {
        if self.cleanup().is_err() && !thread::panicking() {
            panic!("fixture tree cleanup incomplete");
        }
    }
}

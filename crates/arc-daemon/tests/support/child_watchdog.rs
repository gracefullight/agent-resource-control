//! Diagnostic deadlines and bounded cleanup for fixture-owned child processes.
#![allow(dead_code)] // Each test binary uses only its required subset.
use std::{
    io::{self, Read},
    os::{fd::AsRawFd, unix::process::CommandExt},
    process::{Child, Command, ExitStatus, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

pub(crate) const WATCHDOG: Duration = Duration::from_secs(60);
const CLEANUP: Duration = Duration::from_secs(5);

/// The caller owns the child from spawn through reaping, including on unwind.
pub struct Owned {
    pub child: Child,
    group: bool,
    armed: bool,
}

impl Owned {
    pub fn spawn(command: &mut Command) -> io::Result<Self> {
        command.process_group(0);
        Ok(Self {
            child: command.spawn()?,
            group: true,
            armed: true,
        })
    }

    pub fn wait(&mut self, label: &str) -> io::Result<ExitStatus> {
        self.wait_for(label, WATCHDOG)
    }

    pub(crate) fn wait_for(&mut self, label: &str, limit: Duration) -> io::Result<ExitStatus> {
        self.wait_for_with_cleanup(label, limit, || {})
    }

    /// Run owner-specific cleanup before the wrapper group is killed and
    /// reaped, while separately grouped descendants can still be identified.
    pub(crate) fn wait_for_with_cleanup(
        &mut self,
        label: &str,
        limit: Duration,
        before_cleanup: impl FnOnce(),
    ) -> io::Result<ExitStatus> {
        let deadline = Instant::now() + limit;
        loop {
            if let Some(status) = self.child.try_wait()? {
                self.armed = false;
                return Ok(status);
            }
            if Instant::now() >= deadline {
                before_cleanup();
                self.cleanup();
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    format!(
                        "{label}: child {} exceeded {limit:?} watchdog",
                        self.child.id()
                    ),
                ));
            }
            thread::sleep(Duration::from_millis(2));
        }
    }

    fn cleanup(&mut self) {
        if self.armed {
            self.armed = false;
            cleanup(&mut self.child, self.group);
        }
    }
}

impl Drop for Owned {
    fn drop(&mut self) {
        self.cleanup();
    }
}

fn cleanup(child: &mut Child, group: bool) {
    if group {
        // The group was created specifically for this child at spawn.
        let result = unsafe { libc::kill(-(child.id() as libc::pid_t), libc::SIGKILL) };
        if result != 0 && io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH) {
            eprintln!(
                "fixture child {} group cleanup failed: {}",
                child.id(),
                io::Error::last_os_error()
            );
        }
    }
    match child.try_wait() {
        Ok(Some(_)) => return,
        Ok(None) => {
            if let Err(error) = child.kill() {
                eprintln!("fixture child {} kill failed: {error}", child.id());
            }
        }
        Err(error) => {
            eprintln!(
                "fixture child {} status failed during cleanup: {error}",
                child.id()
            );
            return;
        }
    }
    let deadline = Instant::now() + CLEANUP;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => return,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(2)),
            Ok(None) => {
                eprintln!(
                    "fixture child {} could not be reaped within {CLEANUP:?}",
                    child.id()
                );
                return;
            }
            Err(error) => {
                eprintln!("fixture child {} reap failed: {error}", child.id());
                return;
            }
        }
    }
}

/// Used where an existing caller already owns the Child handle.
pub fn wait_child(child: &mut Child, label: &str) -> io::Result<ExitStatus> {
    wait_child_for(child, label, WATCHDOG)
}

pub(crate) fn wait_child_for(
    child: &mut Child,
    label: &str,
    limit: Duration,
) -> io::Result<ExitStatus> {
    struct Borrowed<'a>(&'a mut Child, bool);
    impl Drop for Borrowed<'_> {
        fn drop(&mut self) {
            if self.1 {
                cleanup(self.0, false);
            }
        }
    }
    let mut guard = Borrowed(child, true);
    let deadline = Instant::now() + limit;
    loop {
        if let Some(status) = guard.0.try_wait()? {
            guard.1 = false;
            return Ok(status);
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!(
                    "{label}: child {} exceeded {limit:?} watchdog",
                    guard.0.id()
                ),
            ));
        }
        thread::sleep(Duration::from_millis(2));
    }
}

pub fn status(command: &mut Command, label: &str) -> io::Result<ExitStatus> {
    Owned::spawn(command)?.wait(label)
}

/// Read both pipes without waiting for EOF or exit beyond the same deadline.
pub fn output(command: &mut Command, label: &str) -> io::Result<Output> {
    output_for(command, label, WATCHDOG)
}

pub(crate) fn output_for(
    command: &mut Command,
    label: &str,
    limit: Duration,
) -> io::Result<Output> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut owner = Owned::spawn(command)?;
    let mut stdout = owner.child.stdout.take();
    let mut stderr = owner.child.stderr.take();
    nonblocking(stdout.as_ref().unwrap())?;
    nonblocking(stderr.as_ref().unwrap())?;
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let mut status = None;
    let deadline = Instant::now() + limit;
    loop {
        read_available(&mut stdout, &mut out)?;
        read_available(&mut stderr, &mut err)?;
        if status.is_none() {
            status = owner.child.try_wait()?;
        }
        if let Some(status) = status {
            if stdout.is_none() && stderr.is_none() {
                owner.armed = false;
                return Ok(Output {
                    status,
                    stdout: out,
                    stderr: err,
                });
            }
        }
        if Instant::now() >= deadline {
            owner.cleanup();
            return Err(io::Error::new(io::ErrorKind::TimedOut, format!(
                "{label}: child {} or its output pipes exceeded {limit:?} watchdog; stdout={:?}; stderr={:?}",
                owner.child.id(), String::from_utf8_lossy(&out), String::from_utf8_lossy(&err)
            )));
        }
        thread::sleep(Duration::from_millis(2));
    }
}

fn nonblocking(stream: &impl AsRawFd) -> io::Result<()> {
    let fd = stream.as_raw_fd();
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn read_available<R: Read>(stream: &mut Option<R>, bytes: &mut Vec<u8>) -> io::Result<()> {
    let Some(reader) = stream.as_mut() else {
        return Ok(());
    };
    let mut buffer = [0; 8192];
    // Bound each drain pass so a noisy child cannot starve the watchdog.
    for _ in 0..16 {
        match reader.read(&mut buffer) {
            Ok(0) => {
                *stream = None;
                return Ok(());
            }
            Ok(size) => bytes.extend_from_slice(&buffer[..size]),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

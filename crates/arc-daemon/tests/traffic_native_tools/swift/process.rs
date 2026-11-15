//! Direct SwiftPM children retain their native parent identity until tree cleanup.
//! No watchdog or output reader can reap the parent before its descendants are owned.
use arc_core::process_identity::ProcessIdentity;
use std::{
    io::{self, Read},
    os::{fd::AsRawFd, unix::process::CommandExt},
    process::{Child, Command, ExitStatus, Output},
    thread,
    time::{Duration, Instant},
};

pub struct TreeChild {
    pub child: Child,
    root: ProcessIdentity,
    owned: Vec<ProcessIdentity>,
    status: Option<ExitStatus>,
    cleaned: bool,
}

impl TreeChild {
    pub fn spawn(mut command: Command) -> io::Result<Self> {
        // Establish an owned group before exec, including the first fork interval.
        command.process_group(0);
        let mut child = command.spawn()?;
        let root = match ProcessIdentity::capture(child.id()) {
            Ok(root) => root,
            Err(error) => {
                // Child is still ours and unreaped, so this group cannot be reused.
                unsafe { libc::kill(-(child.id() as i32), libc::SIGKILL) };
                let _ = child.wait();
                return Err(error);
            }
        };
        Ok(Self {
            child,
            owned: vec![root.clone()],
            root,
            status: None,
            cleaned: false,
        })
    }

    pub fn wait_with_output(&mut self, timeout: Duration) -> io::Result<Output> {
        drop(self.child.stdin.take());
        let (mut out, mut err) = (self.child.stdout.take(), self.child.stderr.take());
        nonblocking(out.as_ref())?;
        nonblocking(err.as_ref())?;
        let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
        let end = Instant::now() + timeout;
        loop {
            drain(&mut out, &mut stdout)?;
            drain(&mut err, &mut stderr)?;
            if self.status.is_none() && self.exited_without_reaping()? {
                self.cleanup()?;
            }
            if let (Some(status), None, None) = (self.status, &out, &err) {
                return Ok(Output {
                    status,
                    stdout,
                    stderr,
                });
            }
            if Instant::now() >= end {
                // Cleanup runs while waitid(WNOWAIT) still retains the root identity.
                self.cleanup()?;
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Swift native tree watchdog",
                ));
            }
            thread::sleep(Duration::from_millis(1));
        }
    }

    fn exited_without_reaping(&self) -> io::Result<bool> {
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        // WNOWAIT preserves the parent PID and tree anchor until cleanup finishes.
        let result = unsafe {
            libc::waitid(
                libc::P_PID,
                self.root.pid,
                &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        if result == 0 {
            Ok(info.si_pid != 0)
        } else {
            Err(io::Error::last_os_error())
        }
    }

    pub fn has_exited(&self) -> io::Result<bool> {
        self.exited_without_reaping()
    }

    fn capture_tree(&mut self, identity: &ProcessIdentity, depth: usize) -> io::Result<()> {
        if depth >= 128 {
            return Err(io::Error::other("Swift fixture descendant depth limit"));
        }
        if crate::cleanup::capture(identity.pid)?.as_ref() != Some(identity) {
            return Ok(());
        }
        if !self.owned.contains(identity) {
            self.owned.push(identity.clone());
        }
        signal(identity, libc::SIGSTOP)?;
        for pid in crate::cleanup::children(identity.pid)? {
            let Some(child) = crate::cleanup::capture(pid)? else {
                continue;
            };
            // Verify ancestry after capture so a recycled enumeration cannot be owned.
            let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
            let size = std::mem::size_of_val(&info) as i32;
            let n = unsafe {
                libc::proc_pidinfo(
                    pid as i32,
                    libc::PROC_PIDTBSDINFO,
                    0,
                    (&mut info as *mut libc::proc_bsdinfo).cast(),
                    size,
                )
            };
            if n != size {
                if crate::cleanup::capture(pid)?.is_none() {
                    continue;
                }
                return Err(io::Error::last_os_error());
            }
            if info.pbi_ppid == identity.pid
                && crate::cleanup::capture(pid)?.as_ref() == Some(&child)
            {
                self.capture_tree(&child, depth + 1)?;
            }
        }
        Ok(())
    }

    fn group_members(&self) -> io::Result<Vec<u32>> {
        let mut ids = vec![0i32; 128];
        loop {
            // PROC_PGRP_ONLY observes only the group established before this spawn.
            let bytes = unsafe {
                *libc::__error() = 0;
                libc::proc_listpids(
                    2,
                    self.root.pid,
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
                    return Err(io::Error::other("Swift group fixture limit"));
                }
                ids.resize(ids.len() * 2, 0);
                continue;
            }
            return Ok(ids[..count]
                .iter()
                .copied()
                .filter(|pid| *pid > 0)
                .map(|pid| pid as u32)
                .collect());
        }
    }

    fn capture_group(&mut self) -> io::Result<()> {
        for pid in self.group_members()? {
            if pid == self.root.pid {
                continue;
            }
            let Some(identity) = crate::cleanup::capture(pid)? else {
                continue;
            };
            if unsafe { libc::getpgid(pid as i32) } == self.root.pid as i32
                && !self.owned.contains(&identity)
            {
                self.owned.push(identity);
            }
        }
        Ok(())
    }

    fn signal_group(&self, value: i32) -> io::Result<()> {
        // Darwin rejects group signals to a zombie leader. Keep it unreaped,
        // enumerate its retained group, and stop/terminate each verified member
        // through capture_tree/signal instead. No denied signal is suppressed.
        if self.exited_without_reaping()? {
            return Ok(());
        }
        if unsafe { libc::getpgid(self.root.pid as i32) } != self.root.pid as i32 {
            return Err(io::Error::other("Swift child lost its owned process group"));
        }
        if unsafe { libc::kill(-(self.root.pid as i32), value) } != 0 {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(libc::ESRCH) {
                return Err(error);
            }
        }
        Ok(())
    }

    fn cleanup(&mut self) -> io::Result<()> {
        if self.cleaned {
            return Ok(());
        }
        let mut errors = Vec::new();
        // Freeze the owned group before enumeration. Also follow separately grouped
        // descendants by verified ancestry; never scan or signal unrelated processes.
        if crate::cleanup::capture(self.root.pid)?.as_ref() == Some(&self.root) {
            if let Err(error) = self.signal_group(libc::SIGSTOP) {
                errors.push(format!("stop group: {error}"));
            }
            if let Err(error) = self.capture_group() {
                errors.push(format!("capture group: {error}"));
            }
            // Group members can outlive and be reparented away from an exited root.
            // Capture their trees too, before reaping the group leader.
            for identity in self.owned.clone() {
                if let Err(error) = self.capture_tree(&identity, 0) {
                    errors.push(format!("capture tree {}: {error}", identity.pid));
                }
            }
            for identity in self.owned.iter().rev() {
                if let Err(error) = signal(identity, libc::SIGKILL) {
                    errors.push(format!("kill {}: {error}", identity.pid));
                }
            }
            // The unreaped Child still owns this group even if it has just exited.
            if let Err(error) = self.signal_group(libc::SIGKILL) {
                errors.push(format!("kill group: {error}"));
            }
        }
        self.status = Some(self.child.wait()?);
        let end = Instant::now() + Duration::from_secs(60);
        loop {
            let mut remaining = false;
            for identity in &self.owned {
                match crate::cleanup::capture(identity.pid) {
                    Ok(Some(current)) if current == *identity => remaining = true,
                    Ok(_) => (),
                    // Native process records can be transiently unavailable during
                    // exit. Retain ownership and retry; never call that disappearance.
                    Err(_) => remaining = true,
                }
            }
            if !remaining {
                break;
            }
            if Instant::now() >= end {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Swift descendants did not disappear",
                ));
            }
            thread::sleep(Duration::from_millis(1));
        }
        self.cleaned = true;
        if errors.is_empty() {
            Ok(())
        } else {
            Err(io::Error::other(errors.join("; ")))
        }
    }
}

impl Drop for TreeChild {
    fn drop(&mut self) {
        if let Err(error) = self.cleanup() {
            crate::cleanup::report_errors(vec![format!("Swift direct process tree: {error}")]);
        }
    }
}

fn signal(identity: &ProcessIdentity, value: i32) -> io::Result<()> {
    if crate::cleanup::capture(identity.pid)?.as_ref() == Some(identity)
        && unsafe { libc::kill(identity.pid as i32, value) } != 0
    {
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(libc::ESRCH) {
            return Err(error);
        }
    }
    Ok(())
}
fn nonblocking(pipe: Option<&impl AsRawFd>) -> io::Result<()> {
    if let Some(pipe) = pipe {
        let fd = pipe.as_raw_fd();
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
#[path = "process_tests.rs"]
mod tests;

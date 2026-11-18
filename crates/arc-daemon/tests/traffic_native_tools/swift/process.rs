//! Direct SwiftPM children retain their native parent identity until tree cleanup.
//! No watchdog or output reader can reap the parent before its descendants are owned.
use arc_core::process_identity::ProcessIdentity;
use std::{
    io,
    os::unix::process::CommandExt,
    process::{Child, Command, ExitStatus, Output},
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
                let _ = crate::native::native_readiness::kill_and_reap(&mut child, false);
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
        self.wait_for_output(Some(Instant::now() + timeout))
    }

    pub fn wait_with_output_until_exit(&mut self) -> io::Result<Output> {
        self.wait_for_output(Some(Instant::now() + Duration::from_secs(180)))
    }

    fn wait_for_output(&mut self, end: Option<Instant>) -> io::Result<Output> {
        drop(self.child.stdin.take());
        let (mut out, mut err) = (self.child.stdout.take(), self.child.stderr.take());
        nonblocking(out.as_ref())?;
        nonblocking(err.as_ref())?;
        let events = OutputEvents::new(self.root.pid, out.as_ref(), err.as_ref(), self)?;
        let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
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
            if end.is_some_and(|deadline| Instant::now() >= deadline) {
                // Cleanup runs while waitid(WNOWAIT) still retains the root identity.
                self.cleanup()?;
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "Swift native tree watchdog",
                ));
            }
            events.wait(end)?;
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
            // Darwin can return a pending stop event even for WEXITED. The
            // PID alone therefore does not prove that the leader has exited.
            Ok(events::is_terminal_event(&info))
        } else {
            Err(io::Error::last_os_error())
        }
    }

    pub fn has_exited(&self) -> io::Result<bool> {
        self.exited_without_reaping()
    }

    fn capture_tree_with(
        &mut self,
        identity: &ProcessIdentity,
        depth: usize,
        visit: &mut impl FnMut(&ProcessIdentity) -> io::Result<()>,
    ) -> io::Result<()> {
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
        visit(identity)?;
        let mut errors = Vec::new();
        for pid in events::owned_children(identity)? {
            let result: io::Result<()> = (|| {
                let Some(child) = crate::cleanup::capture(pid)? else {
                    return Ok(());
                };
                // Verify ancestry after capture so a recycled enumeration cannot be owned.
                if events::child_parent(&child)? == Some(identity.pid)
                    && crate::cleanup::capture(pid)?.as_ref() == Some(&child)
                {
                    self.capture_tree_with(&child, depth + 1, visit)?;
                }
                Ok(())
            })();
            if let Err(error) = result {
                errors.push(format!("descendant {pid}: {error}"));
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(io::Error::other(errors.join("; ")))
        }
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

    fn capture_group_with(
        &mut self,
        capture: &mut impl FnMut(u32) -> io::Result<Option<ProcessIdentity>>,
    ) -> io::Result<()> {
        let mut errors = Vec::new();
        for pid in self.group_members()? {
            if pid == self.root.pid {
                continue;
            }
            let identity = match capture(pid) {
                Ok(Some(identity)) => identity,
                Ok(None) => continue,
                Err(error) => {
                    errors.push(format!("group member {pid}: {error}"));
                    continue;
                }
            };
            if unsafe { libc::getpgid(pid as i32) } == self.root.pid as i32
                && !self.owned.contains(&identity)
            {
                self.owned.push(identity);
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(io::Error::other(errors.join("; ")))
        }
    }

    fn signal_group(&self, value: i32) -> io::Result<()> {
        // Darwin rejects group signals to a zombie leader. Keep it unreaped,
        // enumerate its retained group, and stop/terminate each verified member
        // through capture_tree/signal instead. No denied signal is suppressed.
        events::signal_owned_group(
            self.root.pid,
            || self.exited_without_reaping(),
            || {
                let group = unsafe { libc::getpgid(self.root.pid as i32) };
                if group == -1 {
                    Err(io::Error::last_os_error())
                } else {
                    Ok(group)
                }
            },
            || {
                if unsafe { libc::kill(-(self.root.pid as i32), value) } != 0 {
                    Err(io::Error::last_os_error())
                } else {
                    Ok(())
                }
            },
        )
    }

    fn cleanup(&mut self) -> io::Result<()> {
        self.cleanup_with(|child| child.signal_group(libc::SIGSTOP))
    }

    fn cleanup_with(&mut self, stop_group: impl FnOnce(&Self) -> io::Result<()>) -> io::Result<()> {
        self.cleanup_with_observer(stop_group, &mut |_| Ok(()))
    }

    fn cleanup_with_observer(
        &mut self,
        stop_group: impl FnOnce(&Self) -> io::Result<()>,
        visit: &mut impl FnMut(&ProcessIdentity) -> io::Result<()>,
    ) -> io::Result<()> {
        self.cleanup_with_observers(stop_group, visit, &mut crate::cleanup::capture)
    }

    fn cleanup_with_observers(
        &mut self,
        stop_group: impl FnOnce(&Self) -> io::Result<()>,
        visit: &mut impl FnMut(&ProcessIdentity) -> io::Result<()>,
        capture_group: &mut impl FnMut(u32) -> io::Result<Option<ProcessIdentity>>,
    ) -> io::Result<()> {
        self.cleanup_with_termination(stop_group, visit, capture_group, |child| {
            child.signal_group(libc::SIGKILL)
        })
    }

    fn cleanup_with_termination(
        &mut self,
        stop_group: impl FnOnce(&Self) -> io::Result<()>,
        visit: &mut impl FnMut(&ProcessIdentity) -> io::Result<()>,
        capture_group: &mut impl FnMut(u32) -> io::Result<Option<ProcessIdentity>>,
        kill_group: impl FnOnce(&Self) -> io::Result<()>,
    ) -> io::Result<()> {
        if self.cleaned {
            return Ok(());
        }
        let mut errors = Vec::new();
        // Freeze the owned group before enumeration. Also follow separately grouped
        // descendants by verified ancestry; never scan or signal unrelated processes.
        let root_current = match crate::cleanup::capture(self.root.pid) {
            Ok(identity) => identity.as_ref() == Some(&self.root),
            Err(error) => {
                errors.push(format!("capture root: {error}"));
                false
            }
        };
        if root_current {
            match stop_group(self) {
                Ok(()) => {
                    if let Err(error) = events::wait_for_group_leader_stop(self.root.pid) {
                        errors.push(format!("observe group stop: {error}"));
                    }
                }
                Err(error) => errors.push(format!("stop group: {error}")),
            }
            if let Err(error) = self.capture_group_with(capture_group) {
                errors.push(format!("capture group: {error}"));
            }
        }
        // Known identities remain ours even when the root observation failed.
        // Do not abandon a sibling subtree after another subtree reports an error.
        for identity in self.owned.clone() {
            if let Err(error) = self.capture_tree_with(&identity, 0, visit) {
                errors.push(format!("capture tree {}: {error}", identity.pid));
            }
        }
        // Preserve the direct child as the group anchor until the group signal.
        // Killing it first races Darwin's transition to a zombie leader, where
        // an otherwise valid group signal can fail with EPERM.
        for identity in self.owned.iter().rev().filter(|id| id.pid != self.root.pid) {
            if let Err(error) = signal(identity, libc::SIGKILL) {
                errors.push(format!("kill {}: {error}", identity.pid));
            }
        }
        if root_current {
            // If the root has exited, signal_owned_group observes that state;
            // enumerated members above still receive individual termination.
            if let Err(error) = kill_group(self) {
                errors.push(format!("kill group: {error}"));
            }
        }
        if !errors.is_empty() && self.status.is_none() {
            // Even if a verified group/descendant signal failed, still attempt
            // to reap the direct child before reporting the original failure.
            match self.child.try_wait() {
                Ok(Some(status)) => self.status = Some(status),
                Ok(None) => {
                    if let Err(error) = self.child.kill() {
                        errors.push(format!("kill direct child: {error}"));
                    }
                }
                Err(error) => errors.push(format!("observe direct child: {error}")),
            }
        }
        let end = Instant::now() + Duration::from_secs(60);
        loop {
            if self.status.is_some() {
                break;
            }
            match self.child.try_wait() {
                Ok(Some(status)) => {
                    self.status = Some(status);
                    break;
                }
                Ok(None) => (),
                Err(error) => {
                    errors.push(format!("reap direct child: {error}"));
                    break;
                }
            }
            if Instant::now() >= end {
                errors.push(format!(
                    "Swift direct child {} never exited during cleanup",
                    self.root.pid
                ));
                break;
            }
            std::thread::yield_now();
        }
        match wait_for_owned_exit(
            &self.owned,
            Duration::from_secs(60),
            crate::cleanup::capture,
        ) {
            Ok(()) if self.status.is_some() => self.cleaned = true,
            Ok(()) => (),
            Err(error) => errors.push(error.to_string()),
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(io::Error::other(errors.join("; ")))
        }
    }
}

fn wait_for_owned_exit(
    owned: &[ProcessIdentity],
    timeout: Duration,
    mut capture: impl FnMut(u32) -> io::Result<Option<ProcessIdentity>>,
) -> io::Result<()> {
    let end = Instant::now() + timeout;
    let mut errors = Vec::new();
    loop {
        let mut remaining = Vec::new();
        for identity in owned {
            match capture(identity.pid) {
                Ok(Some(current)) if current == *identity => remaining.push(identity.pid),
                Ok(_) => (),
                Err(error) => {
                    let message = format!(
                        "cannot verify cleanup of Swift descendant {}: {error}",
                        identity.pid
                    );
                    if !errors.contains(&message) {
                        errors.push(message);
                    }
                }
            }
        }
        if remaining.is_empty() {
            return if errors.is_empty() {
                Ok(())
            } else {
                Err(io::Error::other(errors.join("; ")))
            };
        }
        if Instant::now() >= end {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!(
                    "Swift descendants never exited during cleanup: {remaining:?}; {}",
                    errors.join("; ")
                ),
            ));
        }
        std::thread::yield_now();
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
#[path = "process_events.rs"]
mod events;
#[cfg(test)]
use events::recover_process_registration;
use events::{drain, nonblocking, OutputEvents};

#[cfg(test)]
#[path = "process_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "process_group_tests.rs"]
mod group_tests;

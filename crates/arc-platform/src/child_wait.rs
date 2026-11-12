//! PID-only terminal accounting for the caller's owned child.
//!
//! The caller remains the sole reaper and owns its signal mask, forwarding and
//! stop/continue policy. This boundary never discovers a process group or tree.
use std::{io, os::unix::process::ExitStatusExt, process::ExitStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaitMode {
    Blocking,
    Poll,
}

/// Raw timeval fields are retained for native calibration and explicit units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuTimeval {
    pub seconds: i64,
    pub microseconds: i64,
}
impl CpuTimeval {
    pub fn core_seconds(self) -> Result<f64, UsageError> {
        if self.seconds < 0 || !(0..1_000_000).contains(&self.microseconds) {
            return Err(UsageError::InvalidTimeval);
        }
        let value = self.seconds as f64 + self.microseconds as f64 / 1_000_000.0;
        (value <= arc_core::meter::MAX_SAFE_INTEGER as f64)
            .then_some(value)
            .ok_or(UsageError::InvalidTimeval)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsageError {
    InvalidTimeval,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerminalUsage {
    pub raw_user: CpuTimeval,
    pub raw_system: CpuTimeval,
    pub user_core_seconds: f64,
    pub system_core_seconds: f64,
    pub total_core_seconds: f64,
}
impl TerminalUsage {
    pub fn from_timevals(user: CpuTimeval, system: CpuTimeval) -> Result<Self, UsageError> {
        let user_core_seconds = user.core_seconds()?;
        let system_core_seconds = system.core_seconds()?;
        let total_core_seconds = user_core_seconds + system_core_seconds;
        if total_core_seconds > arc_core::meter::MAX_SAFE_INTEGER as f64 {
            return Err(UsageError::InvalidTimeval);
        }
        Ok(Self {
            raw_user: user,
            raw_system: system,
            user_core_seconds,
            system_core_seconds,
            total_core_seconds,
        })
    }
}

#[derive(Debug)]
pub struct ChildWaitEvent {
    /// Exact native status, including stop/continue and core-dump information.
    pub status: ExitStatus,
    /// Only terminal events carry usage. Conversion failure never loses status.
    pub terminal_usage: Option<Result<TerminalUsage, UsageError>>,
}

/// Reap/observe only this positive child PID; zero and process-group selectors
/// are rejected. `None` means WNOHANG found no event. EINTR remains an I/O error
/// for the existing waiter to handle under its existing signal policy.
///
/// On macOS terminal wait4 usage includes the child and descendants whose usage
/// has propagated through explicit waits. It is not an arbitrary process-tree
/// total. Never add sampled or overlapping nested totals to this inclusive value.
pub fn wait_owned_child(pid: u32, mode: WaitMode) -> io::Result<Option<ChildWaitEvent>> {
    if pid == 0 || pid > i32::MAX as u32 {
        return Err(io::ErrorKind::InvalidInput.into());
    }
    let options = libc::WUNTRACED
        | libc::WCONTINUED
        | if mode == WaitMode::Poll {
            libc::WNOHANG
        } else {
            0
        };
    let mut status = 0;
    // SAFETY: exact initialized output storage, valid options and positive PID.
    let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
    let observed = unsafe { libc::wait4(pid as i32, &mut status, options, &mut usage) };
    if observed < 0 {
        return Err(io::Error::last_os_error());
    }
    if observed == 0 {
        return Ok(None);
    }
    let terminal_usage = if libc::WIFEXITED(status) || libc::WIFSIGNALED(status) {
        let timeval = |value: libc::timeval| CpuTimeval {
            seconds: value.tv_sec,
            microseconds: value.tv_usec as i64,
        };
        Some(TerminalUsage::from_timevals(
            timeval(usage.ru_utime),
            timeval(usage.ru_stime),
        ))
    } else {
        None
    };
    Ok(Some(ChildWaitEvent {
        status: ExitStatus::from_raw(status),
        terminal_usage,
    }))
}

#[cfg(test)]
#[path = "child_wait_tests.rs"]
mod tests;

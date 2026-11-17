//! Expose the registered adapters to their isolated fixture binaries without
//! compiling the same vendor module twice in one test target.
use super::{HookContext, InstallAction, Scope, VerifiedSurface};
use serde_json::Value;
use std::{
    io::{self, Write},
    os::{
        fd::AsRawFd,
        unix::{net::UnixStream, process::CommandExt},
    },
    path::Path,
    process::{Child, Command, ExitStatus},
    time::{Duration, Instant},
};

const WORKER_TOKEN: &str = "ARC_HOOK_TEST_WORKER_TOKEN";
const WORKER_FD: i32 = 9;
const WATCHDOG: Duration = Duration::from_secs(120);

/// Prepare a private, pre-sent handshake. Keep the returned endpoint alive
/// until the child has been spawned. Ambient environment is not authorization
/// to run an isolated test worker.
pub(in super::super) fn worker_handshake(command: &mut Command) -> UnixStream {
    let (mut sender, receiver) = UnixStream::pair().unwrap();
    let token = arc_core::process_identity::random_job_id().unwrap();
    let hello = format!("arc-hook-test-worker-v1:{token}");
    command.env(WORKER_TOKEN, token);
    sender.write_all(hello.as_bytes()).unwrap();
    drop(sender);
    let fd = receiver.as_raw_fd();
    // SAFETY: only async-signal-safe descriptor syscalls run after fork.
    unsafe {
        command.pre_exec(move || {
            if libc::dup2(fd, WORKER_FD) < 0 || libc::fcntl(WORKER_FD, libc::F_SETFD, 0) < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    receiver
}

/// The ordinary test runner executes this entry without running the worker;
/// only a parent holding the private socket can activate its fixture body.
pub(in super::super) fn worker_enabled() -> bool {
    let Ok(token) = std::env::var(WORKER_TOKEN) else {
        return false;
    };
    if token.len() != 32 || !token.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return false;
    }
    let expected = format!("arc-hook-test-worker-v1:{token}");
    let mut bytes = vec![0u8; expected.len()];
    // SAFETY: recv borrows fd 9 and never blocks. An unrelated descriptor is
    // left untouched when it does not contain the complete handshake.
    let length = unsafe {
        libc::recv(
            WORKER_FD,
            bytes.as_mut_ptr().cast(),
            bytes.len(),
            libc::MSG_PEEK | libc::MSG_DONTWAIT,
        )
    };
    if length != bytes.len() as isize || bytes != expected.as_bytes() {
        return false;
    }
    let length = unsafe {
        libc::recv(
            WORKER_FD,
            bytes.as_mut_ptr().cast(),
            bytes.len(),
            libc::MSG_DONTWAIT,
        )
    };
    unsafe { libc::close(WORKER_FD) };
    length == bytes.len() as isize && bytes == expected.as_bytes()
}

/// Fixture children run in a private process group. Deadlines diagnose hangs;
/// successful tests proceed only after observing the child's actual exit.
pub(in super::super) struct OwnedChild {
    child: Option<Child>,
    label: String,
    own_group: bool,
}

impl OwnedChild {
    pub(in super::super) fn spawn(command: &mut Command, label: &str) -> Self {
        command.process_group(0);
        Self {
            child: Some(command.spawn().expect("spawn owned fixture child")),
            label: label.into(),
            own_group: true,
        }
    }

    /// Nested fixture children remain in their already isolated parent's group,
    /// so its watchdog can terminate them even if the parent itself stalls.
    pub(in super::super) fn spawn_nested(command: &mut Command, label: &str) -> Self {
        Self {
            child: Some(command.spawn().expect("spawn nested fixture child")),
            label: label.into(),
            own_group: false,
        }
    }

    pub(in super::super) fn child_mut(&mut self) -> &mut Child {
        self.child.as_mut().expect("fixture child already reaped")
    }

    pub(in super::super) fn wait(&mut self) -> ExitStatus {
        let deadline = Instant::now() + WATCHDOG;
        loop {
            match self.child_mut().try_wait() {
                Ok(Some(status)) => {
                    self.child = None;
                    return status;
                }
                Ok(None) if Instant::now() < deadline => std::thread::yield_now(),
                Ok(None) => panic!("{}: fixture child did not exit", self.label),
                Err(error) => panic!("{}: cannot observe fixture child exit: {error}", self.label),
            }
        }
    }

    /// Allow ARC's wrapper to forward termination and reap its own child.
    pub(in super::super) fn terminate_gracefully(&mut self) {
        if let Some(child) = &mut self.child {
            if matches!(child.try_wait(), Ok(None)) {
                unsafe { libc::kill(child.id() as i32, libc::SIGTERM) };
                let deadline = Instant::now() + WATCHDOG;
                loop {
                    match child.try_wait() {
                        Ok(Some(_)) => {
                            self.child = None;
                            return;
                        }
                        Ok(None) if Instant::now() < deadline => std::thread::yield_now(),
                        _ => break,
                    }
                }
                eprintln!(
                    "{}: graceful fixture termination did not complete",
                    self.label
                );
            }
        }
    }
}

impl Drop for OwnedChild {
    fn drop(&mut self) {
        let Some(mut child) = self.child.take() else {
            return;
        };
        if matches!(child.try_wait(), Ok(None)) {
            // SAFETY: this process group was created for this fixture child.
            if self.own_group {
                unsafe { libc::kill(-(child.id() as i32), libc::SIGKILL) };
            }
            let _ = child.kill();
        }
        let deadline = Instant::now() + WATCHDOG;
        loop {
            match child.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) if Instant::now() < deadline => std::thread::yield_now(),
                result => {
                    eprintln!("{}: failed to reap fixture child: {result:?}", self.label);
                    return;
                }
            }
        }
    }
}

pub(in super::super) mod claude {
    use super::*;

    pub(in super::super::super) fn handle(event: &Value, context: &HookContext) -> Option<Value> {
        super::super::claude::handle(event, context)
    }

    pub(in super::super::super) fn handle_verified(
        event: &Value,
        context: &HookContext,
        surface: Option<&VerifiedSurface<'_>>,
    ) -> Option<Value> {
        super::super::claude::handle_verified(event, context, surface)
    }

    pub(in super::super::super) fn configure(
        action: InstallAction,
        scope: Scope,
        workspace: Option<&Path>,
        context: &HookContext,
    ) -> Result<(), &'static str> {
        super::super::claude::configure(action, scope, workspace, context)
    }

    pub(in super::super::super) fn configure_with_checkpoint(
        action: InstallAction,
        scope: Scope,
        workspace: Option<&Path>,
        context: &HookContext,
        on_contention: impl FnOnce(),
        checkpoint: impl FnOnce(),
        before_replace: impl FnOnce(),
    ) -> Result<(), &'static str> {
        super::super::claude::configure_with_checkpoint(
            action,
            scope,
            workspace,
            context,
            on_contention,
            checkpoint,
            before_replace,
        )
    }
}

pub(in super::super) mod cursor {
    use super::*;

    pub(in super::super::super) fn handle(event: &Value, context: &HookContext) -> Option<Value> {
        super::super::cursor::handle(event, context)
    }

    pub(in super::super::super) fn handle_verified(
        event: &Value,
        context: &HookContext,
        surface: Option<&VerifiedSurface<'_>>,
    ) -> Option<Value> {
        super::super::cursor::handle_verified(event, context, surface)
    }

    pub(in super::super::super) fn configure(
        action: InstallAction,
        scope: Scope,
        workspace: Option<&Path>,
        context: &HookContext,
    ) -> Result<(), &'static str> {
        super::super::cursor::configure(action, scope, workspace, context)
    }

    pub(in super::super::super) fn configure_intercepted(
        action: InstallAction,
        scope: Scope,
        workspace: Option<&Path>,
        context: &HookContext,
        on_contention: impl FnOnce(),
        before_check: impl FnOnce(),
    ) -> Result<(), &'static str> {
        super::super::cursor::configure_intercepted(
            action,
            scope,
            workspace,
            context,
            on_contention,
            before_check,
        )
    }
}

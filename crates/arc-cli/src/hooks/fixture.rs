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
    process::Command,
};

const WORKER_TOKEN: &str = "ARC_HOOK_TEST_WORKER_TOKEN";
const WORKER_FD: i32 = 9;

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

#[path = "fixture_owned.rs"]
mod owned;
pub(in super::super) use owned::OwnedChild;

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

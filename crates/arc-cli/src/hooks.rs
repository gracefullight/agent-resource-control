//! Shared dispatcher and bounded hook contract. Vendor adapters own their files.
#[path = "agent_hooks/claude.rs"]
mod claude;
#[path = "agent_hooks/cursor.rs"]
mod cursor;
#[path = "hooks/event.rs"]
mod event;
#[cfg(test)]
#[allow(dead_code)]
#[path = "hooks/fixture.rs"]
pub(super) mod fixture;

use clap::{Subcommand, ValueEnum};
use serde_json::Value;
use std::{
    ffi::{CString, OsStr, OsString},
    io::{self, Read, Write},
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    process::ExitCode,
};

const MAX_EVENT_BYTES: u64 = 1024 * 1024;

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(crate) enum Vendor {
    Claude,
    Cursor,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub(super) enum Scope {
    User,
    Workspace,
}

#[derive(Debug, Subcommand)]
pub(crate) enum HooksCommand {
    /// Process one vendor event from standard input
    Run { vendor: Vendor },
    /// Install the selected integration
    Install {
        vendor: Vendor,
        #[arg(long, value_enum)]
        scope: Scope,
        #[arg(long)]
        workspace: Option<PathBuf>,
    },
    /// Remove the selected integration
    Uninstall {
        vendor: Vendor,
        #[arg(long, value_enum)]
        scope: Scope,
        #[arg(long)]
        workspace: Option<PathBuf>,
    },
}

#[derive(Clone, Copy, Debug)]
pub(super) enum InstallAction {
    Install,
    Uninstall,
}

pub(super) struct HookContext {
    /// The current binary; a missing or unusable path must fail open.
    pub(super) executable: Option<PathBuf>,
}

/// The adapter must establish, from its supported execution surface, that the
/// command shell is local, enabled, and will resolve the literal program via
/// this PATH without an alias, function, hash, or startup change. This type is
/// an adapter-to-shared-code contract, not evidence by itself.
pub(super) struct VerifiedSurface<'a> {
    pub(super) command_path: &'a OsStr,
}

pub(crate) fn run(command: HooksCommand) -> ExitCode {
    let context = HookContext {
        executable: std::env::current_exe().ok(),
    };
    match command {
        HooksCommand::Run { vendor } => {
            let mut bytes = Vec::new();
            let event = io::stdin()
                .lock()
                .take(MAX_EVENT_BYTES + 1)
                .read_to_end(&mut bytes)
                .ok()
                .filter(|_| bytes.len() <= MAX_EVENT_BYTES as usize)
                .and_then(|_| event::decode(&bytes));
            let response = event.and_then(|event| dispatch(vendor, &event, &context));
            match response {
                Some(value) => {
                    let mut output = io::stdout().lock();
                    if serde_json::to_writer(&mut output, &value).is_err()
                        || output.write_all(b"\n").is_err()
                    {
                        return ExitCode::FAILURE;
                    }
                    ExitCode::SUCCESS
                }
                // Cursor documents nonzero, other than 2, as fail-open. Its
                // permission hook blocks invalid or schema-invalid JSON.
                None if matches!(vendor, Vendor::Cursor) => ExitCode::FAILURE,
                None => ExitCode::SUCCESS,
            }
        }
        HooksCommand::Install {
            vendor,
            scope,
            workspace,
        } => configure(vendor, InstallAction::Install, scope, workspace, &context),
        HooksCommand::Uninstall {
            vendor,
            scope,
            workspace,
        } => configure(vendor, InstallAction::Uninstall, scope, workspace, &context),
    }
}

fn dispatch(vendor: Vendor, event: &Value, context: &HookContext) -> Option<Value> {
    dispatch_with(vendor, event, context, claude::handle, cursor::handle)
}

fn dispatch_with(
    vendor: Vendor,
    event: &Value,
    context: &HookContext,
    claude: impl FnOnce(&Value, &HookContext) -> Option<Value>,
    cursor: impl FnOnce(&Value, &HookContext) -> Option<Value>,
) -> Option<Value> {
    if !event::exact_numbers(event) {
        return None;
    }
    match vendor {
        Vendor::Claude => claude(event, context),
        Vendor::Cursor => cursor(event, context),
    }
}

/// Fixture drivers may call the same adapter transformation with proof from a
/// controlled shell. Production dispatch does not manufacture such proof.
#[cfg(test)]
pub(super) fn dispatch_verified(
    vendor: Vendor,
    event: &Value,
    context: &HookContext,
    surface: &VerifiedSurface<'_>,
) -> Option<Value> {
    if !event::exact_numbers(event) {
        return None;
    }
    match vendor {
        Vendor::Claude => claude::handle_verified(event, context, Some(surface)),
        Vendor::Cursor => cursor::handle_verified(event, context, Some(surface)),
    }
}

/// Fixture entry which applies the same raw JSON guard as production before
/// providing a proof from a shell whose execution surface the test controls.
#[cfg(test)]
pub(super) fn dispatch_verified_event(
    vendor: Vendor,
    bytes: &[u8],
    context: &HookContext,
    surface: &VerifiedSurface<'_>,
) -> Option<Value> {
    let event = event::decode(bytes)?;
    dispatch_verified(vendor, &event, context, surface)
}

fn configure(
    vendor: Vendor,
    action: InstallAction,
    scope: Scope,
    workspace: Option<PathBuf>,
    context: &HookContext,
) -> ExitCode {
    configure_with(
        vendor,
        action,
        scope,
        workspace,
        context,
        claude::configure,
        cursor::configure,
    )
}

fn configure_with(
    vendor: Vendor,
    action: InstallAction,
    scope: Scope,
    workspace: Option<PathBuf>,
    context: &HookContext,
    claude: impl FnOnce(InstallAction, Scope, Option<&Path>, &HookContext) -> Result<(), &'static str>,
    cursor: impl FnOnce(InstallAction, Scope, Option<&Path>, &HookContext) -> Result<(), &'static str>,
) -> ExitCode {
    if matches!(scope, Scope::Workspace)
        && !workspace
            .as_ref()
            .is_some_and(|p| p.is_absolute() && p.is_dir())
    {
        eprintln!("A workspace scope requires an existing absolute --workspace directory.");
        return ExitCode::FAILURE;
    }
    if matches!(scope, Scope::User) && workspace.is_some() {
        eprintln!("--workspace is only valid for workspace scope.");
        return ExitCode::FAILURE;
    }
    let result = match vendor {
        Vendor::Claude => claude(action, scope, workspace.as_deref(), context),
        Vendor::Cursor => cursor(action, scope, workspace.as_deref(), context),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

pub(crate) fn literal_argv(argv: &[OsString]) -> bool {
    match argv {
        [program, action] => program == "cargo" && action == "build",
        [program, action, script] => program == "bun" && action == "run" && script == "build",
        _ => false,
    }
}

/// Never launches either executable. The caller is responsible for obtaining
/// a real surface proof before constructing `VerifiedSurface`.
pub(super) fn rewrite_literal(
    command: &str,
    context: &HookContext,
    surface: Option<&VerifiedSurface<'_>>,
) -> Option<String> {
    let program = match command {
        "cargo build" => "cargo",
        "bun run build" => "bun",
        _ => return None,
    };
    let surface = surface?;
    let executable = context.executable.as_deref()?;
    if !executable.is_absolute() || !is_executable(executable) {
        return None;
    }
    resolved_executable(OsStr::new(program), surface.command_path)?;
    let quoted = format!("'{}'", executable.to_str()?.replace('\'', "'\\''"));
    Some(format!("{quoted} hook-exec -- {command}"))
}

fn is_executable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    let Ok(path) = CString::new(path.as_os_str().as_bytes()) else {
        return false;
    };
    // SAFETY: the NUL-terminated path lives through the call.
    unsafe { libc::access(path.as_ptr(), libc::X_OK) == 0 }
}

/// Use the same non-canonicalized absolute PATH spelling that the supported
/// fixture shells place in their exported `_` variable for a native command.
pub(crate) fn resolved_executable(program: &OsStr, path: &OsStr) -> Option<PathBuf> {
    let dirs: Vec<PathBuf> = std::env::split_paths(path).collect();
    if dirs.is_empty() || dirs.iter().any(|dir| !dir.is_absolute()) {
        return None;
    }
    dirs.into_iter()
        .map(|dir| dir.join(program))
        .find(|candidate| is_executable(candidate))
}

#[cfg(test)]
#[path = "hooks/tests.rs"]
mod tests;

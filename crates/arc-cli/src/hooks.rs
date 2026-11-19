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
    os::unix::{ffi::OsStrExt, fs::MetadataExt},
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

/// The recorded local vendor shell starts hook-exec with its own cwd and PATH.
/// Tool resolution belongs to that later process, not to the hook process.
pub(super) struct VerifiedSurface;

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
            let response = event
                .and_then(|event| dispatch(vendor, &event, &context))
                .and_then(|response| event::wire_response(vendor, &bytes, &response));
            match response {
                Some(value) => {
                    let mut output = io::stdout().lock();
                    if output.write_all(&value).is_err() || output.write_all(b"\n").is_err() {
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
    match vendor {
        Vendor::Claude => claude(event, context),
        Vendor::Cursor => cursor(event, context),
    }
}

/// Fixture drivers can exercise the same adapter transformation directly.
#[cfg(test)]
pub(super) fn dispatch_verified(
    vendor: Vendor,
    event: &Value,
    context: &HookContext,
    surface: &VerifiedSurface,
) -> Option<Value> {
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
    surface: &VerifiedSurface,
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

/// The hook does not resolve or launch the requested executable.
pub(super) fn rewrite_literal(
    command: &str,
    context: &HookContext,
    surface: Option<&VerifiedSurface>,
) -> Option<String> {
    match command {
        "cargo build" | "bun run build" => (),
        _ => return None,
    }
    surface?;
    let executable = context.executable.as_deref()?;
    if !executable.is_absolute() || !is_executable(executable) {
        return None;
    }
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

/// Use the same non-canonicalized absolute PATH spelling that zsh places in
/// its exported `_` variable, including relative and empty PATH components.
pub(crate) fn resolved_executable(program: &OsStr, path: &OsStr) -> Option<PathBuf> {
    resolved_path_entry(program, path).map(|(path, _)| path)
}

/// Return zsh's absolute `_` spelling and the path used to invoke the tool.
pub(crate) fn resolved_path_entry(program: &OsStr, path: &OsStr) -> Option<(PathBuf, PathBuf)> {
    let physical_cwd = std::env::current_dir().ok()?;
    let cwd = std::env::var_os("PWD")
        .map(PathBuf::from)
        .filter(|pwd| {
            pwd.is_absolute()
                && std::fs::metadata(pwd)
                    .ok()
                    .zip(std::fs::metadata(&physical_cwd).ok())
                    .is_some_and(|(a, b)| a.dev() == b.dev() && a.ino() == b.ino())
        })
        .unwrap_or(physical_cwd);
    std::env::split_paths(path)
        .map(|dir| {
            let current = dir.as_os_str().is_empty() || dir.as_os_str() == OsStr::new(".");
            let invocation = if current {
                program.to_os_string()
            } else {
                let mut name = dir.as_os_str().to_os_string();
                name.push("/");
                name.push(program);
                name
            };
            let absolute = if dir.is_absolute() {
                PathBuf::from(&invocation)
            } else {
                let mut name = cwd.as_os_str().to_os_string();
                name.push("/");
                name.push(&invocation);
                PathBuf::from(name)
            };
            (absolute, PathBuf::from(invocation))
        })
        .find(|(candidate, _)| is_executable(candidate))
}

#[cfg(test)]
#[path = "hooks/tests.rs"]
mod tests;

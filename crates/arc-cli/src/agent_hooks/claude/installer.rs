//! Scoped settings updates with a per-settings installation identity.
#[path = "lossless_json.rs"]
mod lossless_json;
#[path = "metadata.rs"]
mod metadata;
#[path = "transaction.rs"]
mod transaction;

use super::super::{HookContext, InstallAction, Scope};
use lossless_json::Settings;
use serde_json::{json, Value};
use std::{
    ffi::{CString, OsStr},
    fs::{self, OpenOptions},
    io::{self, Write},
    os::unix::{ffi::OsStrExt, fs::OpenOptionsExt},
    path::{Path, PathBuf},
};

const COMMAND_SUFFIX: &str = " hooks run claude # ARC Claude Code hook";
const MARKER: &str = "# ARC Claude Code hook id=";
const INVALID_IDENTITY: &str = "Invalid Claude hook identity; restore settings.json.arc-claude-id, or review and manually remove the identity and marked hook before retrying.";

struct Identity {
    id: String,
    commands: Vec<String>,
}

pub(super) fn configure(
    action: InstallAction,
    scope: Scope,
    workspace: Option<&Path>,
    context: &HookContext,
) -> Result<(), &'static str> {
    configure_inner(action, scope, workspace, context, || {}, || {}, || {})
}

#[cfg(test)]
#[allow(dead_code)] // The separately compiled Claude fixture uses this checkpoint.
pub(super) fn configure_with_checkpoint(
    action: InstallAction,
    scope: Scope,
    workspace: Option<&Path>,
    context: &HookContext,
    on_contention: impl FnOnce(),
    checkpoint: impl FnOnce(),
    before_replace: impl FnOnce(),
) -> Result<(), &'static str> {
    configure_inner(
        action,
        scope,
        workspace,
        context,
        on_contention,
        checkpoint,
        before_replace,
    )
}

fn configure_inner(
    action: InstallAction,
    scope: Scope,
    workspace: Option<&Path>,
    context: &HookContext,
    on_contention: impl FnOnce(),
    checkpoint: impl FnOnce(),
    before_replace: impl FnOnce(),
) -> Result<(), &'static str> {
    let path = settings_path(scope, workspace)?;
    // Follow an existing settings symlink instead of replacing it when the
    // temporary file is renamed over the settings target.
    let path = match fs::symlink_metadata(&path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            fs::canonicalize(&path).map_err(|_| "Cannot resolve Claude settings symlink.")?
        }
        Ok(_) => path,
        Err(error) if error.kind() == io::ErrorKind::NotFound => path,
        Err(_) => return Err("Cannot inspect Claude settings."),
    };
    let _lock = transaction::Lock::acquire(&path, on_contention)?;
    let identity_path = identity_path(&path);
    let identity_snapshot = transaction::Snapshot::read(&identity_path)?;
    let existing_identity = read_identity(identity_snapshot.bytes.as_deref())?;
    let settings_snapshot = transaction::Snapshot::read(&path)?;
    let mut settings = Settings::parse(settings_snapshot.bytes.as_deref().unwrap_or(b"{}"))?;
    checkpoint();
    let known_commands = existing_identity
        .as_ref()
        .map_or(&[][..], |identity| identity.commands.as_slice());
    reject_unknown_candidates(&settings.value, known_commands)?;
    settings_snapshot.ensure_current(&path)?;
    identity_snapshot.ensure_current(&identity_path)?;
    if existing_identity.is_none() && matches!(action, InstallAction::Uninstall) {
        return Ok(());
    }
    let id = match existing_identity.as_ref() {
        Some(identity) => identity.id.clone(),
        None => arc_core::process_identity::random_job_id()
            .map_err(|_| "Cannot allocate Claude hook identity.")?,
    };
    let entry = match action {
        InstallAction::Install => Some(hook_entry(context, &id)?),
        InstallAction::Uninstall => None,
    };
    let root = settings
        .value
        .as_object_mut()
        .ok_or("Claude settings must be a JSON object.")?;
    if matches!(action, InstallAction::Uninstall) && !root.contains_key("hooks") {
        remove_identity(&identity_path)?;
        return Ok(());
    }
    let hooks = root
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or("Claude hooks must be a JSON object.")?;
    if matches!(action, InstallAction::Uninstall) && !hooks.contains_key("PreToolUse") {
        remove_identity(&identity_path)?;
        return Ok(());
    }
    let pre_tool_use = hooks
        .entry("PreToolUse")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .ok_or("Claude PreToolUse hooks must be a JSON array.")?;
    let new_command = entry
        .as_ref()
        .and_then(|entry| entry["hooks"][0]["command"].as_str())
        .map(str::to_owned);
    let changed = update_entries(pre_tool_use, entry, known_commands);
    let created_identity = existing_identity.is_none() && matches!(action, InstallAction::Install);
    if created_identity {
        let identity = Identity {
            id,
            commands: vec![new_command.clone().unwrap()],
        };
        create_identity(&identity_path, &identity)?;
    } else if changed && matches!(action, InstallAction::Install) {
        let mut identity = existing_identity.unwrap();
        let command = new_command.unwrap();
        if !identity.commands.contains(&command) {
            identity.commands.push(command);
            identity_snapshot.write(&identity_path, &encode_identity(&identity)?)?;
        }
    }
    if changed {
        // Keep the identity if the settings update fails. It records both
        // possible commands and permits a safe retry after a stale snapshot.
        settings_snapshot.write_with(&path, &settings.encode()?, before_replace)?;
    }
    if matches!(action, InstallAction::Uninstall) {
        identity_snapshot.ensure_current(&identity_path)?;
        remove_identity(&identity_path)?;
    }
    Ok(())
}

fn reject_unknown_candidates(
    settings: &Value,
    known_commands: &[String],
) -> Result<(), &'static str> {
    // Inspect every value, not just the matcher ARC writes. A marked entry
    // elsewhere may be a moved hook or a user-authored copy; neither is ours
    // to delete or update by command text alone.
    let markers = marker_count(settings);
    let owned = settings
        .get("hooks")
        .and_then(|value| value.get("PreToolUse"))
        .and_then(Value::as_array)
        .map_or(0, |groups| {
            groups
                .iter()
                .filter(|group| owned_group(group, known_commands))
                .count()
        });
    if markers == 0 || (markers == 1 && owned == 1) {
        Ok(())
    } else {
        Err("Ambiguous ARC Claude hook: restore its matching settings.json.arc-claude-id, or review and manually remove the candidate before retrying.")
    }
}

fn marker_count(value: &Value) -> usize {
    match value {
        Value::Array(values) => values.iter().map(marker_count).sum(),
        Value::Object(values) => values
            .iter()
            .map(|(key, value)| markers_in_text(key) + marker_count(value))
            .sum(),
        Value::String(text) => markers_in_text(text),
        _ => 0,
    }
}

fn markers_in_text(text: &str) -> usize {
    // A string is one candidate even if its executable path also contains
    // marker-like text. Inspect every occurrence within the string. Text
    // appended to the identity is still part of the shell comment: it must
    // not hide an otherwise active hook, even without a sidecar or when its
    // executable moved.
    usize::from(text.match_indices(MARKER).any(|(index, _)| {
        let tail = &text[index + MARKER.len()..];
        tail.get(..32).is_some_and(valid_id)
    }))
}

fn valid_id(id: &str) -> bool {
    id.len() == 32
        && id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn owned_group(value: &Value, known_commands: &[String]) -> bool {
    let Some(group) = value.as_object() else {
        return false;
    };
    if group.len() != 2 || group.get("matcher").and_then(Value::as_str) != Some("Bash") {
        return false;
    }
    let Some(handlers) = group.get("hooks").and_then(Value::as_array) else {
        return false;
    };
    let [handler] = handlers.as_slice() else {
        return false;
    };
    let Some(handler) = handler.as_object() else {
        return false;
    };
    handler.len() == 2
        && handler.get("type").and_then(Value::as_str) == Some("command")
        && handler
            .get("command")
            .and_then(Value::as_str)
            .is_some_and(|command| known_commands.iter().any(|known| known == command))
}

fn update_entries(
    groups: &mut Vec<Value>,
    entry: Option<Value>,
    known_commands: &[String],
) -> bool {
    match (
        groups
            .iter()
            .position(|group| owned_group(group, known_commands)),
        entry,
    ) {
        (Some(index), Some(entry)) if groups[index] != entry => {
            groups[index] = entry;
            true
        }
        (Some(_), Some(_)) | (None, None) => false,
        (Some(index), None) => {
            groups.remove(index);
            true
        }
        (None, Some(entry)) => {
            groups.push(entry);
            true
        }
    }
}

fn managed_command(command: &str, id: &str) -> bool {
    let suffix = format!("{COMMAND_SUFFIX} id={id}");
    let Some(quoted_path) = command.strip_suffix(&suffix) else {
        return false;
    };
    let Some(encoded_path) = quoted_path
        .strip_prefix('\'')
        .and_then(|path| path.strip_suffix('\''))
    else {
        return false;
    };
    // Match exactly the quoting emitted by hook_entry, including embedded
    // apostrophes. A suffix alone must not claim a user's compound command.
    let path = encoded_path.replace("'\\''", "'");
    Path::new(&path).is_absolute()
        && !path.contains('\0')
        && path.replace('\'', "'\\''") == encoded_path
}

fn settings_path(scope: Scope, workspace: Option<&Path>) -> Result<PathBuf, &'static str> {
    let directory = match scope {
        Scope::User => {
            if workspace.is_some() {
                return Err("A workspace is only valid for workspace scope.");
            }
            match std::env::var_os("CLAUDE_CONFIG_DIR").filter(|value| !value.is_empty()) {
                Some(directory) => PathBuf::from(directory),
                None => PathBuf::from(
                    std::env::var_os("HOME")
                        .filter(|value| !value.is_empty())
                        .ok_or("HOME is required for Claude user settings.")?,
                )
                .join(".claude"),
            }
        }
        Scope::Workspace => {
            let workspace = workspace
                .filter(|path| path.is_absolute() && path.is_dir())
                .ok_or("An existing absolute workspace is required.")?;
            workspace.join(".claude")
        }
    };
    Ok(directory.join("settings.json"))
}

fn hook_entry(context: &HookContext, id: &str) -> Result<Value, &'static str> {
    let executable = context
        .executable
        .as_deref()
        .filter(|path| path.is_absolute() && executable_file(path))
        .and_then(Path::to_str)
        .ok_or("An available absolute arcctl executable is required.")?;
    let quoted = executable.replace('\'', "'\\''");
    Ok(json!({
        "matcher": "Bash",
        "hooks": [{
            "type": "command",
            "command": format!("'{quoted}'{COMMAND_SUFFIX} id={id}"),
        }],
    }))
}

fn executable_file(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    let Ok(path) = CString::new(OsStr::new(path).as_bytes()) else {
        return false;
    };
    // SAFETY: path is a live, NUL-terminated C string.
    unsafe { libc::access(path.as_ptr(), libc::X_OK) == 0 }
}

fn identity_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".arc-claude-id");
    PathBuf::from(name)
}

fn read_identity(bytes: Option<&[u8]>) -> Result<Option<Identity>, &'static str> {
    let Some(bytes) = bytes else { return Ok(None) };
    let record: Value = serde_json::from_slice(bytes).map_err(|_| INVALID_IDENTITY)?;
    let id = record
        .get("id")
        .and_then(Value::as_str)
        .ok_or(INVALID_IDENTITY)?;
    if id.len() != 32
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(INVALID_IDENTITY);
    }
    let commands = record
        .get("commands")
        .and_then(Value::as_array)
        .ok_or(INVALID_IDENTITY)?;
    if commands.is_empty()
        || commands.iter().any(|value| {
            !value
                .as_str()
                .is_some_and(|command| managed_command(command, id))
        })
    {
        return Err(INVALID_IDENTITY);
    }
    Ok(Some(Identity {
        id: id.to_owned(),
        commands: commands
            .iter()
            .map(|value| value.as_str().unwrap().to_owned())
            .collect(),
    }))
}

fn encode_identity(identity: &Identity) -> Result<Vec<u8>, &'static str> {
    serde_json::to_vec(&json!({"id": identity.id, "commands": identity.commands}))
        .map_err(|_| "Cannot encode Claude hook identity.")
}

fn create_identity(path: &Path, identity: &Identity) -> Result<(), &'static str> {
    let parent = path.parent().ok_or("Invalid Claude hook identity path.")?;
    fs::create_dir_all(parent).map_err(|_| "Cannot create Claude settings directory.")?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|_| "Cannot create Claude hook identity.")?;
    if file
        .write_all(&encode_identity(identity)?)
        .and_then(|()| file.flush())
        .is_err()
    {
        let _ = fs::remove_file(path);
        return Err("Cannot write Claude hook identity.");
    }
    Ok(())
}

fn remove_identity(path: &Path) -> Result<(), &'static str> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err("Cannot remove Claude hook identity."),
    }
}

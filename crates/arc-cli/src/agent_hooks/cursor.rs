//! Cursor Shell fixture transformation and isolated configuration installer.
//!
//! The production entry intentionally fails open: neither the shell's local
//! execution surface nor an approval-preserving Cursor response is verified.
#[path = "cursor/lock.rs"]
mod lock;
#[path = "cursor/lossless_json.rs"]
mod lossless_json;
#[path = "cursor/metadata.rs"]
mod metadata;
#[path = "cursor/snapshot.rs"]
mod snapshot;

use super::{rewrite_literal, HookContext, InstallAction, Scope, VerifiedSurface};
use serde_json::{json, Map, Value};
use std::{
    env,
    ffi::CString,
    fs::{self, OpenOptions},
    io::Write,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
};

pub(super) fn handle(event: &Value, context: &HookContext) -> Option<Value> {
    // A vendor event, PATH, or absent remote flag cannot prove the actual
    // command shell or that a rewritten response preserves approval.
    handle_verified(event, context, None)
}

/// This provisional transformation is for fixtures, not a production response.
/// Its acceptance and effect on Cursor's approval flow have not been verified.
pub(super) fn handle_verified(
    event: &Value,
    context: &HookContext,
    surface: Option<&VerifiedSurface<'_>>,
) -> Option<Value> {
    if env::var_os("ARC_HOOKS_DISABLED").as_deref() == Some("1".as_ref())
        || env::var_os("ARC_HOOKS_CURSOR_DISABLED").as_deref() == Some("1".as_ref())
        || env::var_os("CURSOR_CODE_REMOTE").as_deref() == Some("true".as_ref())
        || event.get("hook_event_name")?.as_str()? != "preToolUse"
        || event.get("tool_name")?.as_str()? != "Shell"
    {
        return None;
    }
    let input = event.get("tool_input")?.as_object()?;
    let command = input.get("command")?.as_str()?;
    let command = rewrite_literal(command, context, surface)?;
    let mut updated = input.clone();
    updated.insert("command".into(), Value::String(command));
    // This permission-free candidate is only a fixture transformation. The
    // production entry above never sends it to Cursor.
    Some(json!({ "updated_input": updated }))
}

pub(super) fn configure(
    action: InstallAction,
    scope: Scope,
    workspace: Option<&Path>,
    context: &HookContext,
) -> Result<(), &'static str> {
    configure_with(action, scope, workspace, context, || {}, || {})
}

#[cfg(test)]
#[allow(dead_code)] // Integration fixtures call this seam through the owned module.
pub(super) fn configure_intercepted(
    action: InstallAction,
    scope: Scope,
    workspace: Option<&Path>,
    context: &HookContext,
    on_contention: impl FnOnce(),
    before_check: impl FnOnce(),
) -> Result<(), &'static str> {
    configure_with(
        action,
        scope,
        workspace,
        context,
        on_contention,
        before_check,
    )
}

fn configure_with(
    action: InstallAction,
    scope: Scope,
    workspace: Option<&Path>,
    context: &HookContext,
    on_contention: impl FnOnce(),
    before_check: impl FnOnce(),
) -> Result<(), &'static str> {
    let root = match scope {
        Scope::User if workspace.is_none() => {
            let home = env::var_os("HOME").ok_or("Cursor user scope requires HOME.")?;
            let home = PathBuf::from(home);
            if !home.is_absolute() || !home.is_dir() {
                return Err("Cursor user scope requires an existing absolute HOME.");
            }
            home
        }
        Scope::Workspace => {
            let root = workspace.ok_or("Cursor workspace scope requires a workspace.")?;
            if !root.is_absolute() || !root.is_dir() {
                return Err("Cursor workspace scope requires an existing absolute workspace.");
            }
            root.to_path_buf()
        }
        _ => return Err("--workspace is only valid for workspace scope."),
    };
    let entry = if matches!(action, InstallAction::Install) {
        let executable = context
            .executable
            .as_deref()
            .filter(|path| executable_available(path))
            .and_then(Path::to_str)
            .ok_or("Cursor hook requires an available absolute arcctl path.")?;
        Some(json!({
            "command": format!("'{}' hooks run cursor # ARC_CURSOR_HOOK_V1", executable.replace('\'', "'\\''")),
            "matcher": "^Shell$"
        }))
    } else {
        None
    };
    let dir = root.join(".cursor");
    let path = dir.join("hooks.json");
    update_config(
        action,
        &dir,
        &path,
        entry.as_ref(),
        on_contention,
        before_check,
    )
}

fn owned_entry(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    if object.len() != 2 || object.get("matcher") != Some(&json!("^Shell$")) {
        return false;
    }
    let Some(quoted_path) = object
        .get("command")
        .and_then(Value::as_str)
        .and_then(|command| command.strip_prefix('\''))
        .and_then(|command| command.strip_suffix("' hooks run cursor # ARC_CURSOR_HOOK_V1"))
    else {
        return false;
    };
    let path = quoted_path.replace("'\\''", "'");
    Path::new(&path).is_absolute()
        && !path.contains('\0')
        && path.replace('\'', "'\\''") == quoted_path
}

const AMBIGUOUS: &str = "Cursor ARC marker has ambiguous ownership; review hooks.json and remove or resolve the marked entry manually.";
const MARKER: &str = "ARC_CURSOR_HOOK_V1";

fn exact_marker(text: &str) -> bool {
    text.match_indices(MARKER).any(|(index, _)| {
        let boundary = |byte: &u8| !byte.is_ascii_alphanumeric() && *byte != b'_';
        text.as_bytes()[..index].last().is_none_or(boundary)
            && text.as_bytes()[index + MARKER.len()..]
                .first()
                .is_none_or(boundary)
    })
}

fn contains_marker(value: &Value) -> bool {
    match value {
        Value::String(text) => exact_marker(text),
        Value::Array(items) => items.iter().any(contains_marker),
        Value::Object(fields) => fields
            .iter()
            .any(|(key, value)| exact_marker(key) || contains_marker(value)),
        _ => false,
    }
}

// A matching command shape alone is not proof. Only an entry matching the
// installer's recorded entry in hooks.preToolUse is safe to change.
fn check_marker_ownership(value: &Value, owner: Option<&Value>) -> Result<bool, &'static str> {
    let root = value
        .as_object()
        .ok_or("Cursor hooks.json must contain an object.")?;
    let mut found = false;
    for (key, value) in root {
        if exact_marker(key) {
            return Err(AMBIGUOUS);
        }
        if key == "hooks" {
            if let Some(hooks) = value.as_object() {
                for (event, entries) in hooks {
                    if exact_marker(event) {
                        return Err(AMBIGUOUS);
                    }
                    if event == "preToolUse" {
                        if let Some(entries) = entries.as_array() {
                            for entry in entries {
                                if contains_marker(entry) {
                                    if found || Some(entry) != owner {
                                        return Err(AMBIGUOUS);
                                    }
                                    found = true;
                                }
                            }
                            continue;
                        }
                    }
                    if contains_marker(entries) {
                        return Err(AMBIGUOUS);
                    }
                }
                continue;
            }
        }
        if contains_marker(value) {
            return Err(AMBIGUOUS);
        }
    }
    Ok(found)
}

fn executable_available(path: &Path) -> bool {
    if !path.is_absolute() || !path.is_file() {
        return false;
    }
    let Ok(path) = CString::new(path.as_os_str().as_bytes()) else {
        return false;
    };
    // SAFETY: the NUL-terminated path remains live through access.
    unsafe { libc::access(path.as_ptr(), libc::X_OK) == 0 }
}

fn update_config(
    action: InstallAction,
    dir: &Path,
    path: &Path,
    entry: Option<&Value>,
    on_contention: impl FnOnce(),
    before_check: impl FnOnce(),
) -> Result<(), &'static str> {
    if matches!(action, InstallAction::Uninstall)
        && matches!(fs::symlink_metadata(path), Err(error) if error.kind() == std::io::ErrorKind::NotFound)
        && matches!(fs::symlink_metadata(dir.join(".hooks.json.arc.lock")), Err(error) if error.kind() == std::io::ErrorKind::NotFound)
    {
        return Ok(());
    }
    // The stable sidecar serializes ARC installers even though hooks.json is
    // replaced by rename. Other programs do not participate in this lock.
    let mut lock = lock::acquire(dir, on_contention)?;
    let snapshot = snapshot::read(path)?;
    if snapshot.is_none() && matches!(action, InstallAction::Uninstall) {
        if lock::read_owner(&mut lock)?.is_some() {
            lock::write_owner(&mut lock, None)?;
        }
        return Ok(());
    }
    let mut settings = lossless_json::Settings::parse(
        snapshot
            .as_ref()
            .map_or(&b"{\"version\":1,\"hooks\":{}}"[..], |old| &old.bytes),
    )?;
    let version_one = settings.is_version_one();
    let value = &mut settings.value;
    let mut owner = lock::read_owner(&mut lock)?;
    let found = check_marker_ownership(value, owner.as_ref())?;
    if !found && owner.is_some() {
        // The entry was removed outside the installer; do not retain stale
        // evidence that could later be mistaken for a new user-authored hook.
        lock::write_owner(&mut lock, None)?;
        owner = None;
    }
    let before = value.clone();
    let root = value
        .as_object_mut()
        .ok_or("Cursor hooks.json must contain an object.")?;
    match root.get("version") {
        Some(_) if version_one => {}
        None if matches!(action, InstallAction::Install) => {
            root.insert("version".into(), json!(1));
        }
        None => {}
        _ => return Err("Unsupported Cursor hooks.json version."),
    }
    if matches!(action, InstallAction::Uninstall)
        && (!root.contains_key("hooks")
            || root
                .get("hooks")
                .and_then(Value::as_object)
                .is_some_and(|hooks| !hooks.contains_key("preToolUse")))
    {
        return Ok(());
    }
    let hooks = root
        .entry("hooks")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or("Cursor hooks must be an object.")?;
    let entries = hooks
        .entry("preToolUse")
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .ok_or("Cursor preToolUse hooks must be an array.")?;
    match action {
        InstallAction::Install => {
            let entry = entry.ok_or("Cursor hook entry is missing.")?;
            if let Some(candidate) = entries
                .iter_mut()
                .find(|candidate| Some(&**candidate) == owner.as_ref())
            {
                *candidate = entry.clone();
            } else {
                entries.push(entry.clone());
            }
        }
        InstallAction::Uninstall => {
            entries.retain(|candidate| Some(candidate) != owner.as_ref());
        }
    }
    if *value == before {
        return Ok(());
    }
    let bytes = settings.encode()?;
    let nonce = arc_core::process_identity::random_job_id()
        .map_err(|_| "Cannot allocate Cursor configuration temporary file.")?;
    let temporary = dir.join(format!(".hooks.json.arc-{nonce}.tmp"));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|_| "Cannot create Cursor configuration temporary file.")?;
        file.write_all(&bytes)
            .map_err(|_| "Cannot write Cursor configuration.")?;
        if let Some(old) = snapshot.as_ref() {
            metadata::preserve(&old.source, &file)
                .map_err(|_| "Cannot preserve Cursor configuration access metadata.")?;
        }
        before_check();
        snapshot::check(path, snapshot.as_ref())?;
        // Rename publishes the same-directory temporary atomically. A writer
        // outside ARC's lock is detected by the immediately preceding check.
        fs::rename(&temporary, path).map_err(|_| "Cannot replace Cursor configuration.")
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result?;
    let recorded = match action {
        InstallAction::Install => entry,
        InstallAction::Uninstall => None,
    };
    lock::write_owner(&mut lock, recorded)
}

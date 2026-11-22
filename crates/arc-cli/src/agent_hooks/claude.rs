//! Claude Code PreToolUse/Bash transformation for the recorded local surface.
#[path = "claude/installer.rs"]
mod installer;

use super::{rewrite_literal, HookContext, InstallAction, Scope, VerifiedSurface};
use serde_json::{json, Value};
use std::{ffi::OsStr, path::Path};

pub(super) fn handle(event: &Value, context: &HookContext) -> Option<Value> {
    handle_verified(event, context, Some(&VerifiedSurface))
}

pub(super) fn handle_verified(
    event: &Value,
    context: &HookContext,
    surface: Option<&VerifiedSurface>,
) -> Option<Value> {
    if std::env::var_os("ARC_HOOKS_DISABLED").as_deref() == Some(OsStr::new("1"))
        || std::env::var_os("ARC_HOOKS_CLAUDE_DISABLED").as_deref() == Some(OsStr::new("1"))
        || std::env::var_os("CLAUDE_CODE_REMOTE").as_deref() == Some(OsStr::new("true"))
        || std::env::var_os("SSH_CONNECTION").is_some()
        || event.get("hook_event_name")?.as_str()? != "PreToolUse"
        || event.get("tool_name")?.as_str()? != "Bash"
        || !Path::new(event.get("cwd")?.as_str()?).is_absolute()
    {
        return None;
    }
    let input_value = event.get("tool_input")?;
    let input = input_value.as_object()?;
    let command = input.get("command")?.as_str()?;
    let rewritten = rewrite_literal(command, context, surface)?;
    let rewritten = super::event_key::keyed(super::event_key::CLAUDE, event, command, rewritten);
    let mut updated_input = input.clone();
    updated_input.insert("command".into(), Value::String(rewritten));
    Some(json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "updatedInput": updated_input,
        }
    }))
}

pub(super) fn configure(
    action: InstallAction,
    scope: Scope,
    workspace: Option<&Path>,
    context: &HookContext,
) -> Result<(), &'static str> {
    installer::configure(action, scope, workspace, context)
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
    installer::configure_with_checkpoint(
        action,
        scope,
        workspace,
        context,
        on_contention,
        checkpoint,
        before_replace,
    )
}

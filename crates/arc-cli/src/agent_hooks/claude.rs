//! Claude Code PreToolUse/Bash transformation. Production remains fail-open
//! until the calling surface can prove the shell and execution locality.
#[path = "claude/installer.rs"]
mod installer;

use super::{rewrite_literal, HookContext, InstallAction, Scope, VerifiedSurface};
use serde_json::{json, Value};
use std::{ffi::OsStr, path::Path};

pub(super) fn handle(event: &Value, context: &HookContext) -> Option<Value> {
    // A hook process does not establish the resolution or locality of the
    // separate shell that will run the tool call.
    handle_verified(event, context, None)
}

pub(super) fn handle_verified(
    event: &Value,
    context: &HookContext,
    surface: Option<&VerifiedSurface<'_>>,
) -> Option<Value> {
    if std::env::var_os("ARC_HOOKS_DISABLED").as_deref() == Some(OsStr::new("1"))
        || std::env::var_os("ARC_HOOKS_CLAUDE_DISABLED").as_deref() == Some(OsStr::new("1"))
        || std::env::var_os("CLAUDE_CODE_REMOTE").as_deref() == Some(OsStr::new("true"))
        || event.get("hook_event_name")?.as_str()? != "PreToolUse"
        || event.get("tool_name")?.as_str()? != "Bash"
        || !Path::new(event.get("cwd")?.as_str()?).is_absolute()
    {
        return None;
    }
    let input_value = event.get("tool_input")?;
    // The shared JSON parser can round out-of-range integers and decimals
    // before they reach this adapter. Without the original bytes, declining
    // the update is the only way to preserve such fields for Claude.
    if !exact_input_numbers(input_value) {
        return None;
    }
    let input = input_value.as_object()?;
    let command = input.get("command")?.as_str()?;
    let rewritten = rewrite_literal(command, context, surface)?;
    let mut updated_input = input.clone();
    updated_input.insert("command".into(), Value::String(rewritten));
    Some(json!({
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "updatedInput": updated_input,
        }
    }))
}

fn exact_input_numbers(value: &Value) -> bool {
    match value {
        Value::Number(number) => !number.is_f64(),
        Value::Array(values) => values.iter().all(exact_input_numbers),
        Value::Object(values) => values.values().all(exact_input_numbers),
        _ => true,
    }
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

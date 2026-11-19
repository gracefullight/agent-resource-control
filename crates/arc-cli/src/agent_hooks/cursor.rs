//! Cursor Shell hook transformation and isolated configuration installer.
#[path = "cursor/installer.rs"]
mod installer;

use super::{rewrite_literal, HookContext, InstallAction, Scope, VerifiedSurface};
use serde_json::{json, Value};
use std::{env, path::Path};

pub(super) fn handle(event: &Value, context: &HookContext) -> Option<Value> {
    handle_verified(event, context, Some(&VerifiedSurface))
}

/// Transform the complete tool input only when the caller supplies surface proof.
pub(super) fn handle_verified(
    event: &Value,
    context: &HookContext,
    surface: Option<&VerifiedSurface>,
) -> Option<Value> {
    if env::var_os("ARC_HOOKS_DISABLED").as_deref() == Some("1".as_ref())
        || env::var_os("ARC_HOOKS_CURSOR_DISABLED").as_deref() == Some("1".as_ref())
        || env::var_os("CURSOR_CODE_REMOTE").as_deref() == Some("true".as_ref())
        || env::var_os("SSH_CONNECTION").is_some()
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
    // Cursor accepts this response without making a permission decision; the
    // operator's recorded session observed the rewritten command at the
    // subsequent beforeShellExecution hook.
    Some(json!({ "updated_input": updated }))
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
#[allow(dead_code)] // Integration fixtures call this seam through the owned module.
pub(super) fn configure_intercepted(
    action: InstallAction,
    scope: Scope,
    workspace: Option<&Path>,
    context: &HookContext,
    on_contention: impl FnOnce(),
    before_check: impl FnOnce(),
) -> Result<(), &'static str> {
    installer::configure_intercepted(
        action,
        scope,
        workspace,
        context,
        on_contention,
        before_check,
    )
}

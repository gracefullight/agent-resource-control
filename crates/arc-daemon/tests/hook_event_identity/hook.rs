//! The installed vendor hook as the vendor drives it. The recorded payloads
//! stand for the vendors; no live vendor session is run or claimed.
use crate::{
    journey::{self, output_with_input, Sandbox},
    runtime::{child_watchdog, Fixture},
};
use serde_json::{json, Value};
use std::{fs, path::Path};

/// The adapter whose installed hook receives the event.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Vendor {
    Claude,
    Cursor,
}

impl Vendor {
    fn name(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Cursor => "cursor",
        }
    }

    /// The recorded PreToolUse event for `cargo build`, byte for byte: Claude
    /// Code 2.1.327, and Cursor's `preToolUse`.
    pub fn recorded(self) -> Value {
        serde_json::from_str(match self {
            Self::Claude => include_str!(
                "../../../arc-cli/tests/fixtures/agent_hooks/claude-recorded-2.1.327-cargo-build.json"
            ),
            Self::Cursor => include_str!(
                "../../../arc-cli/tests/fixtures/agent_hooks/cursor/recorded/preToolUse-cargo-build-rewrite.json"
            ),
        })
        .unwrap()
    }

    /// Install the hook into the sandbox HOME and return its command.
    pub fn install(self, f: &Fixture, s: &Sandbox, project: &Path) -> String {
        let install = child_watchdog::output(
            s.arcctl(f, project)
                .args(["hooks", "install", self.name(), "--scope", "user"]),
            "install hook",
        )
        .unwrap();
        assert!(install.status.success(), "{install:?}");
        let (file, list, matcher) = match self {
            Self::Claude => (".claude/settings.json", "PreToolUse", "Bash"),
            Self::Cursor => (".cursor/hooks.json", "preToolUse", "^Shell$"),
        };
        let settings: Value =
            serde_json::from_slice(&fs::read(s.home.join(file)).unwrap()).unwrap();
        let entry = &settings["hooks"][list][0];
        assert_eq!(entry["matcher"], matcher, "{settings}");
        let command = match self {
            Self::Claude => &entry["hooks"][0]["command"],
            Self::Cursor => &entry["command"],
        };
        command.as_str().unwrap().to_owned()
    }

    /// Pass one event through the installed hook and return the command the
    /// vendor would execute, checking that only the command was replaced and
    /// that the hook left the vendor's permission evaluation alone.
    pub fn returned_command(
        self,
        f: &Fixture,
        s: &Sandbox,
        project: &Path,
        installed: &str,
        event: &Value,
    ) -> String {
        let response = output_with_input(
            &mut s.shell(f, project, installed),
            &serde_json::to_vec(event).unwrap(),
            "installed hook",
        );
        assert!(response.status.success(), "{response:?}");
        assert!(response.stderr.is_empty(), "{response:?}");
        let response: Value = serde_json::from_slice(&response.stdout).unwrap();
        assert_eq!(response.as_object().unwrap().len(), 1, "{response}");
        let updated = match self {
            Self::Claude => {
                let hook = &response["hookSpecificOutput"];
                assert_eq!(hook.as_object().unwrap().len(), 2, "{response}");
                assert_eq!(hook["hookEventName"], "PreToolUse", "{response}");
                assert!(hook.get("permissionDecision").is_none(), "{response}");
                &hook["updatedInput"]
            }
            Self::Cursor => {
                assert!(response.get("permission").is_none(), "{response}");
                &response["updated_input"]
            }
        };
        let mut restored = updated.clone();
        let command = restored["command"].as_str().unwrap().to_owned();
        restored["command"] = event["tool_input"]["command"].clone();
        assert_eq!(restored, event["tool_input"], "{response}");
        command
    }
}

/// The recorded Claude Code 2.1.327 PreToolUse event, re-addressed to the
/// fixture project and carrying `command`.
pub fn claude_event(project: &Path, command: &str) -> Value {
    let mut event = Vendor::Claude.recorded();
    event["cwd"] = json!(project);
    event["tool_input"]["command"] = json!(command);
    event
}

/// The shell-quoted built `arcctl` exactly as the installed hook writes it,
/// taken from a returned command that ends with `rest`.
pub fn quoted_arcctl<'a>(command: &'a str, rest: &str) -> &'a str {
    let quoted = command
        .strip_suffix(rest)
        .unwrap_or_else(|| panic!("{command:?} does not end with {rest:?}"));
    let path = quoted
        .strip_prefix('\'')
        .and_then(|path| path.strip_suffix('\''))
        .unwrap_or_else(|| panic!("not one quoted executable: {quoted:?}"));
    assert_eq!(
        fs::canonicalize(path).unwrap(),
        fs::canonicalize(journey::binary("arcctl")).unwrap()
    );
    quoted
}

/// The event key of a returned command, which must be exactly
/// `'<arcctl>' hook-exec --arc-event <64 lowercase hex> -- <literal>`: the
/// option directly after `hook-exec` and the literal unchanged.
#[track_caller]
pub fn event_key(command: &str, literal: &str) -> String {
    let (quoted, key) = command
        .strip_suffix(&format!(" -- {literal}"))
        .and_then(|rest| rest.split_once(" hook-exec --arc-event "))
        .unwrap_or_else(|| panic!("not the keyed managed route: {command}"));
    assert!(
        key.len() == 64
            && key
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "{command}"
    );
    quoted_arcctl(quoted, "");
    key.to_owned()
}

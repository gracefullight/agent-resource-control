//! Installed-hook fixture setup, response checks and child observations.
use crate::{
    journey::{output_with_input, Sandbox},
    runtime::{child_watchdog, Fixture},
    stage,
};
use serde_json::{json, Value};
use std::{
    fs,
    os::{
        fd::AsRawFd,
        unix::{ffi::OsStrExt, fs::OpenOptionsExt},
    },
    path::Path,
};

/// POSIX write locks on capability advertisements beside the socket, held by
/// this test process as a daemon advertising exactly those capabilities
/// would hold them. Closing the files releases the locks.
pub(super) struct Advertised(#[allow(dead_code)] Vec<fs::File>);

pub(super) fn advertise(f: &Fixture, names: &[&str]) -> Advertised {
    let directory = f.socket.parent().unwrap();
    Advertised(
        names
            .iter()
            .map(|name| {
                let file = fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .create(true)
                    .truncate(false)
                    .mode(0o600)
                    .open(directory.join(name))
                    .unwrap();
                // SAFETY: a zeroed flock is valid; the descriptor is live.
                let mut lock: libc::flock = unsafe { std::mem::zeroed() };
                lock.l_type = libc::F_WRLCK as _;
                lock.l_whence = libc::SEEK_SET as _;
                assert_eq!(
                    unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETLK, &lock) },
                    0,
                    "advertisement {name} is already held"
                );
                file
            })
            .collect(),
    )
}

pub(super) fn install(f: &Fixture, s: &Sandbox, project: &Path, vendor: &str) -> String {
    let (folder, file) = if vendor == "claude" {
        (".claude", "settings.json")
    } else {
        (".codex", "hooks.json")
    };
    let dir = s.home.join(folder);
    fs::create_dir_all(&dir).unwrap();
    let original = json!({"description":"keep", "hooks":{"PreToolUse":[{"matcher":"Other", "hooks":[{"type":"command","command":"existing"}]}], "PostToolUse":[{"hooks":[{"type":"command","command":"other"}]}]}});
    fs::write(dir.join(file), serde_json::to_vec(&original).unwrap()).unwrap();
    let output = child_watchdog::output(
        s.arcctl(f, project)
            .args(["hooks", "install", vendor, "--scope", "user"]),
        "install hook",
    )
    .unwrap();
    assert!(output.status.success(), "{output:?}");
    let again = child_watchdog::output(
        s.arcctl(f, project)
            .args(["hooks", "install", vendor, "--scope", "user"]),
        "reinstall hook",
    )
    .unwrap();
    assert!(again.status.success(), "{again:?}");
    let config: Value = serde_json::from_slice(&fs::read(dir.join(file)).unwrap()).unwrap();
    assert_eq!(config["hooks"]["PreToolUse"].as_array().unwrap().len(), 2);
    assert_eq!(config["description"], original["description"]);
    assert_eq!(
        config["hooks"]["PostToolUse"],
        original["hooks"]["PostToolUse"]
    );
    assert_eq!(
        config["hooks"]["PreToolUse"][0],
        original["hooks"]["PreToolUse"][0]
    );
    let entry = &config["hooks"]["PreToolUse"][1];
    assert_eq!(entry["matcher"], "Bash");
    entry["hooks"][0]["command"].as_str().unwrap().to_owned()
}

pub(super) fn event(project: &Path, vendor: &str, suffix: &str) -> Value {
    let mut event = if vendor == "claude" {
        crate::pre_tool_use(project, "cargo build")
    } else {
        json!({"session_id":"codex-session-private-fixture", "turn_id":"turn-fixture", "tool_use_id":"codex-tool-private-fixture", "cwd":project, "hook_event_name":"PreToolUse", "permission_mode":"default", "tool_name":"Bash", "tool_input":{"command":"bun run build", "workdir":project, "timeout_ms":9000, "nested":{"preserve":true}}})
    };
    event["session_id"] = json!(format!("{vendor}-session-private-{suffix}"));
    event["tool_use_id"] = json!(format!("{vendor}-tool-private-{suffix}"));
    event
}

pub(super) fn reply(
    f: &Fixture,
    s: &Sandbox,
    project: &Path,
    hook: &str,
    vendor: &str,
    event: &Value,
) -> String {
    let input = serde_json::to_vec(event).unwrap();
    let out = output_with_input(&mut s.shell(f, project, hook), &input, "installed hook");
    assert!(out.status.success() && out.stderr.is_empty(), "{out:?}");
    let response: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(response.as_object().unwrap().len(), 1);
    let specific = &response["hookSpecificOutput"];
    assert_eq!(specific["hookEventName"], "PreToolUse");
    if vendor == "codex" {
        assert_eq!(specific.as_object().unwrap().len(), 3);
        assert_eq!(specific["permissionDecision"], "allow");
    } else {
        assert_eq!(specific.as_object().unwrap().len(), 2);
        assert!(specific.get("permissionDecision").is_none());
    }
    let mut restored = specific["updatedInput"].clone();
    let command = restored["command"].as_str().unwrap().to_owned();
    restored["command"] = event["tool_input"]["command"].clone();
    assert_eq!(restored, event["tool_input"]);
    for field in ["session_id", "tool_use_id"] {
        assert!(!command.contains(event[field].as_str().unwrap()));
    }
    stage::assert_managed(&command, event["tool_input"]["command"].as_str().unwrap());
    command
}

pub(super) fn verify_finish(
    finished: &stage::Finished,
    observed: &stage::Observed,
    shell: &std::process::Command,
    project: &Path,
    vendor: &str,
) {
    assert_eq!(finished.status.code(), Some(stage::TOOL_EXIT));
    assert_eq!(finished.stdout, stage::TOOL_STDOUT);
    assert!(finished.stderr.ends_with(stage::TOOL_STDERR));
    stage::assert_environment(observed, shell);
    assert_eq!(
        observed.cwd,
        fs::canonicalize(project).unwrap().as_os_str().as_bytes()
    );
    assert_eq!(observed.stdin, stage::INPUT);
    let args: Vec<Vec<u8>> = if vendor == "claude" {
        vec![b"build".to_vec()]
    } else {
        vec![b"run".to_vec(), b"build".to_vec()]
    };
    assert_eq!(observed.args, args);
}

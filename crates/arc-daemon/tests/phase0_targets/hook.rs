//! Timed runs of the installed hook and of a reference no-op process (NFR-003).
//!
//! Both go through one function: the same pipes, the same isolated
//! environment and a blocking wait, so the difference between the two sample
//! sets is what the hook adds to a process start on this host.
use crate::{
    journey::Sandbox,
    runtime::{child_watchdog, Fixture},
    watchdog::Watchdog,
};
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
    process::{Command, Output, Stdio},
    time::Instant,
};

/// Install the Claude hook into the sandbox HOME with the built `arcctl` and
/// return the command the vendor would run for each Bash tool call.
pub fn install(f: &Fixture, s: &Sandbox, project: &Path) -> String {
    let install = child_watchdog::output(
        s.arcctl(f, project)
            .args(["hooks", "install", "claude", "--scope", "user"]),
        "install Claude hook",
    )
    .unwrap();
    assert!(install.status.success(), "{install:?}");
    let settings: Value =
        serde_json::from_slice(&fs::read(s.home.join(".claude/settings.json")).unwrap()).unwrap();
    let entry = &settings["hooks"]["PreToolUse"][0];
    assert_eq!(entry["matcher"], "Bash", "{settings}");
    entry["hooks"][0]["command"].as_str().unwrap().to_owned()
}

/// The recorded Claude Code 2.1.327 PreToolUse event, re-addressed to the
/// fixture project and carrying `command`.
pub fn event(project: &Path, command: &str) -> Vec<u8> {
    let mut event: Value = serde_json::from_str(include_str!(
        "../../../arc-cli/tests/fixtures/agent_hooks/claude-recorded-2.1.327-cargo-build.json"
    ))
    .unwrap();
    event["cwd"] = json!(project);
    event["tool_input"]["command"] = json!(command);
    serde_json::to_vec(&event).unwrap()
}

/// A system no-op executable: the reference cost of starting any process.
pub fn no_op() -> &'static Path {
    ["/usr/bin/true", "/bin/true"]
        .into_iter()
        .map(Path::new)
        .find(|path| path.is_file())
        .expect("a system no-op executable")
}

/// Start `command`, feed it `input`, read both streams to their end and reap
/// it. The sample spans spawn to exit status; nothing in it polls.
pub fn timed(command: &mut Command, input: &[u8], label: &'static str) -> (u64, Output) {
    command
        .stdin(if input.is_empty() {
            Stdio::null()
        } else {
            Stdio::piped()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let start = Instant::now();
    let mut child = command.spawn().unwrap();
    let watchdog = Watchdog::arm(child.id(), label);
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input).unwrap();
    }
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    let mut out = child.stdout.take().unwrap();
    out.read_to_end(&mut stdout).unwrap();
    let mut err = child.stderr.take().unwrap();
    err.read_to_end(&mut stderr).unwrap();
    let status = child.wait().unwrap();
    let elapsed = u64::try_from(start.elapsed().as_nanos()).unwrap();
    watchdog.disarm();
    (
        elapsed,
        Output {
            status,
            stdout,
            stderr,
        },
    )
}

//! The two agent Test literals through the built `arcctl`, for both vendors.
//!
//! Every process gets a cleared environment, a temporary HOME, an offline
//! stub PATH and a socket path where no daemon listens.
#[path = "fixtures/agent_hooks/event_key.rs"]
mod event_key;

use serde_json::{json, Value};
use std::{
    fs::{self, File},
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

const OUTAGE: &str = "TRACKING_UNAVAILABLE\nCommand started. ARC tracking is unavailable. A lifecycle record may be absent.\nDo not launch another copy to recover tracking.\n";

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        // Short, so the socket path below fits a socket address.
        let id = arc_core::process_identity::random_job_id().unwrap();
        let root = std::env::temp_dir().join(format!("atl-{}", &id[..12]));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        for directory in ["bin", "home", "project"] {
            fs::create_dir(root.join(directory)).unwrap();
        }
        // Offline tools: each start is one line, then fixed bytes and exit 17.
        for tool in ["cargo", "bun", "docker", "gh", "vitest"] {
            let path = root.join("bin").join(tool);
            let starts = root.join("starts");
            fs::write(
                &path,
                format!(
                    "#!/bin/sh\necho \"{tool} $*\" >> '{}'\necho tool-stdout\necho tool-stderr >&2\nexit 17\n",
                    starts.display()
                ),
            )
            .unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        Self { root }
    }

    fn arcctl(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_arcctl"));
        command
            .env_clear()
            .env("HOME", self.root.join("home"))
            .env("PATH", self.root.join("bin"))
            .env(arc_core::ARC_SOCKET_PATH, self.root.join("absent.sock"))
            .current_dir(self.root.join("project"));
        if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
            command.env("LLVM_PROFILE_FILE", profile);
        }
        command
    }

    /// Run to exit under a 60 s diagnostic watchdog.
    fn output(&self, command: &mut Command, input: &[u8]) -> Output {
        let (stdin, stdout, stderr) = ("stdin", "stdout", "stderr");
        fs::write(self.root.join(stdin), input).unwrap();
        command
            .stdin(Stdio::from(File::open(self.root.join(stdin)).unwrap()))
            .stdout(File::create(self.root.join(stdout)).unwrap())
            .stderr(File::create(self.root.join(stderr)).unwrap());
        let mut child = command.spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(60);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("{command:?} exceeded the 60 s watchdog");
            }
            thread::sleep(Duration::from_millis(2));
        };
        Output {
            status,
            stdout: fs::read(self.root.join(stdout)).unwrap(),
            stderr: fs::read(self.root.join(stderr)).unwrap(),
        }
    }

    fn starts(&self) -> Vec<String> {
        fs::read_to_string(self.root.join("starts"))
            .map(|text| text.lines().map(str::to_owned).collect())
            .unwrap_or_default()
    }

    /// The command the vendor would execute after the hook saw `command`:
    /// the update, or `None` when the hook leaves the tool input alone.
    fn hook(&self, vendor: &str, command: &str) -> Option<String> {
        self.hook_event(vendor, command, recorded(vendor))
    }

    /// `hook` for `event`, a recorded payload or a mutation derived from one.
    fn hook_event(&self, vendor: &str, command: &str, mut event: Value) -> Option<String> {
        if vendor == "claude" {
            event["cwd"] = json!(self.root.join("project"));
        }
        event["tool_input"]["command"] = json!(command);
        let output = self.output(
            self.arcctl().args(["hooks", "run", vendor]),
            &serde_json::to_vec(&event).unwrap(),
        );
        assert!(output.stderr.is_empty(), "{vendor} {command}: {output:?}");
        assert_eq!(self.starts(), [""; 0], "the hook ran {command}");
        if output.stdout.is_empty() {
            // The documented no-change exits: Claude 0, Cursor's fail-open 1.
            let unchanged = if vendor == "claude" { 0 } else { 1 };
            assert_eq!(output.status.code(), Some(unchanged), "{vendor} {command}");
            return None;
        }
        assert!(output.status.success(), "{vendor} {command}: {output:?}");
        let reply: Value = serde_json::from_slice(&output.stdout).unwrap();
        // Only the command is replaced and no permission decision is made.
        let mut restored = event["tool_input"].clone();
        let updated = if vendor == "claude" {
            &reply["hookSpecificOutput"]["updatedInput"]
        } else {
            &reply["updated_input"]
        };
        let rewritten = updated["command"].as_str().unwrap().to_owned();
        restored["command"] = json!(rewritten);
        let expected = if vendor == "claude" {
            json!({"hookSpecificOutput": {"hookEventName": "PreToolUse", "updatedInput": restored}})
        } else {
            json!({ "updated_input": restored })
        };
        assert_eq!(reply, expected, "{vendor} {command}");
        Some(rewritten)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if !thread::panicking() {
            fs::remove_dir_all(&self.root).unwrap();
        }
    }
}

/// The recorded PreToolUse payload of `vendor`.
fn recorded(vendor: &str) -> Value {
    serde_json::from_str(if vendor == "claude" {
        include_str!("fixtures/agent_hooks/claude-recorded-2.1.327-cargo-build.json")
    } else {
        include_str!("fixtures/agent_hooks/cursor/recorded/preToolUse-cargo-build-rewrite.json")
    })
    .unwrap()
}

/// The four whole commands whose managed rewrite names its event.
const KEYED: [&str; 4] = ["cargo build", "bun run build", "cargo test", "bun run test"];

/// The one-child managed command for `literal` as `vendor`'s recorded event
/// asks for it: the four keyed literals name that event, nothing else does.
fn managed(vendor: &str, literal: &str) -> String {
    let route = if KEYED.contains(&literal) {
        event_key::route(vendor, &recorded(vendor))
    } else {
        " hook-exec -- ".to_owned()
    };
    format!("'{}'{route}{literal}", env!("CARGO_BIN_EXE_arcctl"))
}

fn arcctl_path_is_plain() {
    assert!(!Path::new(env!("CARGO_BIN_EXE_arcctl"))
        .to_str()
        .unwrap()
        .contains('\''));
}

#[test]
fn built_hook_rewrites_exactly_cargo_test_and_bun_run_test_for_both_vendors() {
    arcctl_path_is_plain();
    let fixture = Fixture::new();
    for vendor in ["claude", "cursor"] {
        for literal in ["cargo test", "bun run test"] {
            assert_eq!(
                fixture.hook(vendor, literal),
                Some(managed(vendor, literal)),
                "{vendor}"
            );
        }
        // A Test command in any other spelling keeps today's pass-through.
        for other in [
            "cargo test -p arc-cli",
            "cargo test --workspace",
            "cargo test --",
            "cargo  test",
            " cargo test",
            "cargo test ",
            "cargo test > test.log",
            "cargo test; cargo build",
            "CARGO_TERM_COLOR=never cargo test",
            "/usr/local/bin/cargo test",
            "cargo nextest run",
            "bun run test --watch",
            "bun run test:unit",
            "bun run  test",
            "bun --silent run test",
            "bun test",
            "bunx vitest",
            "vitest",
            "vitest run",
            "npm test",
            "make test",
        ] {
            assert_eq!(fixture.hook(vendor, other), None, "{vendor} {other}");
        }
    }
}

/// AT-TRF-09, ADR-014: the returned command names the event by a digest of
/// the tuple, directly after `hook-exec`. The same event gives the same
/// command, another tool call another key, and a payload without both ids
/// exactly the command it had before. Neither raw id is ever written.
#[test]
fn built_hook_names_the_event_only_when_the_payload_carries_both_ids() {
    arcctl_path_is_plain();
    let fixture = Fixture::new();
    let today =
        |literal: &str| format!("'{}' hook-exec -- {literal}", env!("CARGO_BIN_EXE_arcctl"));
    for vendor in ["claude", "cursor"] {
        let event = recorded(vendor);
        let key = event_key::expected(vendor, &event).unwrap();
        // Pinned for the two recordings: another domain, framing or field
        // order would give one event two keys across `arcctl` versions.
        assert_eq!(
            key,
            if vendor == "claude" {
                "d320d29c55508336cfbafff39151f2d0ca29bdcb9c09aaae34eca307278b1130"
            } else {
                "2e2eba146ea3c8b6691e3e5299d6d50598225287a0bf54454a81a8bf235786e0"
            }
        );
        let (session, tool) = (
            event["session_id"].as_str().unwrap(),
            event["tool_use_id"].as_str().unwrap(),
        );
        for literal in KEYED {
            let keyed = format!(
                "'{}' hook-exec --arc-event {key} -- {literal}",
                env!("CARGO_BIN_EXE_arcctl")
            );
            for _delivery in 0..2 {
                let command = fixture.hook(vendor, literal).unwrap();
                assert_eq!(command, keyed, "{vendor}");
                assert!(!command.contains(session) && !command.contains(tool));
            }
            // Another tool call of the same session is another event.
            let mut other = event.clone();
            other["tool_use_id"] = json!(format!("{tool}-next"));
            let other_key = event_key::expected(vendor, &other).unwrap();
            assert_ne!(other_key, key);
            assert_eq!(
                fixture.hook_event(vendor, literal, other),
                Some(keyed.replace(&key, &other_key)),
                "{vendor}"
            );
            // No identity: exactly the command returned before.
            for (field, value) in [
                ("session_id", None),
                ("tool_use_id", None),
                ("session_id", Some(json!(""))),
                ("tool_use_id", Some(json!(""))),
                ("session_id", Some(json!(7))),
                ("tool_use_id", Some(json!(null))),
            ] {
                let mut incomplete = event.clone();
                match value {
                    Some(value) => incomplete[field] = value,
                    None => {
                        incomplete.as_object_mut().unwrap().remove(field).unwrap();
                    }
                }
                assert_eq!(
                    fixture.hook_event(vendor, literal, incomplete),
                    Some(today(literal)),
                    "{vendor} {literal} {field}"
                );
            }
        }
    }
    // The two adapters never share a key for the same ids.
    let mut cursor = recorded("cursor");
    let claude = recorded("claude");
    cursor["session_id"] = claude["session_id"].clone();
    cursor["tool_use_id"] = claude["tool_use_id"].clone();
    assert_ne!(
        event_key::expected("cursor", &cursor),
        event_key::expected("claude", &claude)
    );
}

#[test]
fn built_hook_keeps_the_existing_rewrites_beside_the_test_literals() {
    arcctl_path_is_plain();
    let fixture = Fixture::new();
    let quoted = format!("'{}'", env!("CARGO_BIN_EXE_arcctl"));
    for vendor in ["claude", "cursor"] {
        for literal in [
            "cargo build",
            "bun run build",
            "docker build .",
            "docker buildx build .",
        ] {
            assert_eq!(
                fixture.hook(vendor, literal),
                Some(managed(vendor, literal)),
                "{vendor}"
            );
        }
        // The gh literal keeps its shell-resolving wrapper around the route.
        let gh = fixture.hook(vendor, "gh workflow run build.yml").unwrap();
        assert!(
            gh.starts_with("case \"$-\" in *) ")
                && gh.contains(&format!("{quoted} hook-exec -- --arc-check-gh-path "))
                && gh.contains(" hook-exec -- gh workflow run build.yml; }")
                && !gh.contains("cargo")
                && !gh.contains("bun"),
            "{vendor}: {gh}"
        );
        // Each mixed compound keeps its two request-linked stages.
        for test in ["bun test", "make test"] {
            let stages = fixture
                .hook(vendor, &format!("{test} && gh workflow run ci.yml"))
                .unwrap();
            let request = stages
                .strip_prefix(&format!("{quoted} hook-exec --arc-request "))
                .and_then(|rest| rest.split_once(' '))
                .map(|(request, _)| request.to_owned())
                .unwrap_or_else(|| panic!("{vendor}: {stages}"));
            assert_eq!(
                stages,
                format!(
                    "{quoted} hook-exec --arc-request {request} -- {test} && \
                     {quoted} hook-exec --arc-request {request} -- gh workflow run ci.yml"
                ),
                "{vendor}"
            );
        }
        for other in [
            "cargo build --release",
            "bun run lint",
            "ls",
            "docker build",
        ] {
            assert_eq!(fixture.hook(vendor, other), None, "{vendor} {other}");
        }
    }
}

#[test]
fn hook_exec_accepts_exactly_the_two_test_argv_forms() {
    let fixture = Fixture::new();
    for rejected in [
        &["cargo", "test", "--workspace"][..],
        &["cargo", "test", "-p", "arc-cli"],
        &["cargo test"],
        &["bun", "run", "test", "--watch"],
        &["bun", "run", "test:unit"],
        &["vitest"],
        &["vitest", "run"],
    ] {
        let output = fixture.output(
            fixture.arcctl().args(["hook-exec", "--"]).args(rejected),
            b"",
        );
        assert_eq!(output.status.code(), Some(2), "{rejected:?}: {output:?}");
        assert!(output.stdout.is_empty(), "{rejected:?}");
        assert!(output.stderr.starts_with(b"INVALID_REQUEST"), "{output:?}");
    }
    assert_eq!(fixture.starts(), [""; 0], "a rejected argv started a tool");

    // With no daemon each accepted form still runs its tool exactly once,
    // with its own arguments, streams and exit status.
    fs::write(
        fixture.root.join("project/package.json"),
        r#"{"scripts":{"test":"vitest"}}"#,
    )
    .unwrap();
    for (index, accepted) in [&["cargo", "test"][..], &["bun", "run", "test"]]
        .into_iter()
        .enumerate()
    {
        let output = fixture.output(
            fixture.arcctl().args(["hook-exec", "--"]).args(accepted),
            b"",
        );
        assert_eq!(output.status.code(), Some(17), "{accepted:?}: {output:?}");
        assert_eq!(output.stdout, b"tool-stdout\n", "{accepted:?}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert_eq!(
            stderr.replacen(OUTAGE, "", 1),
            "tool-stderr\n",
            "{accepted:?}: {stderr}"
        );
        assert_eq!(stderr.matches("TRACKING_").count(), 1, "{stderr}");
        assert_eq!(fixture.starts().len(), index + 1, "{accepted:?}");
        assert_eq!(fixture.starts()[index], accepted.join(" "));
    }
}

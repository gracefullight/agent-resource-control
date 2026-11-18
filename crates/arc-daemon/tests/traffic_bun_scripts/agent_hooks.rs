//! Documented vendor-event fixtures, controlled shells and production admission.
//! The proof supplied here applies only to these fresh, non-login fixture shells.
use super::{projects::*, *};
use std::{ffi::OsStr, os::unix::fs::OpenOptionsExt};

#[allow(dead_code)]
#[path = "../../../arc-cli/src/hooks.rs"]
mod hooks;

#[test]
fn hook_transform_worker() {
    if !hooks::fixture::worker_enabled() {
        return;
    }
    let vendor = match std::env::var("HOOK_TEST_VENDOR").unwrap().as_str() {
        "claude" => hooks::Vendor::Claude,
        "cursor" => hooks::Vendor::Cursor,
        other => panic!("unknown fixture vendor {other}"),
    };
    let context = hooks::HookContext {
        executable: Some(PathBuf::from(std::env::var_os("HOOK_TEST_ARCCTL").unwrap())),
    };
    let path = std::env::var_os("HOOK_TEST_PATH").unwrap();
    let surface = hooks::VerifiedSurface {
        command_path: &path,
    };
    let event = fs::read(std::env::var_os("HOOK_TEST_EVENT").unwrap()).unwrap();
    let response = hooks::dispatch_verified_event(vendor, &event, &context, &surface).unwrap();
    fs::write(
        std::env::var_os("HOOK_TEST_RESPONSE").unwrap(),
        serde_json::to_vec(&response).unwrap(),
    )
    .unwrap();
    println!("ARC_AGENT_HOOK_TRANSFORM_COMPLETE");
}

fn node_executable(bun: &Path) -> PathBuf {
    let mut candidates = vec![
        PathBuf::from("/opt/homebrew/bin/node"),
        PathBuf::from("/usr/local/bin/node"),
        PathBuf::from("/usr/bin/node"),
    ];
    if let Some(mise) = bun
        .ancestors()
        .find(|path| path.file_name() == Some(OsStr::new("mise")))
    {
        candidates.push(mise.join("installs/node/latest/bin/node"));
    }
    candidates
        .into_iter()
        .filter_map(|path| path.canonicalize().ok())
        .find(|path| path.is_file() && path.file_name() == Some(OsStr::new("node")))
        .expect("Node executable required for Vite")
}

fn host_path() -> std::ffi::OsString {
    // Find the pinned installed tools without importing the runner's PATH or
    // HOME. The ordinary shared fixture still verifies their versions.
    let cargo = PathBuf::from(env!("CARGO")).canonicalize().unwrap();
    let mut bun = vec![
        PathBuf::from("/opt/homebrew/bin/bun"),
        PathBuf::from("/usr/local/bin/bun"),
        PathBuf::from("/usr/bin/bun"),
    ];
    if let Some(rustup) = cargo
        .ancestors()
        .find(|path| path.file_name() == Some(OsStr::new(".rustup")))
    {
        bun.insert(
            0,
            rustup
                .parent()
                .unwrap()
                .join(".local/share/mise/installs/bun/1.4.4/bin/bun"),
        );
    }
    let bun = bun
        .into_iter()
        .filter_map(|path| path.canonicalize().ok())
        .find(|path| path.is_file() && path.file_name() == Some(OsStr::new("bun")))
        .expect("pinned Bun executable required for fixture");
    let node = node_executable(&bun);
    std::env::join_paths([
        bun.parent().unwrap(),
        cargo.parent().unwrap(),
        node.parent().unwrap(),
        Path::new("/usr/bin"),
        Path::new("/bin"),
        Path::new("/usr/sbin"),
        Path::new("/sbin"),
    ])
    .unwrap()
}

/// The outer test holds the usual shared fixture lock while this child runs.
pub fn run_isolated(worker_name: &str) {
    let mut worker = Command::new(std::env::current_exe().unwrap());
    worker
        .args(["--exact", worker_name, "--nocapture"])
        .env_clear()
        .env("PATH", host_path())
        .env("HOME", "/tmp")
        .env("TMPDIR", "/tmp");
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        worker.env("LLVM_PROFILE_FILE", profile);
    }
    let _handshake = hooks::fixture::worker_handshake(&mut worker);
    let output = projects::fixture_output(&mut worker).unwrap();
    assert!(
        output.status.success(),
        "isolated {worker_name}: {output:?}"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(&format!("test {worker_name} ..."))
            && stdout.contains("test result: ok. 1 passed;")
            && stdout.contains("ARC_AGENT_HOOK_ORDER_COMPLETE"),
        "worker did not execute: {output:?}"
    );
    print!("{}", String::from_utf8_lossy(&output.stdout));
}

#[test]
fn isolated_worker_parent_observes_execution() {
    let thread = std::thread::current();
    let name = thread.name().unwrap();
    let prefix = name
        .strip_suffix("isolated_worker_parent_observes_execution")
        .unwrap();
    run_isolated(&format!("{prefix}isolated_worker_probe"));
}

#[test]
fn isolated_worker_probe() {
    if hooks::fixture::worker_enabled() {
        println!("ARC_AGENT_HOOK_ORDER_COMPLETE");
    }
}

fn command_path(case: &Case) -> std::ffi::OsString {
    // The fixture shell gets an explicit PATH, never the test runner's PATH.
    let mut dirs = vec![
        case.tools.shims.clone(),
        case.tools.cargo.parent().unwrap().to_owned(),
    ];
    let node = node_executable(&case.tools.bun);
    dirs.push(node.parent().unwrap().to_owned());
    dirs.extend(
        [
            "/opt/homebrew/bin",
            "/usr/local/bin",
            "/usr/bin",
            "/bin",
            "/usr/sbin",
            "/sbin",
        ]
        .map(PathBuf::from),
    );
    std::env::join_paths(dirs).unwrap()
}

fn transform(project: &Path, vendor: &str, literal: &str, path: &OsStr) -> String {
    let source = match vendor {
        "claude" => {
            include_str!("../../../arc-cli/tests/fixtures/agent_hooks/claude-pre-tool-use.json")
        }
        "cursor" => {
            include_str!("../../../arc-cli/tests/fixtures/agent_hooks/cursor-pre-tool-use.json")
        }
        _ => unreachable!(),
    };
    let mut event: Value = serde_json::from_str(source).unwrap();
    event["cwd"] = json!(project);
    event["tool_input"]["command"] = json!(literal);
    if vendor == "cursor" {
        event["tool_input"]["working_directory"] = json!(project);
    }
    let input = project.join("event.json");
    let response = project.join("response.json");
    write(&input, serde_json::to_vec(&event).unwrap());
    let mut worker = Command::new(std::env::current_exe().unwrap());
    let thread = thread::current();
    let test_name = thread.name().unwrap_or_default();
    let worker_name = match test_name.split_once("::agent_hooks::") {
        Some((prefix, _)) => format!("{prefix}::agent_hooks::hook_transform_worker"),
        None => "suite::agent_hooks::hook_transform_worker".into(),
    };
    worker
        .args(["--exact", &worker_name, "--nocapture"])
        .env_clear()
        .env("HOOK_TEST_VENDOR", vendor)
        .env("HOOK_TEST_ARCCTL", binary("arcctl"))
        .env("HOOK_TEST_PATH", path)
        .env("HOOK_TEST_EVENT", &input)
        .env("HOOK_TEST_RESPONSE", &response);
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        worker.env("LLVM_PROFILE_FILE", profile);
    }
    let _handshake = hooks::fixture::worker_handshake(&mut worker);
    let output = projects::fixture_output(&mut worker).unwrap();
    assert!(output.status.success(), "fixture transformer: {output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(&format!("test {worker_name} ..."))
            && stdout.contains("test result: ok. 1 passed;")
            && stdout.contains("ARC_AGENT_HOOK_TRANSFORM_COMPLETE"),
        "fixture transformer did not execute: {output:?}"
    );
    assert!(!project.join("launches").exists(), "hook started a command");
    let result: Value = serde_json::from_slice(&fs::read(response).unwrap()).unwrap();
    let updated = if vendor == "claude" {
        assert!(result["hookSpecificOutput"]
            .get("permissionDecision")
            .is_none());
        &result["hookSpecificOutput"]["updatedInput"]
    } else {
        assert!(result.get("permission").is_none());
        &result["updated_input"]
    };
    let command = updated["command"].as_str().unwrap().to_owned();
    let mut original = updated.clone();
    original["command"] = json!(literal);
    assert_eq!(original, event["tool_input"]);
    assert!(command.ends_with(&format!(" hook-exec -- {literal}")));
    command
}

#[test]
fn transformer_parent_observes_both_vendor_workers() {
    struct Directory(PathBuf);
    impl Drop for Directory {
        fn drop(&mut self) {
            if !thread::panicking() {
                fs::remove_dir_all(&self.0).unwrap();
            } else {
                eprintln!("retained transformer fixture {}", self.0.display());
            }
        }
    }
    let directory = Directory(std::env::temp_dir().join(format!(
        "arc-transform-{}",
        arc_core::process_identity::random_job_id().unwrap()
    )));
    fs::create_dir(&directory.0).unwrap();
    for tool in ["cargo", "bun"] {
        let path = directory.0.join(tool);
        fs::write(&path, b"#!/bin/sh\nexit 0\n").unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    for (vendor, literal) in [("claude", "cargo build"), ("cursor", "bun run build")] {
        assert!(
            transform(&directory.0, vendor, literal, directory.0.as_os_str())
                .ends_with(&format!(" hook-exec -- {literal}"))
        );
    }
}

fn prepare(case: &Case, name: &str, kind: Kind) -> PathBuf {
    let project = project(case, name, kind, true);
    if kind == Kind::Cargo {
        // A blocking FIFO lets the parent observe entry before granting release.
        write(
            &project.join("build.rs"),
            r#"
use std::{fs::{File,OpenOptions}, io::{Read,Write}};
fn main() {
    writeln!(OpenOptions::new().append(true).create(true).open(std::env::var_os("TBS_BODY").unwrap()).unwrap(), "{}", std::process::id()).unwrap();
    let mut byte = [0u8; 1];
    File::open(std::env::var_os("TBS_RELEASE").unwrap()).unwrap().read_exact(&mut byte).unwrap();
    assert_eq!(byte, [b'R']);
}
"#,
        );
    } else {
        write(
            &project.join("package.json"),
            json!({"type":"module","scripts":{"build":"vite build"}}).to_string(),
        );
        write(
            &project.join("vite.config.mjs"),
            r#"
import { appendFileSync, readFileSync } from 'node:fs';
export default { plugins: [{ name: 'arc-fixture-gate', buildStart() {
  appendFileSync(process.env.TBS_BODY, `${process.pid}\n`);
  if (readFileSync(process.env.TBS_RELEASE, 'utf8') !== 'R') throw new Error('invalid release');
} }] };
"#,
        );
    }
    let gate = std::ffi::CString::new(project.join("release").as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(gate.as_ptr(), 0o600) }, 0);
    project
}

fn start(
    case: &Case,
    project: &Path,
    command: &str,
    path: &OsStr,
    predecessor: Option<(&str, &Value)>,
) -> Run {
    let mut shell = Command::new("/bin/sh");
    shell
        .args(["-c", &format!("exec {command}")])
        .env_clear()
        .current_dir(project)
        .env("PATH", path)
        .env("HOME", &case.fixture.root)
        .env("CARGO_HOME", case.fixture.root.join("cargo-home"))
        .env("CARGO_NET_OFFLINE", "true")
        .env(arc_core::ARC_SOCKET_PATH, &case.fixture.socket)
        .env(arc_core::ARC_DB_PATH, &case.fixture.database)
        .env("TBS_PROJECT", project)
        .env("TBS_BODY", project.join("bodies"))
        .env("TBS_RELEASE", project.join("release"))
        .env("TBS_ARCCTL", binary("arcctl"))
        .env("CARGO_TARGET_DIR", project.join("target"))
        .stdin(Stdio::null())
        .stdout(File::create(project.join("stdout")).unwrap())
        .stderr(File::create(project.join("stderr")).unwrap());
    if command.ends_with(" -- cargo build") {
        shell.env("CARGO_INCREMENTAL", "0");
    }
    if let Some((id, entry)) = predecessor {
        shell
            .env("TBS_PREDECESSOR", id)
            .env("TBS_PREDECESSOR_PID", entry["child"]["pid"].to_string());
    }
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        shell.env("LLVM_PROFILE_FILE", profile);
    }
    let wrapper = native::OwnedChild::new(shell.spawn().unwrap());
    let identity = ProcessIdentity::capture(wrapper.child.id()).unwrap();
    Run {
        wrapper,
        identity,
        project: project.to_owned(),
        fifo: None,
    }
}

fn release(run: &mut Run) {
    let mut writer = None;
    eventually(|| {
        assert!(
            run.wrapper.child.try_wait().unwrap().is_none(),
            "child exited before release: {}",
            log(&run.project.join("stderr"))
        );
        match OpenOptions::new()
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(run.project.join("release"))
        {
            Ok(file) => {
                writer = Some(file);
                true
            }
            Err(error) if error.raw_os_error() == Some(libc::ENXIO) => false,
            Err(error) => panic!("open release FIFO: {error}"),
        }
    });
    writer.unwrap().write_all(b"R").unwrap();
}

fn install_permit_witness(case: &Case) {
    let mut sqlite = Command::new("/usr/bin/sqlite3");
    sqlite.arg(&case.fixture.database).args(["-cmd", ".timeout 2000", "CREATE TABLE hook_permit_observations (active INTEGER NOT NULL); CREATE TRIGGER hook_permit_insert AFTER INSERT ON permits BEGIN INSERT INTO hook_permit_observations SELECT count(*) FROM permits WHERE state='ACTIVE'; END; CREATE TRIGGER hook_permit_update AFTER UPDATE ON permits BEGIN INSERT INTO hook_permit_observations SELECT count(*) FROM permits WHERE state='ACTIVE'; END;"]);
    let output = projects::fixture_output(&mut sqlite).unwrap();
    assert!(output.status.success(), "permit witness setup: {output:?}");
}

/// Registered as a normal test, but only its handshaken parent runs the body.
pub fn run_worker(read: Read, claude_first: bool) {
    if !hooks::fixture::worker_enabled() {
        return;
    }
    builds(read, claude_first);
    println!("ARC_AGENT_HOOK_ORDER_COMPLETE");
}

fn builds(read: Read, claude_first: bool) {
    let case = Case::new(
        if claude_first {
            "agent-claude-first"
        } else {
            "agent-cursor-first"
        },
        read,
        false,
    );
    install_permit_witness(&case);
    let order = if claude_first {
        [
            ("claude", Kind::Cargo, "cargo build"),
            ("cursor", Kind::Vite, "bun run build"),
        ]
    } else {
        [
            ("cursor", Kind::Vite, "bun run build"),
            ("claude", Kind::Cargo, "cargo build"),
        ]
    };
    let paths = [
        prepare(&case, "first", order[0].1),
        prepare(&case, "second", order[1].1),
    ];
    let path = command_path(&case);
    let commands = [
        transform(&paths[0], order[0].0, order[0].2, &path),
        transform(&paths[1], order[1].0, order[1].2, &path),
    ];
    let mut first = start(&case, &paths[0], &commands[0], &path, None);
    let first_entry = first.entered(&case, false);
    let first_id = first_entry["jobId"].as_str().unwrap();
    let mut second = start(
        &case,
        &paths[1],
        &commands[1],
        &path,
        Some((first_id, &first_entry)),
    );
    let second_id = case.job(second.wrapper.child.id());
    eventually(|| case.row(&second_id)["state"] == "QUEUED");
    assert!(second.wrapper.child.try_wait().unwrap().is_none());
    assert_eq!(second.count("bun") + second.count("cargo"), 0);
    assert!(!paths[1].join("bodies").exists());
    assert!(cleanup::children(second.wrapper.child.id())
        .unwrap()
        .is_empty());
    assert_eq!(case.active()["permits"], 1);
    let queued = case.reconcile(&second_id, "BUILD", order[1].1.reason());
    assert_eq!(
        queued["diagnostic"]["admission"]["wait_reason"],
        "class_window"
    );
    release(&mut first);
    assert_eq!(
        first.wait().code(),
        Some(0),
        "{}",
        log(&paths[0].join("stderr"))
    );
    let first_done = case.terminal(&first, "BUILD", order[0].1.reason(), 0);
    let second_entry = second.entered(&case, false);
    assert_ne!(first_id, second_id);
    assert!(log(&paths[1].join("release-witness")).contains("RELEASED"));
    assert_eq!(case.active()["permits"], 1);
    release(&mut second);
    assert_eq!(
        second.wait().code(),
        Some(0),
        "{}",
        log(&paths[1].join("stderr"))
    );
    let second_done = case.terminal(&second, "BUILD", order[1].1.reason(), 0);
    for (run, (_, kind, _)) in [(&first, order[0]), (&second, order[1])] {
        assert_eq!(
            run.count(if kind == Kind::Cargo { "cargo" } else { "bun" }),
            1
        );
        assert_eq!(log(&run.project.join("bodies")).lines().count(), 1);
        validate_artifact(&case, &run.project, kind);
    }
    assert_eq!(case.active()["permits"], 0);
    assert_eq!(
        case.rows("SELECT max(active) AS high_water FROM hook_permit_observations")[0]
            ["high_water"],
        1
    );
    case.save("agent-hook-contention", json!({"fixtureOnly":true,"order":[order[0].0,order[1].0],"firstEntry":first_entry,"secondQueued":queued,"secondEntry":second_entry,"terminal":[first_done,second_done],"secondStartsBeforeRelease":0,"permitHighWater":1,"starts":[1,1],"activePermitsAtEnd":0}));
    case.done();
}

//! Offline tools, their observations and the vendor's side of the journey.
//!
//! `cargo` is the native capture fixture: it records what it was started
//! with, writes fixed bytes to both streams and exits 17. Every wait below
//! is on observed state; the 60 s deadlines are diagnostic watchdogs.
use crate::{
    journey::{self, eventually, Sandbox},
    runtime::{child_watchdog::Owned, instrumentation, Fixture},
};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
};

/// The one internal-error record of a fail-open run (contract D8).
pub const TRACKING_UNAVAILABLE: &[u8] = b"TRACKING_UNAVAILABLE\nCommand started. ARC tracking is unavailable. A lifecycle record may be absent.\nDo not launch another copy to recover tracking.\n";
pub const TOOL_STDOUT: &[u8] = b"hook-stdout\0payload\n";
pub const TOOL_STDERR: &[u8] = b"child-stderr\n";
pub const TOOL_EXIT: i32 = 17;
/// Standard input of the agent command, including bytes that are not text.
pub const INPUT: &[u8] = b"stdin\0bytes \xff\n";

/// The offline `cargo` on the sandbox PATH.
pub struct Tool {
    key: PathBuf,
    record: PathBuf,
    marker: PathBuf,
}

/// What one execution of the tool was started with.
#[derive(Debug, PartialEq)]
pub struct Observed {
    pub program: Vec<u8>,
    pub args: Vec<Vec<u8>>,
    pub cwd: Vec<u8>,
    pub stdin: Vec<u8>,
    pub environment: BTreeMap<String, String>,
}

impl Tool {
    pub fn cargo(f: &Fixture, s: &Sandbox) -> Self {
        s.stub("cargo");
        let key = f.root.join("capture-key");
        fs::write(&key, b"arc-hook-fixture-capture-v1").unwrap();
        Self {
            key,
            record: f.root.join("capture-record"),
            marker: f.root.join("capture-starts"),
        }
    }

    /// The agent's own environment for the command, which the tool must see.
    pub fn arm(&self, command: &mut Command) {
        command
            .env("ARC_FIXTURE_MODE", "hook-capture")
            .env("ARC_HOOK_CAPTURE_KEY", &self.key)
            .env("ARC_HOOK_CAPTURE_RECORD", &self.record)
            .env("ARC_HOOK_CAPTURE_MARKER", &self.marker)
            .env("PRIVATE_ENV", "literal $HOME ' value");
    }

    /// Executions so far: one line per start, across every run.
    pub fn starts(&self) -> usize {
        journey::starts(&self.marker)
    }

    /// The observation of the latest execution, consumed so that a later run
    /// cannot be mistaken for it.
    pub fn take(&self) -> Observed {
        let record: Value = serde_json::from_slice(&fs::read(&self.record).unwrap()).unwrap();
        fs::remove_file(&self.record).unwrap();
        let bytes = |value: &Value| serde_json::from_value::<Vec<u8>>(value.clone()).unwrap();
        let text = |value: &Value| String::from_utf8(bytes(value)).unwrap();
        Observed {
            program: bytes(&record["argv0"]),
            args: record["args"]
                .as_array()
                .unwrap()
                .iter()
                .map(bytes)
                .collect(),
            cwd: bytes(&record["cwd"]),
            stdin: bytes(&record["stdin"]),
            environment: record["environment"]
                .as_array()
                .unwrap()
                .iter()
                .map(|pair| (text(&pair[0]), text(&pair[1])))
                .collect(),
        }
    }
}

/// The tool's environment is exactly the one the agent command was started
/// with. The system shell that runs the command owns `_`, `PWD` and `SHLVL`
/// and, being a protected binary, drops the loader variable; those are the
/// same with and without ARC, which the caller compares separately.
pub fn assert_environment(observed: &Observed, launched: &Command) {
    let mut expected: BTreeMap<String, String> = launched
        .get_envs()
        .filter_map(|(key, value)| {
            Some((
                key.to_str().unwrap().to_owned(),
                value?.to_str().unwrap().to_owned(),
            ))
        })
        .collect();
    instrumentation::include_expected(&mut expected);
    let mut actual = observed.environment.clone();
    instrumentation::normalize_actual(&mut actual, &expected);
    for shell_owned in ["_", "PWD", "SHLVL"] {
        actual.remove(shell_owned);
    }
    for loader in ["DYLD_INSERT_LIBRARIES", "LD_PRELOAD"] {
        if !actual.contains_key(loader) {
            expected.remove(loader);
        }
    }
    assert_eq!(actual, expected);
}

/// Two observations of the same command, one of them through ARC.
pub fn assert_same_start(managed: &Observed, direct: &Observed) {
    let normal = |observed: &Observed| {
        let mut environment = observed.environment.clone();
        environment.remove("__LLVM_PROFILE_RT_INIT_ONCE");
        environment
    };
    assert_eq!(normal(managed), normal(direct));
    assert_eq!(
        (
            &managed.program,
            &managed.args,
            &managed.cwd,
            &managed.stdin
        ),
        (&direct.program, &direct.args, &direct.cwd, &direct.stdin)
    );
}

/// One agent command running in the vendor's shell, with its streams.
pub struct Run {
    child: Owned,
    stdout: PathBuf,
    stderr: PathBuf,
}

pub struct Finished {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl Run {
    pub fn start(command: &mut Command, directory: &Path, name: &str) -> Self {
        let stdout = directory.join(format!("{name}.stdout"));
        let stderr = directory.join(format!("{name}.stderr"));
        command
            .stdin(Stdio::piped())
            .stdout(File::create(&stdout).unwrap())
            .stderr(File::create(&stderr).unwrap());
        Self {
            child: Owned::spawn(command).unwrap(),
            stdout,
            stderr,
        }
    }

    pub fn stderr(&self) -> Vec<u8> {
        fs::read(&self.stderr).unwrap()
    }

    /// Whether the command is still running; a finished one is not reaped.
    pub fn alive(&mut self) -> bool {
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        let flags = libc::WEXITED | libc::WNOHANG | libc::WNOWAIT;
        // SAFETY: a non-consuming status query of the child this value owns.
        assert_eq!(
            unsafe { libc::waitid(libc::P_PID, self.child.child.id(), &mut info, flags) },
            0
        );
        #[cfg(target_os = "macos")]
        let exited = info.si_pid != 0;
        #[cfg(not(target_os = "macos"))]
        let exited = unsafe { info.si_pid() } != 0;
        !exited
    }

    /// Wait for observed state while the command is running.
    #[track_caller]
    pub fn until(&mut self, label: &str, mut check: impl FnMut() -> bool) {
        eventually(label, || {
            if check() {
                return true;
            }
            assert!(
                self.alive(),
                "{label}: the command exited first: {}",
                String::from_utf8_lossy(&self.stderr())
            );
            false
        });
    }

    /// Supply standard input and close it.
    pub fn input(&mut self, bytes: &[u8]) {
        let mut stdin = self.child.child.stdin.take().unwrap();
        stdin.write_all(bytes).unwrap();
    }

    pub fn finish(mut self, label: &str) -> Finished {
        drop(self.child.child.stdin.take());
        let status = self.child.wait(label).unwrap();
        Finished {
            status,
            stdout: fs::read(&self.stdout).unwrap(),
            stderr: fs::read(&self.stderr).unwrap(),
        }
    }
}

/// What the vendor does with a PreToolUse hook's reply: no output leaves the
/// tool input as it was; an update may replace the command and nothing else,
/// and ARC never answers with a permission decision.
pub fn vendor_command(reply: &[u8], event: &Value) -> String {
    let original = event["tool_input"]["command"].as_str().unwrap();
    if reply.is_empty() {
        return original.to_owned();
    }
    let reply: Value = serde_json::from_slice(reply).unwrap();
    let hook = &reply["hookSpecificOutput"];
    assert_eq!(reply.as_object().unwrap().len(), 1, "{reply}");
    assert_eq!(hook.as_object().unwrap().len(), 2, "{reply}");
    assert_eq!(hook["hookEventName"], "PreToolUse", "{reply}");
    let mut restored = hook["updatedInput"].clone();
    let command = restored["command"].as_str().unwrap().to_owned();
    restored["command"] = event["tool_input"]["command"].clone();
    assert_eq!(restored, event["tool_input"], "{reply}");
    command
}

/// The managed route for `literal`, and nothing else, through `arcctl`.
#[track_caller]
pub fn assert_managed(command: &str, literal: &str) {
    let arcctl = command
        .strip_suffix(&format!(" hook-exec -- {literal}"))
        .and_then(|quoted| quoted.strip_prefix('\''))
        .and_then(|quoted| quoted.strip_suffix('\''))
        .unwrap_or_else(|| panic!("not the managed route: {command}"));
    assert_eq!(
        fs::canonicalize(arcctl).unwrap(),
        fs::canonicalize(journey::binary("arcctl")).unwrap()
    );
}

pub fn executable(path: &Path, source: &str) {
    fs::write(path, source).unwrap();
    let mut permissions = fs::metadata(path).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
    fs::set_permissions(path, permissions).unwrap();
}

pub fn fifo(path: &Path) {
    use std::os::unix::ffi::OsStrExt;
    let name = std::ffi::CString::new(path.as_os_str().as_bytes()).unwrap();
    // SAFETY: the NUL-terminated path lives through the call.
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
}

/// Let the process blocked reading `gate` continue. Opening fails with ENXIO
/// until that reader exists, so this cannot release anything early.
#[track_caller]
pub fn release(run: &mut Run, gate: &Path) {
    let mut writer = None;
    run.until("a reader at the gate", || {
        match OpenOptions::new()
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(gate)
        {
            Ok(file) => {
                writer = Some(file);
                true
            }
            Err(error) if error.raw_os_error() == Some(libc::ENXIO) => false,
            Err(error) => panic!("open {}: {error}", gate.display()),
        }
    });
    writer.unwrap().write_all(b"go\n").unwrap();
}

/// Every file of the stopped daemon's record: the database with its journal
/// files and the runtime report.
pub fn daemon_record(f: &Fixture) -> BTreeMap<String, Vec<u8>> {
    let database = f.database.file_name().unwrap().to_str().unwrap();
    fs::read_dir(&f.root)
        .unwrap()
        .map(|entry| entry.unwrap())
        .filter_map(|entry| {
            let name = entry.file_name().into_string().unwrap();
            (name.starts_with(database) || name == "runtime.json")
                .then(|| (name, fs::read(entry.path()).unwrap()))
        })
        .collect()
}

/// Names directly inside `directory`, sorted.
pub fn names(directory: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    names
}

/// Install the Claude hook of the `arcctl` at `executable` into the project
/// and return its command.
pub fn install_workspace_hook(
    f: &Fixture,
    s: &Sandbox,
    project: &Path,
    executable: &Path,
) -> String {
    let install = crate::runtime::child_watchdog::output(
        s.command(f, executable, project)
            .args(["hooks", "install", "claude", "--scope", "workspace"])
            .arg("--workspace")
            .arg(project),
        "install workspace Claude hook",
    )
    .unwrap();
    assert!(install.status.success(), "{install:?}");
    let settings: Value =
        serde_json::from_slice(&fs::read(project.join(".claude/settings.json")).unwrap()).unwrap();
    settings["hooks"]["PreToolUse"][0]["hooks"][0]["command"]
        .as_str()
        .unwrap()
        .to_owned()
}

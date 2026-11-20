//! Combined built-CLI and real-daemon endpoint discovery, with offline stubs.
use super::{cli, protocol, runtime::Fixture, support};
use arc_ipc::jobs::{JobId, StatusRequest, OPERATION_JOB_STATUS_LOCALITY};
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Write},
    os::unix::{ffi::OsStrExt, fs::symlink},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

#[allow(clippy::duplicate_mod)]
#[path = "../support/child_watchdog.rs"]
mod child_watchdog;
#[path = "capture_integration/endpoints.rs"]
mod endpoints;
#[path = "capture_integration/git_system.rs"]
mod git_system;
#[path = "capture_integration/held.rs"]
mod held;

struct Sandbox {
    home: PathBuf,
    repo: PathBuf,
    bin: PathBuf,
    marker: PathBuf,
    interfaces: PathBuf,
}

fn write(path: &Path, data: impl AsRef<[u8]>) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, data).unwrap();
}

pub(super) fn fixture_interfaces(f: &Fixture) -> PathBuf {
    // Each fixture gets a private library and a deterministic inventory.
    let source = f.root.join("interfaces.c");
    let interfaces = f.root.join("interfaces-library");
    write(&source, include_str!("capture_integration/interfaces.c"));
    let mut compiler = Command::new("/usr/bin/cc");
    compiler.env_clear().env("PATH", "/usr/bin:/bin");
    if cfg!(target_os = "macos") {
        compiler.arg("-dynamiclib");
    } else {
        compiler.args(["-shared", "-fPIC"]);
    }
    compiler.arg("-o").arg(&interfaces).arg(&source);
    let result = child_watchdog::output(&mut compiler, "fixture interface compiler").unwrap();
    assert!(result.status.success(), "fixture interface compiler failed");
    interfaces
}

pub(super) fn preload_key() -> &'static str {
    if cfg!(target_os = "macos") {
        "DYLD_INSERT_LIBRARIES"
    } else {
        "LD_PRELOAD"
    }
}

pub(super) fn use_fixture_interfaces(command: &mut Command, f: &Fixture) {
    command
        .env(preload_key(), f.root.join("interfaces-library"))
        .env("ARC_TEST_INTERFACE_MARKER", f.root.join("interface-calls"));
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile);
    }
}

pub(super) fn isolated_endpoint_stub(f: &Fixture, name: &str) -> Command {
    let home = f.root.join("isolated-home");
    fs::create_dir_all(&home).unwrap();
    let mut command = Command::new(cli::binary("arcctl"));
    command
        .env_clear()
        .env(arc_core::ARC_SOCKET_PATH, &f.socket)
        .env("HOME", &home)
        .env("XDG_CONFIG_HOME", home.join("xdg"))
        .env("GH_CONFIG_DIR", home.join("gh"))
        .env("GIT_CONFIG_GLOBAL", home.join("missing-git-config"))
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("DOCKER_CONFIG", home.join("missing-docker-config"))
        .env("BUILDX_CONFIG", home.join("missing-buildx-config"))
        .env("KUBECONFIG", home.join("missing-kubeconfig"))
        .args(["exec", "--", name]);
    if name == "ssh" {
        command.arg("-F").arg(home.join("missing-ssh-config"));
    }
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile);
    }
    command
}

impl Sandbox {
    fn new(f: &Fixture) -> Self {
        let home = f.root.join("home");
        let repo = f.root.join("repo");
        let bin = f.root.join("bin");
        for directory in [
            home.join("gh"),
            home.join("xdg"),
            repo.join(".git/objects/info"),
            repo.join(".git/objects/pack"),
            repo.join(".git/refs/heads"),
            repo.join(".git/refs/tags"),
            bin.clone(),
        ] {
            fs::create_dir_all(directory).unwrap();
        }
        write(&repo.join(".git/HEAD"), "ref: refs/heads/topic\n");
        write(&repo.join(".git/config"), "");
        write(&home.join("global"), "");
        for name in ["git", "gh", "ssh", "docker", "kubectl"] {
            symlink(cli::binary("arc-exec-fixture"), bin.join(name)).unwrap();
        }
        let interfaces = fixture_interfaces(f);
        Self {
            home,
            repo,
            bin,
            marker: f.root.join("calls"),
            interfaces,
        }
    }

    fn command(&self, f: &Fixture, env: &[(&str, Option<&str>)]) -> Command {
        let mut command = Command::new(cli::binary("arcctl"));
        command
            .env_clear()
            .env(arc_core::ARC_SOCKET_PATH, &f.socket)
            .env("HOME", &self.home)
            .env("XDG_CONFIG_HOME", self.home.join("xdg"))
            .env("GH_CONFIG_DIR", self.home.join("gh"))
            .env("GIT_CONFIG_GLOBAL", self.home.join("global"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("PATH", &self.bin)
            .env("ARC_FIXTURE_MODE", "echo")
            .env("ARC_FIXTURE_MARKER", &self.marker)
            .env(
                "ARC_TEST_INTERFACE_MARKER",
                self.home.join("interface-calls"),
            )
            .env("PRIVATE_ENV", "offline-fixture-value")
            .current_dir(&self.repo);
        command.env(preload_key(), &self.interfaces);
        // Coverage runners supply an output destination, not endpoint config.
        if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
            command.env("LLVM_PROFILE_FILE", profile);
        }
        for (key, value) in env {
            if let Some(value) = value {
                command.env(key, value);
            } else {
                command.env_remove(key);
            }
        }
        command
    }

    fn calls(&self) -> usize {
        fs::read_to_string(&self.marker)
            .map(|value| value.lines().count())
            .unwrap_or(0)
    }

    fn classify(&self, f: &Fixture, argv: &[&str], env: &[(&str, Option<&str>)]) -> Value {
        let shell = argv
            .iter()
            .map(|word| {
                assert!(!word.contains('\''));
                format!("'{word}'")
            })
            .collect::<Vec<_>>()
            .join(" ");
        let before = self.calls();
        let mut command = self.command(f, env);
        command.args(["classify", "--json", "--", &shell]);
        let output = child_watchdog::output(&mut command, "combined classify").unwrap();
        assert!(output.status.success(), "{argv:?}: {output:?}");
        assert!(output.stderr.is_empty(), "{argv:?}: {output:?}");
        assert_eq!(self.calls(), before, "classify executed an offline stub");
        serde_json::from_slice(&output.stdout).unwrap()
    }

    fn exec(&self, f: &Fixture, argv: &[&str], env: &[(&str, Option<&str>)]) -> (Value, JobId) {
        let before = self.calls();
        let mut command = self.command(f, env);
        command
            .args(["exec", "--"])
            .args(argv)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = child_watchdog::Owned::spawn(&mut command).unwrap();
        child
            .child
            .stdin
            .take()
            .unwrap()
            .write_all(b"input\n")
            .unwrap();
        let status = child.wait("combined exec").unwrap();
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        child
            .child
            .stdout
            .take()
            .unwrap()
            .read_to_end(&mut stdout)
            .unwrap();
        child
            .child
            .stderr
            .take()
            .unwrap()
            .read_to_end(&mut stderr)
            .unwrap();
        assert_eq!(status.code(), Some(17), "{argv:?}: {stderr:?}");
        assert_eq!(stderr, b"fixture-stderr\n");
        let child_output: Value =
            serde_json::from_slice(stdout.strip_prefix(b"input\n").unwrap()).unwrap();
        let arguments: Vec<Vec<u8>> = argv[1..]
            .iter()
            .map(|arg| arg.as_bytes().to_vec())
            .collect();
        assert_eq!(child_output["args"], json!(arguments));
        assert_eq!(
            child_output["cwd"],
            json!(fs::canonicalize(&self.repo).unwrap().as_os_str().as_bytes())
        );
        assert_eq!(child_output["environment"], json!(b"offline-fixture-value"));
        assert_eq!(self.calls(), before + 1, "{argv:?}");
        let id = cli::recent(f);
        let response = protocol::call(
            f,
            OPERATION_JOB_STATUS_LOCALITY,
            json!(StatusRequest::ById { job_id: id.clone() }),
        );
        assert_eq!(response.status, "ok", "{response:?}");
        let job = response.result.unwrap()["job"].clone();
        assert_eq!(job["outcome"]["code"], 17);
        assert!(job["child"]["pid"].is_u64());
        (job, id)
    }
}

fn assert_route_outcomes(name: &str, inspection: &Value, execution: &Value, expected: &str) {
    let run = &execution["classification"];
    for (route, actual) in [("classify", inspection), ("exec", run)] {
        assert_eq!(
            actual["classification"], expected,
            "{name}/{route}: {actual}"
        );
        let (locality, uncertainty) = if expected == "UNKNOWN" {
            ("UNKNOWN", "OBSERVE")
        } else {
            ("REMOTE", "NONE")
        };
        assert_eq!(actual["locality"], locality, "{name}/{route}: {actual}");
        assert_eq!(
            actual["uncertainty"], uncertainty,
            "{name}/{route}: {actual}"
        );
    }
}

fn assert_routes(name: &str, inspection: &Value, execution: &Value, expected: &str) -> bool {
    assert_route_outcomes(name, inspection, execution, expected);
    let run = &execution["classification"];
    let differs = run["reason"] != inspection["reason"];
    if differs {
        eprintln!("{name}: classify {inspection} versus exec {run}");
    }
    differs
}

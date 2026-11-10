//! Native macOS Cargo concurrency through the real daemon and CLI.
#![cfg(target_os = "macos")]

#[allow(dead_code)]
#[path = "support/runtime.rs"]
mod runtime;
mod support;

use runtime::{Fixture, OwnedChild};
use std::{
    fs::{self, File},
    ops::{Deref, DerefMut},
    os::unix::fs::symlink,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

fn binary(name: &str) -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_arc-daemon")).with_file_name(name)
}

fn eventually(mut check: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(45);
    while !check() {
        assert!(Instant::now() < deadline, "Traffic fixture timed out");
        thread::sleep(Duration::from_millis(10));
    }
}

fn projects(root: &Path, scenario: &str) -> [PathBuf; 2] {
    std::array::from_fn(|index| {
        let path = root.join(format!("{scenario}-{index}"));
        fs::create_dir_all(path.join("src")).unwrap();
        fs::write(path.join("Cargo.toml"), format!("[package]\nname = \"traffic_fixture_{scenario}_{index}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n")).unwrap();
        fs::write(path.join("src/lib.rs"), "pub fn fixture() {}\n").unwrap();
        fs::write(path.join("build.rs"), r#"
use std::{fs::{self, OpenOptions}, io::Write, thread, time::Duration};
fn main() {
    let marker = std::env::var_os("TRAFFIC_TEST_MARKER").unwrap();
    let release = std::env::var_os("TRAFFIC_TEST_RELEASE").unwrap();
    writeln!(OpenOptions::new().create(true).append(true).open(marker).unwrap(), "started").unwrap();
    while !std::path::Path::new(&release).exists() { thread::sleep(Duration::from_millis(10)); }
    let _ = fs::metadata(release).unwrap();
}
"#).unwrap();
        path
    })
}

struct BuildChild {
    owned: OwnedChild,
    owner: Option<arc_core::process_identity::ProcessIdentity>,
    database: PathBuf,
    root: PathBuf,
    managed: bool,
}
impl BuildChild {
    fn new(owned: OwnedChild, f: &Fixture, managed: bool) -> Self {
        let pid = owned.child.id();
        let owner = Some(
            process_cleanup::capture(pid)
                .unwrap_or_else(|error| panic!("wrapper identity observation failed: {error}"))
                .unwrap_or_else(|| panic!("wrapper pid {pid} disappeared before identity capture")),
        );
        Self {
            owned,
            owner,
            database: f.database.clone(),
            root: f.root.clone(),
            managed,
        }
    }
}
impl Deref for BuildChild {
    type Target = OwnedChild;
    fn deref(&self) -> &OwnedChild {
        &self.owned
    }
}
impl DerefMut for BuildChild {
    fn deref_mut(&mut self) -> &mut OwnedChild {
        &mut self.owned
    }
}
impl Drop for BuildChild {
    fn drop(&mut self) {
        use arc_core::process_identity::{probe_live, IdentityStatus, ProcessIdentity};
        let Some(owner) = self.owner.as_ref() else {
            return;
        };
        if probe_live(owner) == IdentityStatus::VerifiedMatch {
            process_cleanup::report_errors(process_cleanup::kill_tree(owner, 0));
            return;
        }
        if probe_live(owner) == IdentityStatus::Indeterminate {
            match process_cleanup::capture(owner.pid) {
                Ok(None) => {}
                Ok(Some(_)) => process_cleanup::report_errors(vec![format!(
                    "cannot establish live status for wrapper pid {}",
                    owner.pid
                )]),
                Err(error) => process_cleanup::report_errors(vec![error.to_string()]),
            }
        }
        if !self.managed {
            return;
        }
        let Ok(db) = rusqlite::Connection::open(&self.database) else {
            return;
        };
        let row = db.query_row("SELECT job_id,child_pid,child_start_token,child_boot_id FROM job_lifecycle WHERE observer_pid=?1 AND observer_start_token=?2 AND observer_boot_id=?3", (&owner.pid, &owner.start_token, &owner.boot_id), |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<u32>>(1)?, r.get::<_, Option<String>>(2)?, r.get::<_, Option<String>>(3)?)));
        let Ok((id, pid, start, boot)) = row else {
            return;
        };
        let child = match (pid, start, boot) {
            (Some(pid), Some(start_token), Some(boot_id)) => Some(ProcessIdentity {
                pid,
                start_token,
                boot_id,
            }),
            _ => fs::read(self.root.join(format!(".traffic-witness-{id}")))
                .ok()
                .and_then(|bytes| {
                    if bytes.len() != 136 || &bytes[..8] != b"ARCWIT03" {
                        return None;
                    }
                    let pid =
                        u32::try_from(u64::from_be_bytes(bytes[24..32].try_into().ok()?)).ok()?;
                    let start = u64::from_be_bytes(bytes[32..40].try_into().ok()?);
                    (pid > 0 && start > 0).then(|| ProcessIdentity {
                        pid,
                        start_token: start.to_string(),
                        boot_id: owner.boot_id.clone(),
                    })
                }),
        };
        if let Some(child) = child {
            process_cleanup::report_errors(process_cleanup::kill_tree(&child, 0));
        }
    }
}

fn spawn_build(f: &Fixture, project: &Path, managed: bool) -> BuildChild {
    let mut command = if managed {
        let mut command = Command::new(binary("arcctl"));
        command.args(["exec", "--", "cargo"]);
        command.env(arc_core::ARC_SOCKET_PATH, &f.socket);
        command
    } else {
        Command::new("cargo")
    };
    command
        .args(["build", "--offline"])
        .current_dir(project)
        .env("CARGO_TARGET_DIR", project.join("target"))
        .env("TRAFFIC_TEST_MARKER", project.join("started"))
        .env("TRAFFIC_TEST_RELEASE", project.join("release"))
        .env_remove("ARC_TRAFFIC_LINEAGE")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(File::create(project.join("stderr")).unwrap()));
    BuildChild::new(OwnedChild::new(command.spawn().unwrap()), f, managed)
}

// The shim records each attempted Cargo exec independently of build.rs, then
// replaces itself with the real Cargo executable without creating another PID.
fn install_cargo_observer(f: &Fixture) {
    use std::os::unix::fs::PermissionsExt;
    let shim = f.root.join("cargo");
    fs::write(
        &shim,
        "#!/bin/sh\nprintf '%s\\n' \"$$\" >> \"$TRAFFIC_LAUNCH_LOG\" || exit 125\nexec \"$TRAFFIC_REAL_CARGO\" \"$@\"\n",
    )
    .unwrap();
    fs::set_permissions(shim, fs::Permissions::from_mode(0o700)).unwrap();
}

fn observe_cargo(command: &mut Command, f: &Fixture, project: &Path) {
    command
        .env(
            "PATH",
            format!("{}:{}", f.root.display(), std::env::var("PATH").unwrap()),
        )
        .env("TRAFFIC_REAL_CARGO", env!("CARGO"))
        .env("TRAFFIC_LAUNCH_LOG", project.join("cargo-launches"));
}

fn spawn_observed_build(f: &Fixture, project: &Path, managed: bool) -> BuildChild {
    let mut command = if managed {
        let mut command = Command::new(binary("arcctl"));
        command.args(["exec", "--", "cargo"]);
        command.env(arc_core::ARC_SOCKET_PATH, &f.socket);
        command
    } else {
        Command::new("cargo")
    };
    observe_cargo(&mut command, f, project);
    command
        .args(["build", "--offline"])
        .current_dir(project)
        .env("CARGO_TARGET_DIR", project.join("target"))
        .env("TRAFFIC_TEST_MARKER", project.join("started"))
        .env("TRAFFIC_TEST_RELEASE", project.join("release"))
        .env_remove("ARC_TRAFFIC_LINEAGE")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(File::create(project.join("stderr")).unwrap()));
    BuildChild::new(OwnedChild::new(command.spawn().unwrap()), f, managed)
}

fn cargo_launches(project: &Path) -> Vec<u32> {
    fs::read_to_string(project.join("cargo-launches"))
        .unwrap()
        .lines()
        .map(|line| line.parse().unwrap())
        .collect()
}

fn cargo_pid(pid: u32) -> bool {
    let mut path = [0u8; 4096];
    // SAFETY: proc_pidpath writes at most the provided buffer length.
    let size =
        unsafe { libc::proc_pidpath(pid as i32, path.as_mut_ptr().cast(), path.len() as u32) };
    if size <= 0 {
        return false;
    }
    let value = String::from_utf8_lossy(&path[..size as usize]);
    Path::new(value.trim_end_matches('\0'))
        .file_name()
        .is_some_and(|name| name == "cargo")
}

fn children(parent: u32) -> Vec<u32> {
    process_cleanup::children(parent).unwrap_or_else(|error| panic!("{error}"))
}

fn database(f: &Fixture) -> rusqlite::Connection {
    rusqlite::Connection::open(&f.database).unwrap()
}

type Entry = (String, Option<u32>, Option<i64>, Option<i64>, Option<i64>);

fn entry(f: &Fixture, owner: u32) -> Option<Entry> {
    database(f).query_row("SELECT t.state,l.child_pid,t.queued_at_ms,t.granted_at_ms,p.released_at FROM traffic_entries t JOIN job_lifecycle l ON l.job_id=t.job_id LEFT JOIN permits p ON p.job_id=t.job_id WHERE l.observer_pid=?1", [owner], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).ok()
}

fn active(f: &Fixture) -> (i64, i64, i64, i64) {
    database(f).query_row("SELECT (SELECT count(*) FROM traffic_entries), count(*), coalesce(sum(cpu_millis_est),0), coalesce(sum(memory_bytes_est),0) FROM permits WHERE state='ACTIVE'", [], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?))).unwrap()
}

fn finish(child: &mut OwnedChild) {
    eventually(|| child.child.try_wait().unwrap().is_some());
    assert!(child.child.wait().unwrap().success());
}

fn invocations(path: &Path) -> usize {
    fs::read_to_string(path.join("started"))
        .unwrap()
        .lines()
        .count()
}

fn traffic(f: &Fixture, input: &arc_ipc::traffic::JobRequest) -> arc_ipc::Response {
    let mut request =
        arc_ipc::Request::new("traffic-test", arc_ipc::traffic::OPERATION_TRAFFIC_JOB);
    request.payload = serde_json::to_value(input).unwrap();
    arc_ipc::exchange(&mut f.connect(), &request).unwrap()
}

fn traffic_result(response: arc_ipc::Response) -> arc_ipc::traffic::JobResult {
    assert_eq!(response.status, arc_ipc::STATUS_OK, "{response:?}");
    serde_json::from_value(response.result.unwrap()).unwrap()
}

fn cargo_class(class: &str, reason: &str) -> arc_ipc::Classification {
    serde_json::from_value(serde_json::json!({"classification":class,"locality":"HOST_LOCAL","reason":reason,"uncertainty":"NONE"})).unwrap()
}

#[path = "traffic_cli/cleanup.rs"]
mod cleanup;
#[path = "traffic_cli/diagnostics.rs"]
mod diagnostics;
#[path = "traffic_cli/measurement.rs"]
mod measurement;
#[path = "traffic_cli/process_cleanup.rs"]
mod process_cleanup;
#[path = "traffic_cli/protocol.rs"]
mod protocol;
#[path = "traffic_cli/recovery.rs"]
mod recovery;
#[path = "traffic_cli/robustness.rs"]
mod robustness;
#[path = "traffic_cli/semantics.rs"]
mod semantics;
#[path = "traffic_cli/status.rs"]
mod status;
#[path = "traffic_cli/status_compat.rs"]
mod status_compat;
#[path = "traffic_cli/unattached.rs"]
mod unattached;

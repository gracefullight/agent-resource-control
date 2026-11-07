//! End-to-end resident contract: socket modes, length-prefixed IPC, arcctl,
//! single instance, SIGTERM shutdown, and a clean reopen.

mod support;

#[path = "resident/coordination.rs"]
mod coordination;
#[path = "resident/identity.rs"]
mod identity;
#[path = "resident/lifecycle.rs"]
mod lifecycle;
#[path = "resident/provisioning.rs"]
mod provisioning;
#[path = "resident/schema.rs"]
mod schema;

use arc_ipc::{
    self, Request, Response, ERROR_PROTOCOL_INCOMPATIBLE, OPERATION_HELLO, PROTOCOL_VERSION,
    STATUS_ERROR, STATUS_OK,
};
use std::{
    env, fs,
    io::{Read, Write},
    ops::{Deref, DerefMut},
    os::unix::{
        fs::{symlink, DirBuilderExt, FileTypeExt, MetadataExt, PermissionsExt},
        net::UnixStream,
        process::CommandExt,
    },
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Output, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

const READY_TIMEOUT: Duration = Duration::from_secs(15);
const EXIT_TIMEOUT: Duration = Duration::from_secs(10);

struct Fixture {
    directory: PathBuf,
    database: PathBuf,
    socket: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let directory = arc_core::runtime_root().unwrap().join(format!(
            "r{:x}-{:x}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .unwrap();
        Self {
            database: directory.join("arc.sqlite3"),
            socket: directory.join("arc.sock"),
            directory,
        }
    }

    fn spawn(&self) -> Daemon {
        self.spawn_configured(false, None)
    }

    fn spawn_configured(&self, default_socket: bool, mask: Option<libc::mode_t>) -> Daemon {
        let mut command = self.command(default_socket);
        if let Some(mask) = mask {
            // SAFETY: only this child changes its umask; umask is async-signal-safe.
            unsafe {
                command.pre_exec(move || {
                    libc::umask(mask);
                    Ok(())
                });
            }
        }
        Daemon(command.spawn().unwrap())
    }

    fn command(&self, default_socket: bool) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_arc-daemon"));
        command
            .env(arc_core::ARC_DB_PATH, &self.database)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if default_socket {
            command.env_remove(arc_core::ARC_SOCKET_PATH);
        } else {
            command.env(arc_core::ARC_SOCKET_PATH, &self.socket);
        }
        command
    }

    fn spawn_gated(&self) -> Daemon {
        // Both children exist before either is released into daemon startup.
        // exec preserves the owned child PID, so cleanup never targets a shell's orphan.
        let child = Command::new("/bin/sh")
            .args([
                "-c",
                "read -r gate; exec \"$@\"",
                "arc-fixture",
                env!("CARGO_BIN_EXE_arc-daemon"),
            ])
            .env(arc_core::ARC_DB_PATH, &self.database)
            .env(arc_core::ARC_SOCKET_PATH, &self.socket)
            .env("TMPDIR", self.directory.join("different-tmpdir"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        Daemon(child)
    }
}

// Always reap a daemon on assertion failure as well as on normal SIGTERM exit.
struct Daemon(Child);

impl Deref for Daemon {
    type Target = Child;

    fn deref(&self) -> &Child {
        &self.0
    }
}

impl DerefMut for Daemon {
    fn deref_mut(&mut self) -> &mut Child {
        &mut self.0
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

fn wait_until_ready(child: &mut Child, socket: &Path) {
    let deadline = Instant::now() + READY_TIMEOUT;
    loop {
        if let Ok(mut stream) = UnixStream::connect(socket) {
            stream
                .set_read_timeout(Some(Duration::from_millis(100)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_millis(100)))
                .unwrap();
            if let Ok(response) = arc_ipc::exchange(
                &mut stream,
                &Request::new("resident-ready", OPERATION_HELLO),
            ) {
                if response.status == STATUS_OK && response.protocol_version == PROTOCOL_VERSION {
                    return;
                }
            }
        }
        if let Some(status) = child.try_wait().unwrap() {
            let mut stderr = String::new();
            if let Some(pipe) = child.stderr.as_mut() {
                let _ = pipe.read_to_string(&mut stderr);
            }
            let mut stdout = String::new();
            if let Some(pipe) = child.stdout.as_mut() {
                let _ = pipe.read_to_string(&mut stdout);
            }
            panic!(
                "daemon exited before serving {socket:?}: {status:?}\nstdout: {stdout}\nstderr: {stderr}"
            );
        }
        assert!(Instant::now() < deadline, "daemon did not become ready");
        thread::sleep(Duration::from_millis(20));
    }
}

fn wait_for_exit(child: &mut Child) -> ExitStatus {
    let deadline = Instant::now() + EXIT_TIMEOUT;
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            panic!("daemon did not exit within {EXIT_TIMEOUT:?}");
        }
        thread::sleep(Duration::from_millis(20));
    }
}

fn exchange(socket: &Path, request: &Request) -> Response {
    let mut stream = UnixStream::connect(socket).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    arc_ipc::exchange(&mut stream, request).unwrap()
}

fn assert_hello(socket: &Path) {
    let response = exchange(socket, &Request::new("resident-hello", OPERATION_HELLO));
    assert_eq!(response.status, STATUS_OK);
    assert_eq!(response.protocol_version, PROTOCOL_VERSION);
    let result = response.result.expect("hello result");
    assert_eq!(result["protocol_version"], PROTOCOL_VERSION);
    assert_eq!(result["sweeper"], "off");
}

fn mode(path: &Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

fn binary(name: &str) -> PathBuf {
    Path::new(env!("CARGO_BIN_EXE_arc-daemon"))
        .with_file_name(format!("{name}{}", env::consts::EXE_SUFFIX))
}

fn wait_for_binary(name: &str) -> PathBuf {
    let path = binary(name);
    let deadline = Instant::now() + Duration::from_secs(30);
    while !path.is_file() {
        assert!(
            Instant::now() < deadline,
            "{} was not built next to the test binary",
            path.display()
        );
        thread::sleep(Duration::from_millis(20));
    }
    path
}

fn arcctl(fixture: &Fixture, command: &str) -> Output {
    Command::new(wait_for_binary("arcctl"))
        .arg(command)
        .env(arc_core::ARC_DB_PATH, &fixture.database)
        .env(arc_core::ARC_SOCKET_PATH, &fixture.socket)
        .output()
        .unwrap()
}

fn assert_no_tcp_listener(pid: u32) {
    let lsof = ["/usr/sbin/lsof", "/usr/bin/lsof"]
        .into_iter()
        .find(|path| Path::new(path).exists())
        .unwrap_or("lsof");
    let output = Command::new(lsof)
        .args(["-nP", "-p", &pid.to_string()])
        .output()
        .unwrap();
    let listing = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success() && output.stderr.is_empty() && !listing.trim().is_empty(),
        "could not inspect daemon {pid} file descriptors: {output:?}"
    );
    assert!(
        !listing
            .lines()
            .any(|line| line.contains("TCP") && line.contains("(LISTEN)")),
        "daemon {pid} opened a TCP listener:\n{listing}"
    );
}

// Capture every entry without following links. Equality proves inode/mode/
// ownership preservation, unchanged names and bytes, and no startup artifacts.
#[derive(Debug, PartialEq, Eq)]
struct EntrySnapshot {
    path: PathBuf,
    identity: (u64, u64, u32, u32, u32),
    bytes: Vec<u8>,
    link: Option<PathBuf>,
}

fn snapshot(root: &Path) -> Vec<EntrySnapshot> {
    fn visit(root: &Path, path: &Path, entries: &mut Vec<EntrySnapshot>) {
        let metadata = fs::symlink_metadata(path).unwrap();
        entries.push(EntrySnapshot {
            path: path.strip_prefix(root).unwrap().to_owned(),
            identity: (
                metadata.dev(),
                metadata.ino(),
                metadata.mode(),
                metadata.uid(),
                metadata.gid(),
            ),
            bytes: if metadata.is_file() {
                fs::read(path).unwrap()
            } else {
                Vec::new()
            },
            link: if metadata.file_type().is_symlink() {
                Some(fs::read_link(path).unwrap())
            } else {
                None
            },
        });
        if metadata.is_dir() {
            let mut children: Vec<_> = fs::read_dir(path)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .collect();
            children.sort();
            for child in children {
                visit(root, &child, entries);
            }
        }
    }
    let mut entries = Vec::new();
    visit(root, root, &mut entries);
    entries
}

fn cli_configured(fixture: &Fixture, operation: &str, default_socket: bool) -> Output {
    let mut command = Command::new(wait_for_binary("arcctl"));
    command
        .arg(operation)
        .env(arc_core::ARC_DB_PATH, &fixture.database);
    if default_socket {
        command.env_remove(arc_core::ARC_SOCKET_PATH);
    } else {
        command.env(arc_core::ARC_SOCKET_PATH, &fixture.socket);
    }
    command.output().unwrap()
}

fn assert_preserved_rejection(fixture: &Fixture, default_socket: bool, reason: &str) {
    let before = snapshot(&fixture.directory);
    let socket_existed = fs::symlink_metadata(&fixture.socket).is_ok();
    let mut daemon = fixture.spawn_configured(default_socket, None);
    assert!(!wait_for_exit(&mut daemon).success());
    let mut stdout = String::new();
    daemon
        .stdout
        .as_mut()
        .unwrap()
        .read_to_string(&mut stdout)
        .unwrap();
    assert!(stdout.is_empty(), "unexpected readiness: {stdout}");
    let mut stderr = String::new();
    daemon
        .stderr
        .as_mut()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    assert!(
        stderr.starts_with("Daemon startup failed: socket directory:"),
        "{stderr}"
    );
    assert!(stderr.contains(reason), "{stderr}");
    assert!(stderr.contains("ARC_SOCKET_PATH"), "{stderr}");
    assert!(
        stderr.contains("choose a separate owned 0700 directory"),
        "{stderr}"
    );
    assert!(
        stderr.contains(fixture.directory.to_str().unwrap()),
        "{stderr}"
    );
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert_eq!(
        before,
        snapshot(&fixture.directory),
        "startup mutated the rejected fixture"
    );
    for operation in ["status", "doctor"] {
        let output = cli_configured(fixture, operation, default_socket);
        assert!(!output.status.success(), "{output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("DAEMON_UNAVAILABLE"),
            "{output:?}"
        );
    }
    assert_eq!(
        before,
        snapshot(&fixture.directory),
        "CLI mutated the rejected fixture"
    );
    assert!(
        !fixture.database.exists(),
        "database initialized before validation"
    );
    assert!(
        fs::symlink_metadata(&fixture.socket).is_ok() == socket_existed,
        "socket entry changed before validation"
    );
}

fn stop(daemon: &mut Daemon) {
    assert_eq!(
        unsafe { libc::kill(daemon.id() as libc::pid_t, libc::SIGTERM) },
        0
    );
    assert!(wait_for_exit(daemon).success());
}

fn assert_private(path: &Path, permissions: u32) {
    let metadata = fs::symlink_metadata(path).unwrap();
    assert_eq!(metadata.uid(), unsafe { libc::geteuid() });
    assert_eq!(metadata.mode() & 0o7777, permissions, "{}", path.display());
}

#[test]
fn fixture_serialization_probe() {
    if let Some(root) = env::var_os("ARC_FIXTURE_COORDINATION_PROBE") {
        let root = PathBuf::from(root);
        fs::write(root.join("ready"), b"waiting").unwrap();
        let _serial = support::serial();
        fs::write(root.join("acquired"), b"held").unwrap();
    }
}

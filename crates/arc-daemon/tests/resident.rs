//! End-to-end resident contract: socket modes, length-prefixed IPC, arcctl,
//! single instance, SIGTERM shutdown, and a clean reopen.

mod support;

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

#[test]
fn resident_contract_holds_until_sigterm() {
    let _serial = support::serial();
    let fixture = Fixture::new();
    let mut daemon = fixture.spawn();
    wait_until_ready(&mut daemon, &fixture.socket);

    assert!(fs::symlink_metadata(&fixture.socket)
        .unwrap()
        .file_type()
        .is_socket());
    assert_eq!(mode(&fixture.socket), 0o600, "socket mode must be 0600");
    assert_eq!(
        mode(&fixture.directory),
        0o700,
        "socket directory mode must be 0700"
    );
    assert_no_tcp_listener(daemon.id());

    assert_hello(&fixture.socket);

    let mut incompatible = Request::new("resident-v99", OPERATION_HELLO);
    incompatible.protocol_version = 99;
    let mut stream = UnixStream::connect(&fixture.socket).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let response = arc_ipc::exchange(&mut stream, &incompatible).unwrap();
    assert_eq!(response.status, STATUS_ERROR);
    assert_eq!(
        response.error.as_ref().map(|error| error.code.as_str()),
        Some(ERROR_PROTOCOL_INCOMPATIBLE)
    );
    let response = arc_ipc::exchange(
        &mut stream,
        &Request::new("resident-after-v99", OPERATION_HELLO),
    )
    .unwrap();
    assert_eq!(response.status, STATUS_OK);
    assert_eq!(response.protocol_version, PROTOCOL_VERSION);
    assert_hello(&fixture.socket);

    // An oversized declaration must be rejected before waiting for any body.
    let mut oversized = UnixStream::connect(&fixture.socket).unwrap();
    oversized
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    std::io::Write::write_all(
        &mut oversized,
        &(arc_ipc::MAX_FRAME_BYTES + 1).to_be_bytes(),
    )
    .unwrap();
    let response = arc_ipc::read_response(&mut oversized).unwrap();
    assert_eq!(response.status, STATUS_ERROR);
    assert_eq!(response.error.unwrap().code, arc_ipc::ERROR_FRAME_TOO_LARGE);
    assert_hello(&fixture.socket);

    let status = arcctl(&fixture, "status");
    assert!(status.status.success(), "{status:?}");
    let stdout = String::from_utf8(status.stdout).unwrap();
    assert_eq!(
        stdout.trim_end(),
        format!(
            "running. Socket: {}. Protocol: 1. Sweeper off.",
            fixture.socket.display()
        )
    );
    assert!(!stdout.contains("eleven"), "{stdout}");

    let doctor = arcctl(&fixture, "doctor");
    assert!(doctor.status.success(), "{doctor:?}");
    let stdout = String::from_utf8(doctor.stdout).unwrap();
    assert_eq!(
        stdout.trim_end(),
        "ok. IPC owner-only. Tray not connected. Vendors not detected. \
         Command classification not checked. Sweeper off."
    );
    assert!(!stdout.contains("eleven"), "{stdout}");

    let mut second = fixture.spawn();
    let second_status = wait_for_exit(&mut second);
    assert!(
        !second_status.success(),
        "a second daemon must exit non-zero: {second_status:?}"
    );
    let mut second_stderr = String::new();
    if let Some(pipe) = second.stderr.as_mut() {
        let _ = pipe.read_to_string(&mut second_stderr);
    }
    assert!(
        second_stderr.contains("socket already served"),
        "second daemon stderr: {second_stderr}"
    );
    assert_hello(&fixture.socket);

    let result = unsafe { libc::kill(daemon.id() as libc::pid_t, libc::SIGTERM) };
    assert_eq!(result, 0, "SIGTERM failed");
    let exit = wait_for_exit(&mut daemon);
    assert!(exit.success(), "daemon exit status after SIGTERM: {exit:?}");
    assert!(
        !fixture.socket.exists(),
        "socket {} must be removed on shutdown",
        fixture.socket.display()
    );

    let reopened = rusqlite::Connection::open(&fixture.database).unwrap();
    let integrity: String = reopened
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");
    assert_eq!(
        reopened
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        reopened
            .query_row("SELECT COUNT(*) FROM gc_operations", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    reopened.close().unwrap();
}

#[test]
fn commands_report_daemon_unavailable_when_absent() {
    let _serial = support::serial();
    let fixture = Fixture::new();
    for command in ["status", "doctor"] {
        let output = arcctl(&fixture, command);
        assert!(!output.status.success(), "{command} must fail: {output:?}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            stderr.contains("DAEMON_UNAVAILABLE"),
            "{command} stderr: {stderr}"
        );
    }
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

#[test]
fn permissive_directories_and_their_contents_survive_daemon_and_cli_rejection() {
    let _serial = support::serial();
    for permissions in [0o755, 0o770] {
        let mut fixture = Fixture::new();
        let directory = fixture.directory.join("arc");
        fs::create_dir(&directory).unwrap();
        fs::set_permissions(&directory, fs::Permissions::from_mode(permissions)).unwrap();
        fs::write(directory.join("sentinel"), b"operator-owned bytes\0\xff").unwrap();
        fixture.socket = directory.join("arc.sock");
        assert_preserved_rejection(&fixture, false, &format!("permissions {permissions:04o}"));
    }
}

#[test]
fn symlink_targets_and_dangling_links_survive_daemon_and_cli_rejection() {
    let _serial = support::serial();
    for permissions in [0o700, 0o755, 0o770] {
        for intermediate in [false, true] {
            let mut fixture = Fixture::new();
            let target = fixture.directory.join("target");
            fs::create_dir(&target).unwrap();
            fs::set_permissions(&target, fs::Permissions::from_mode(permissions)).unwrap();
            fs::write(target.join("sentinel"), b"symlink target stays intact").unwrap();
            let link = fixture.directory.join("arc");
            symlink(&target, &link).unwrap();
            fixture.socket = if intermediate {
                link.join("missing/arc.sock")
            } else {
                link.join("arc.sock")
            };
            assert_preserved_rejection(&fixture, false, "symlink");
            if !intermediate {
                fixture.socket = fixture.directory.join("new/../arc/arc.sock");
                assert_preserved_rejection(&fixture, false, "symlink");
                assert!(!fixture.directory.join("new").exists());
            }
        }
    }
    let mut fixture = Fixture::new();
    let target = fixture.directory.join("absent");
    let link = fixture.directory.join("arc");
    symlink(&target, &link).unwrap();
    fixture.socket = link.join("arc.sock");
    assert_preserved_rejection(&fixture, false, "symlink");
    assert!(!target.exists(), "dangling target was created");
}

#[test]
fn nondirectory_parent_is_rejected_before_lock_or_database_initialization() {
    let _serial = support::serial();
    let mut fixture = Fixture::new();
    let file = fixture.directory.join("file");
    fs::write(&file, b"not a directory").unwrap();
    fixture.socket = file.join("arc.sock");
    assert_preserved_rejection(&fixture, false, "must be a directory");
}

#[test]
fn private_existing_and_new_nested_directories_support_override_startup() {
    let _serial = support::serial();
    for nested in [false, true] {
        for existing in [false, true] {
            for mask in [0o000, 0o022, 0o077] {
                let mut fixture = Fixture::new();
                let first = fixture.directory.join("arc");
                let parent = if nested {
                    first.join("nested")
                } else {
                    first.clone()
                };
                let sentinel = parent.join("sentinel");
                let before = if existing {
                    fs::DirBuilder::new()
                        .recursive(true)
                        .mode(0o700)
                        .create(&parent)
                        .unwrap();
                    fs::write(&sentinel, b"existing private content").unwrap();
                    let metadata = fs::metadata(&parent).unwrap();
                    Some((
                        metadata.dev(),
                        metadata.ino(),
                        metadata.uid(),
                        metadata.gid(),
                    ))
                } else {
                    None
                };
                fixture.socket = parent.join("arc.sock");
                let mut daemon = fixture.spawn_configured(false, Some(mask));
                wait_until_ready(&mut daemon, &fixture.socket);
                assert_eq!(mode(&first), 0o700);
                assert_eq!(mode(&parent), 0o700);
                assert_eq!(mode(&fixture.socket), 0o600);
                if let Some(before) = before {
                    let metadata = fs::metadata(&parent).unwrap();
                    assert_eq!(
                        before,
                        (
                            metadata.dev(),
                            metadata.ino(),
                            metadata.uid(),
                            metadata.gid()
                        )
                    );
                    assert_eq!(fs::read(&sentinel).unwrap(), b"existing private content");
                }
                for operation in ["status", "doctor"] {
                    let output = cli_configured(&fixture, operation, false);
                    assert!(output.status.success(), "{output:?}");
                }
                let mut duplicate = fixture.spawn_configured(false, Some(mask));
                assert!(!wait_for_exit(&mut duplicate).success());
                let mut stderr = String::new();
                duplicate
                    .stderr
                    .as_mut()
                    .unwrap()
                    .read_to_string(&mut stderr)
                    .unwrap();
                assert!(stderr.contains("socket already served"), "{stderr}");
                assert_hello(&fixture.socket);
                // SAFETY: this child is owned by the fixture and still running.
                assert_eq!(
                    unsafe { libc::kill(daemon.id() as libc::pid_t, libc::SIGTERM) },
                    0
                );
                assert!(wait_for_exit(&mut daemon).success());
                assert!(!fixture.socket.exists());
                assert_eq!(mode(&parent), 0o700);
            }
        }
    }
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

#[cfg(target_os = "macos")]
#[test]
fn native_default_and_cli_ignore_unset_changed_and_unwritable_tmpdir() {
    let _serial = support::serial();
    let mut fixture = Fixture::new();
    fixture.socket = arc_core::runtime_root().unwrap().join("arc/arc.sock");
    let unwritable = fixture.directory.join("no-write");
    fs::DirBuilder::new()
        .mode(0o500)
        .create(&unwritable)
        .unwrap();
    assert!(
        fs::write(unwritable.join("probe"), b"cannot write").is_err(),
        "TMPDIR must be demonstrably unwritable"
    );
    let changed = fixture.directory.join("changed");
    fs::DirBuilder::new().mode(0o700).create(&changed).unwrap();
    for daemon_tmpdir in [None, Some(&changed), Some(&unwritable)] {
        let mut command = fixture.command(true);
        command.env_remove("TMPDIR");
        if let Some(path) = daemon_tmpdir {
            command.env("TMPDIR", path);
        }
        let mut daemon = Daemon(command.spawn().unwrap());
        wait_until_ready(&mut daemon, &fixture.socket);
        assert_private(fixture.socket.parent().unwrap(), 0o700);
        assert_private(&fixture.socket, 0o600);
        let anchor = arc_core::instance_anchor().unwrap();
        assert_private(anchor.parent().unwrap(), 0o700);
        assert_private(&anchor, 0o600);
        for client_tmpdir in [None, Some(&changed), Some(&unwritable)] {
            for operation in ["status", "doctor"] {
                let mut client = Command::new(wait_for_binary("arcctl"));
                client
                    .arg(operation)
                    .env_remove(arc_core::ARC_SOCKET_PATH)
                    .env_remove("TMPDIR");
                if let Some(path) = client_tmpdir {
                    client.env("TMPDIR", path);
                }
                let output = client.output().unwrap();
                assert!(output.status.success(), "{output:?}");
                if operation == "status" {
                    assert!(String::from_utf8_lossy(&output.stdout)
                        .contains(fixture.socket.to_str().unwrap()));
                }
            }
        }
        stop(&mut daemon);
        assert!(!fixture.socket.exists());
    }
    assert!(!changed.join("arc").exists());
    assert!(!unwritable.join("arc").exists());
    fs::set_permissions(unwritable, fs::Permissions::from_mode(0o700)).unwrap();
}

#[test]
fn concurrent_distinct_overrides_admit_one_and_recover_after_shutdown_and_death() {
    let _serial = support::serial();
    let mut fixtures = [Fixture::new(), Fixture::new()];
    for (index, fixture) in fixtures.iter_mut().enumerate() {
        fixture.socket = fixture.directory.join(format!("ipc/{index}.sock"));
        fixture.database = fixture.directory.join("db/arc.sqlite3");
        fs::write(
            fixture.directory.join("sentinel"),
            b"preserve contender content",
        )
        .unwrap();
    }
    let before = [
        snapshot(&fixtures[0].directory),
        snapshot(&fixtures[1].directory),
    ];
    let mut children = [fixtures[0].spawn_gated(), fixtures[1].spawn_gated()];
    for child in &mut children {
        child.stdin.take().unwrap().write_all(b"go\n").unwrap();
    }
    let deadline = Instant::now() + READY_TIMEOUT;
    let loser = loop {
        let exits = [
            children[0].try_wait().unwrap(),
            children[1].try_wait().unwrap(),
        ];
        assert!(
            !(exits[0].is_some() && exits[1].is_some()),
            "both contenders exited: {exits:?}"
        );
        if let Some(index) = exits.iter().position(Option::is_some) {
            break index;
        }
        assert!(Instant::now() < deadline, "neither contender was rejected");
        thread::sleep(Duration::from_millis(10));
    };
    let winner = 1 - loser;
    assert!(!wait_for_exit(&mut children[loser]).success());
    let mut stdout = String::new();
    let mut stderr = String::new();
    children[loser]
        .stdout
        .as_mut()
        .unwrap()
        .read_to_string(&mut stdout)
        .unwrap();
    children[loser]
        .stderr
        .as_mut()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    assert!(stdout.is_empty(), "loser announced readiness: {stdout}");
    assert!(stderr.contains("only one daemon per user"), "{stderr}");
    assert_eq!(before[loser], snapshot(&fixtures[loser].directory));
    assert!(!fixtures[loser].database.parent().unwrap().exists());
    assert!(!fixtures[loser].socket.parent().unwrap().exists());
    for suffix in ["", "-wal", "-shm"] {
        assert!(!PathBuf::from(format!("{}{suffix}", fixtures[loser].database.display())).exists());
    }
    let fixture = &fixtures[winner];
    wait_until_ready(&mut children[winner], &fixture.socket);
    for operation in ["status", "doctor"] {
        assert!(arcctl(fixture, operation).status.success());
    }
    assert_private(fixture.socket.parent().unwrap(), 0o700);
    assert_private(&fixture.socket, 0o600);
    assert_private(&arc_core::instance_anchor().unwrap(), 0o600);
    stop(&mut children[winner]);
    assert!(!fixture.socket.exists());

    let mut graceful_restart = fixture.spawn();
    wait_until_ready(&mut graceful_restart, &fixture.socket);
    for operation in ["status", "doctor"] {
        assert!(arcctl(fixture, operation).status.success());
    }
    graceful_restart.kill().unwrap();
    assert!(!wait_for_exit(&mut graceful_restart).success());
    assert!(
        fixture.socket.exists(),
        "forced death must leave the stale socket for recovery"
    );
    let mut recovered = fixture.spawn();
    wait_until_ready(&mut recovered, &fixture.socket);
    for operation in ["status", "doctor"] {
        assert!(arcctl(fixture, operation).status.success());
    }
    stop(&mut recovered);
    assert!(!fixture.socket.exists());
}

#[test]
fn overlong_socket_paths_and_existing_socket_entries_reject_without_mutation() {
    let _serial = support::serial();
    let mut fixture = Fixture::new();
    fixture.database = fixture.directory.join("db/arc.sqlite3");
    fs::write(fixture.directory.join("sentinel"), b"protected").unwrap();
    let anchor = arc_core::instance_anchor().unwrap();
    let anchor_before = fs::read(&anchor).ok();
    fixture.socket = fixture.directory.join("new").join("s".repeat(104));
    assert_preserved_rejection(&fixture, false, "maximum");
    assert_eq!(
        anchor_before,
        fs::read(&anchor).ok(),
        "overlong rejection wrote admission contents"
    );
    fixture.socket = fixture.directory.join("s".repeat(104));
    fs::write(&fixture.socket, b"overlong existing entry stays intact").unwrap();
    assert_preserved_rejection(&fixture, false, "maximum");
    assert_eq!(anchor_before, fs::read(&anchor).ok());
    let target = fixture.directory.join("target");
    fs::write(&target, b"socket target").unwrap();
    fixture.socket = fixture.directory.join("entry");
    symlink(&target, &fixture.socket).unwrap();
    assert_preserved_rejection(&fixture, false, "symlink");
    fs::remove_file(&fixture.socket).unwrap();
    fs::write(&fixture.socket, b"existing regular file").unwrap();
    assert_preserved_rejection(&fixture, false, "Unix socket");
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

#[test]
fn fixture_coordination_is_shared_across_processes() {
    let serial = support::serial();
    let fixture = Fixture::new();
    let mut child = Daemon(
        Command::new(env::current_exe().unwrap())
            .args(["--exact", "fixture_serialization_probe"])
            .env("ARC_FIXTURE_COORDINATION_PROBE", &fixture.directory)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + READY_TIMEOUT;
    while !fixture.directory.join("ready").exists() {
        assert!(
            child.try_wait().unwrap().is_none(),
            "coordination child exited before waiting"
        );
        assert!(
            Instant::now() < deadline,
            "coordination child did not start"
        );
        thread::sleep(Duration::from_millis(10));
    }
    assert!(!fixture.directory.join("acquired").exists());
    drop(serial);
    assert!(wait_for_exit(&mut child).success());
    assert!(fixture.directory.join("acquired").exists());
}

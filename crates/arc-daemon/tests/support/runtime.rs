//! Owned native children. The fixture lock must outlive every child.
use std::{
    fs,
    io::{Read, Write},
    os::unix::{fs::DirBuilderExt, net::UnixStream},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

pub struct Fixture {
    pub root: PathBuf,
    pub socket: PathBuf,
    pub database: PathBuf,
}

impl Fixture {
    pub fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = arc_core::runtime_root().unwrap().join(format!(
            "b{:x}-{:x}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        Self {
            socket: root.join("arc.sock"),
            database: root.join("arc.sqlite3"),
            root,
        }
    }

    pub fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_arc-daemon"));
        self.configure(&mut command);
        command
    }

    pub fn configure(&self, command: &mut Command) {
        command
            .env(arc_core::ARC_DB_PATH, &self.database)
            .env(arc_core::ARC_SOCKET_PATH, &self.socket)
            .env("ARC_RUNTIME_REPORT", self.root.join("runtime.json"))
            .env_remove("ARC_IPC_PROFILE")
            .stdout(fs::File::create(self.root.join("stdout")).unwrap())
            .stderr(fs::File::create(self.root.join("stderr")).unwrap())
            .stdin(Stdio::piped());
    }

    pub fn spawn(&self, profile: &str) -> OwnedChild {
        let mut command = self.command();
        command.env("ARC_IPC_PROFILE", profile);
        let mut child = OwnedChild::new(command.spawn().unwrap());
        self.wait_for(&mut child, "arc-daemon ready.");
        child
    }

    pub fn wait_for(&self, child: &mut OwnedChild, marker: &str) {
        let end = Instant::now() + Duration::from_secs(10);
        loop {
            let stdout = fs::read_to_string(self.root.join("stdout")).unwrap();
            if stdout.contains(marker) {
                return;
            }
            assert!(
                child.child.try_wait().unwrap().is_none(),
                "child exited: {stdout} {}",
                fs::read_to_string(self.root.join("stderr")).unwrap()
            );
            assert!(Instant::now() < end, "missing {marker}: {stdout}");
            thread::sleep(Duration::from_millis(1));
        }
    }

    pub fn connect(&self) -> UnixStream {
        connect(&self.socket)
    }

    pub fn cli(&self, operation: &str) -> std::process::Output {
        let path = std::path::Path::new(env!("CARGO_BIN_EXE_arc-daemon")).with_file_name("arcctl");
        assert!(
            path.is_file(),
            "workspace CLI prerequisite: {}",
            path.display()
        );
        Command::new(path)
            .arg(operation)
            .env(arc_core::ARC_SOCKET_PATH, &self.socket)
            .env(arc_core::ARC_DB_PATH, &self.database)
            .output()
            .unwrap()
    }

    pub fn assert_cli(&self) {
        let status = self.cli("status");
        assert!(status.status.success(), "{status:?}");
        assert!(status.stderr.is_empty());
        let text = String::from_utf8(status.stdout).unwrap();
        assert!(text.contains("Sweeper off."), "{text}");
        assert!(text.contains(self.socket.to_str().unwrap()), "{text}");
        let doctor = self.cli("doctor");
        assert!(doctor.status.success(), "{doctor:?}");
        assert!(doctor.stderr.is_empty());
        let text = String::from_utf8(doctor.stdout).unwrap();
        assert_eq!(text, "ok. IPC owner-only. Tray not connected. Vendors not detected. Command classification not checked. Sweeper off. Schema compatible. Version 1. Migration history verified.\n");
        assert!(!text.contains('\u{1b}'));
    }

    pub fn report(&self) -> serde_json::Value {
        let bytes = fs::read(self.root.join("runtime.json")).unwrap();
        assert!(bytes.len() <= 4096);
        let report = serde_json::from_slice(&bytes).unwrap();
        assert_report(&report);
        assert!(!self.socket.exists());
        let database = arc_storage::open_and_migrate(&self.database).unwrap();
        let integrity: String = database
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .unwrap();
        assert_eq!(integrity, "ok");
        database.close().unwrap();
        report
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

pub fn connect(socket: &std::path::Path) -> UnixStream {
    let stream = UnixStream::connect(socket).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    stream
}

pub struct OwnedChild {
    pub child: Child,
    reaped: bool,
}
impl OwnedChild {
    pub fn new(child: Child) -> Self {
        Self {
            child,
            reaped: false,
        }
    }
    pub fn stop(&mut self, signal: i32) -> (u128, u64) {
        let start = Instant::now();
        assert_eq!(unsafe { libc::kill(self.child.id() as i32, signal) }, 0);
        loop {
            let mut status = 0;
            let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
            let pid = unsafe {
                libc::wait4(
                    self.child.id() as i32,
                    &mut status,
                    libc::WNOHANG,
                    &mut usage,
                )
            };
            if pid > 0 {
                self.reaped = true;
                assert!(libc::WIFEXITED(status));
                assert_eq!(libc::WEXITSTATUS(status), 0);
                assert!(start.elapsed() <= Duration::from_secs(5));
                let native = u64::try_from(usage.ru_maxrss).unwrap();
                return (
                    start.elapsed().as_micros(),
                    if cfg!(target_os = "linux") {
                        native * 1024
                    } else {
                        native
                    },
                );
            }
            assert_eq!(pid, 0, "wait4: {}", std::io::Error::last_os_error());
            assert!(
                start.elapsed() < Duration::from_secs(5),
                "signal-to-exit exceeds five seconds"
            );
            thread::sleep(Duration::from_millis(1));
        }
    }
}
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if !self.reaped {
            if self.child.try_wait().ok().flatten().is_none() {
                let _ = self.child.kill();
            }
            let _ = self.child.wait();
        }
    }
}

pub fn assert_report(report: &serde_json::Value) {
    assert_eq!(report["event"], "ipc_shutdown");
    for key in [
        "accounted",
        "handlers_reaped",
        "worker_joined",
        "connection_closed",
        "gauges_zero",
        "socket_removed",
        "deadline_met",
    ] {
        assert_eq!(report[key], true, "{key}: {report}");
    }
    assert_eq!(report["handlers_remaining"], 0);
    let m = &report["metrics"];
    let cap = |key: &str| report["limits"][key].as_u64().unwrap();
    for (key, bound) in [
        ("connections", cap("connections")),
        ("requests", cap("requests")),
        ("tracked_tasks", cap("connections")),
        ("db_queued", cap("db_queue")),
        ("db_executing", 1),
        ("db_outstanding", cap("db_queue") + 1),
        (
            "request_body_bytes",
            cap("requests") * arc_ipc::MAX_FRAME_BYTES as u64,
        ),
        (
            "response_bytes",
            cap("requests") * arc_ipc::MAX_FRAME_BYTES as u64,
        ),
        ("rejection_work", 1),
    ] {
        assert_eq!(m[key]["current"], 0, "{key}: {report}");
        assert!(
            m[key]["high_water"].as_u64().unwrap() <= bound,
            "{key}: {report}"
        );
    }
    assert_eq!(m["handlers_spawned"], m["handlers_reaped"]);
    assert_eq!(m["worker_started"], 1);
    assert_eq!(m["worker_joined"], 1);
    assert_eq!(report["supervisor"]["retry_tasks"], 0);
}

pub fn receipt(name: &str, value: &serde_json::Value) {
    eprintln!("bounded-runtime-{name}: {value}");
    if let Some(root) = std::env::var_os("ARC_TEST_EVIDENCE_DIR") {
        let root = PathBuf::from(root);
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join(format!("{name}.json")),
            serde_json::to_vec_pretty(value).unwrap(),
        )
        .unwrap();
    }
}

pub fn assert_closed(mut stream: UnixStream, budget: Duration) {
    let start = Instant::now();
    // macOS may reject SO_RCVTIMEO after the peer has already disconnected.
    // Nonblocking reads also keep this observation on one absolute deadline.
    stream.set_nonblocking(true).unwrap();
    let mut byte = [0];
    loop {
        match stream.read(&mut byte) {
            Ok(0) => break,
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::BrokenPipe
                ) =>
            {
                break
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(
                    start.elapsed() < budget,
                    "peer did not close within {budget:?}"
                );
                thread::sleep(Duration::from_millis(1));
            }
            value => panic!("expected closure: {value:?}"),
        }
    }
    assert!(start.elapsed() <= budget);
}

pub fn send(stream: &mut UnixStream, request: &arc_ipc::Request) {
    arc_ipc::write_message(stream, request).unwrap();
    stream.flush().unwrap();
}

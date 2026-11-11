//! Shared native daemon fixtures; callers retain support::serial() through cleanup.
//! An explicit binary path lets another workspace crate reuse private fixtures
//! without a production dependency on the daemon or storage implementation.
use std::{
    fs,
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
    daemon_binary: PathBuf,
}

impl Fixture {
    pub fn with_binary(daemon_binary: PathBuf) -> Self {
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
            daemon_binary,
        }
    }

    pub fn command(&self) -> Command {
        let mut command = Command::new(&self.daemon_binary);
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

    pub fn binary(&self, name: &str) -> PathBuf {
        self.daemon_binary.with_file_name(name)
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

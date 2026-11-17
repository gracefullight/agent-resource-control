//! Shared native daemon fixtures; callers retain support::serial() through cleanup.
//! An explicit binary path lets another workspace crate reuse private fixtures
//! without a production dependency on the daemon or storage implementation.
use std::{
    fs,
    io::{self, Read},
    os::{
        fd::AsRawFd,
        unix::{fs::DirBuilderExt, net::UnixStream, process::CommandExt},
    },
    path::PathBuf,
    process::{Child, Command, Output, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

#[path = "native_readiness.rs"]
pub mod native_readiness;

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
        native_readiness::wait_for(self, child, marker);
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
        if thread::panicking() {
            for name in ["stdout", "stderr"] {
                if let Ok(bytes) = fs::read(self.root.join(name)) {
                    let tail = &bytes[bytes.len().saturating_sub(8192)..];
                    eprintln!("owned fixture {name}: {}", String::from_utf8_lossy(tail));
                }
            }
        }
        if !thread::panicking() && !self.root.join("owned-cleanup-incomplete").exists() {
            if let Err(error) = fs::remove_dir_all(&self.root) {
                if error.kind() != io::ErrorKind::NotFound {
                    panic!(
                        "cannot remove owned fixture {}: {error}",
                        self.root.display()
                    );
                }
            }
        }
    }
}

pub fn connect(socket: &std::path::Path) -> UnixStream {
    let stream = UnixStream::connect(socket).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(60)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(60)))
        .unwrap();
    stream
}

pub struct OwnedChild {
    pub child: Child,
    reaped: bool,
    owned: Vec<arc_core::process_identity::ProcessIdentity>,
    identity_directory: Option<PathBuf>,
    lifetime: Option<Lifetime>,
}

// Both channels are inherited before the wrapper can spawn. Closing cancel
// reaches even a target that has not published its identity yet; EOF on done
// proves all inherited writers have closed, including escaped descendants.
struct Lifetime {
    cancel: Option<UnixStream>,
    done: UnixStream,
    closed: bool,
}
impl OwnedChild {
    pub fn new(child: Child) -> Self {
        Self {
            child,
            reaped: false,
            owned: Vec::new(),
            identity_directory: None,
            lifetime: None,
        }
    }
    /// Establish ownership before spawn, including the pre-publication interval.
    #[allow(dead_code)] // Shared across CLI and desktop acceptance targets.
    pub fn spawn_with_descendants(mut command: Command, directory: PathBuf) -> io::Result<Self> {
        fs::create_dir_all(&directory)?;
        let (cancel, receiver) = UnixStream::pair()?;
        let (done, token) = UnixStream::pair()?;
        done.set_nonblocking(true)?;
        let (cancel_fd, token_fd) = (receiver.as_raw_fd(), token.as_raw_fd());
        command
            .env("ARC_FIXTURE_OWNERS", &directory)
            .env("ARC_FIXTURE_CANCEL_FD", cancel_fd.to_string())
            .env("ARC_FIXTURE_LIFETIME_FD", token_fd.to_string());
        // SAFETY: only fcntl runs after fork. These two exact fixture-owned FDs
        // intentionally survive exec; other endpoints retain CLOEXEC. Consume
        // Command so the hook cannot later reuse closed descriptor numbers.
        unsafe {
            command.pre_exec(move || {
                for fd in [cancel_fd, token_fd] {
                    if libc::fcntl(fd, libc::F_SETFD, 0) < 0 {
                        return Err(io::Error::last_os_error());
                    }
                }
                Ok(())
            });
        }
        let mut owner = Self::new(command.spawn()?);
        owner.identity_directory = Some(directory);
        owner.lifetime = Some(Lifetime {
            cancel: Some(cancel),
            done,
            closed: false,
        });
        Ok(owner)
    }
    #[allow(dead_code)] // Some native suites own only direct children.
    pub fn own_process(&mut self, identity: arc_core::process_identity::ProcessIdentity) {
        self.owned.push(identity);
    }
    fn stop_descendants(&mut self) {
        if let Some(lifetime) = &mut self.lifetime {
            drop(lifetime.cancel.take());
        }
        use arc_core::process_identity::{probe_live, IdentityStatus, ProcessIdentity};
        if let Some(root) = &self.identity_directory {
            if let Ok(entries) = fs::read_dir(root) {
                for entry in entries.flatten() {
                    if let Ok(bytes) = fs::read(entry.path()) {
                        if let Ok(identity) = serde_json::from_slice::<ProcessIdentity>(&bytes) {
                            if !self.owned.contains(&identity) {
                                self.owned.push(identity);
                            }
                        }
                    }
                }
            }
        }
        for identity in &self.owned {
            if probe_live(identity) == IdentityStatus::VerifiedMatch {
                // SIGKILL also terminates a separately grouped, stopped target.
                unsafe {
                    libc::kill(identity.pid as i32, libc::SIGKILL);
                }
            }
        }
    }
    fn descendants_gone(&mut self) -> bool {
        use arc_core::process_identity::ProcessIdentity;
        let mut gone = true;
        for identity in &self.owned {
            // An orphan's native reaper owns waitpid; for our direct children
            // consume the terminal status here. capture also sees zombies, so
            // termination alone is insufficient to finish cleanup.
            if ProcessIdentity::capture(identity.pid).as_ref().ok() == Some(identity) {
                unsafe {
                    libc::waitpid(identity.pid as i32, std::ptr::null_mut(), libc::WNOHANG);
                }
            }
            match ProcessIdentity::capture(identity.pid) {
                Ok(current) if current == *identity => gone = false,
                Ok(_) => (),
                Err(e)
                    if e.kind() == io::ErrorKind::NotFound
                        || e.raw_os_error() == Some(libc::ESRCH) => {}
                Err(_) => gone = false,
            }
        }
        if let Some(lifetime) = &mut self.lifetime {
            if !lifetime.closed {
                match lifetime.done.read(&mut [0u8]) {
                    Ok(0) => lifetime.closed = true,
                    Err(e)
                        if e.kind() == io::ErrorKind::WouldBlock
                            || e.kind() == io::ErrorKind::Interrupted => {}
                    result => panic!("invalid owned-child lifetime channel: {result:?}"),
                }
            }
            gone &= lifetime.closed;
        }
        gone
    }
    /// Drain both pipes under one watchdog without blocking reader threads.
    pub fn wait_with_output(&mut self) -> io::Result<Output> {
        self.wait_with_output_for(Duration::from_secs(60), false)
    }
    pub fn wait_with_output_for(
        &mut self,
        timeout: Duration,
        owned_group: bool,
    ) -> io::Result<Output> {
        fn nonblocking(pipe: Option<&impl AsRawFd>) -> io::Result<()> {
            if let Some(pipe) = pipe {
                let fd = pipe.as_raw_fd();
                let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
                if flags < 0
                    || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0
                {
                    return Err(io::Error::last_os_error());
                }
            }
            Ok(())
        }
        fn drain(pipe: &mut Option<impl Read>, bytes: &mut Vec<u8>) -> io::Result<()> {
            let mut buffer = [0; 16384];
            // Bound each turn so a continuously writing child cannot starve
            // stderr, cancellation, native wait or the absolute watchdog.
            for _ in 0..16 {
                let Some(reader) = pipe.as_mut() else {
                    break;
                };
                match reader.read(&mut buffer) {
                    Ok(0) => {
                        *pipe = None;
                        break;
                    }
                    Ok(n) => bytes.extend_from_slice(&buffer[..n]),
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(e),
                }
            }
            Ok(())
        }
        drop(self.child.stdin.take());
        let mut out = self.child.stdout.take();
        let mut err = self.child.stderr.take();
        nonblocking(out.as_ref())?;
        nonblocking(err.as_ref())?;
        let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
        let end = Instant::now() + timeout;
        let mut status = None;
        loop {
            drain(&mut out, &mut stdout)?;
            drain(&mut err, &mut stderr)?;
            if status.is_none() {
                status = self.child.try_wait()?;
                if status.is_some() {
                    self.reaped = true;
                    self.stop_descendants();
                }
            }
            if let (Some(status), None, None) = (status, &out, &err) {
                self.stop_descendants();
                if !self.descendants_gone() {
                    if Instant::now() >= end {
                        return Err(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "owned descendants did not exit/reap",
                        ));
                    }
                    thread::yield_now();
                    continue;
                }
                return Ok(Output {
                    status,
                    stdout,
                    stderr,
                });
            }
            if Instant::now() >= end {
                self.stop_descendants();
                if !self.reaped {
                    native_readiness::kill_and_reap(&mut self.child, owned_group)?;
                    self.reaped = true;
                }
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "owned child exit/pipe drain watchdog",
                ));
            }
            thread::yield_now();
        }
    }
    pub fn stop(&mut self, signal: i32) -> (u128, u64) {
        assert_eq!(unsafe { libc::kill(self.child.id() as i32, signal) }, 0);
        self.wait_for_exit()
    }
    pub fn wait_for_exit(&mut self) -> (u128, u64) {
        let start = Instant::now();
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
                start.elapsed() < Duration::from_secs(60),
                "owned child did not exit before fixture watchdog"
            );
            thread::sleep(Duration::from_millis(1));
        }
    }
}
impl std::ops::Deref for OwnedChild {
    type Target = Child;
    fn deref(&self) -> &Child {
        &self.child
    }
}
impl std::ops::DerefMut for OwnedChild {
    fn deref_mut(&mut self) -> &mut Child {
        &mut self.child
    }
}
impl Drop for OwnedChild {
    fn drop(&mut self) {
        // Targets must be terminated before the wrapper, so its native waiter
        // can reap them even if wrapper and target both suspended themselves.
        self.stop_descendants();
        drop(self.child.stdin.take());
        drop(self.child.stdout.take());
        drop(self.child.stderr.take());
        if !self.reaped {
            match self.child.try_wait() {
                Ok(Some(_)) => self.reaped = true,
                Err(error) if error.raw_os_error() == Some(libc::ECHILD) => self.reaped = true,
                _ => (),
            }
        }
        if !self.reaped {
            if self.lifetime.is_some() {
                // Cooperative fixtures must get cancellation before native
                // termination: a pre-adoption parent still owns its Child and
                // must reap it before closing the inherited lifetime token.
                unsafe {
                    libc::kill(self.child.id() as i32, libc::SIGCONT);
                }
            } else if self.owned.is_empty() {
                let _ = self.child.kill();
            } else {
                unsafe {
                    libc::kill(self.child.id() as i32, libc::SIGTERM);
                    libc::kill(self.child.id() as i32, libc::SIGCONT);
                }
            }
        }
        let end = Instant::now() + Duration::from_secs(60);
        loop {
            self.stop_descendants(); // includes late atomic publications
            if !self.reaped {
                match self.child.try_wait() {
                    Ok(Some(_)) => self.reaped = true,
                    Err(e) if e.raw_os_error() == Some(libc::ECHILD) => self.reaped = true,
                    _ => (),
                }
            }
            if self.descendants_gone() && self.reaped {
                return;
            }
            if Instant::now() >= end {
                if !self.reaped {
                    if let Err(error) = native_readiness::kill_and_reap(&mut self.child, false) {
                        eprintln!("owned child cleanup failed: {error}");
                    }
                }
                if let Some(root) = self
                    .identity_directory
                    .as_ref()
                    .and_then(|path| path.parent())
                {
                    let _ = fs::write(
                        root.join("owned-cleanup-incomplete"),
                        b"retain fixture: child cleanup unconfirmed",
                    );
                }
                if thread::panicking() {
                    eprintln!("owned descendant cleanup unconfirmed");
                } else {
                    panic!("owned descendant cleanup unconfirmed");
                }
                return;
            }
            thread::sleep(Duration::from_millis(1));
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
    // Product-owned monotonic outcome; parent scheduling is only a fixture watchdog.
    assert!(
        report["shutdown_elapsed_ms"].as_u64().unwrap()
            <= report["limits"]["shutdown_timeout_ms"].as_u64().unwrap()
    );
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

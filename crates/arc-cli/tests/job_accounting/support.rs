use arc_core::job_meter::{
    AccountingRelation, JobMeterReport, JobMeterSummary, QueueWait, SummaryAccounting,
};
use arc_ipc::{
    jobs::*,
    traffic::{JobRequest, JobResult, LineageContext, Resources},
    Request, Response,
};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    fs,
    os::unix::{
        fs::{symlink, DirBuilderExt, PermissionsExt},
        net::UnixListener,
    },
    path::PathBuf,
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

#[derive(Clone, Copy, Debug)]
pub enum PeerMode {
    Normal,
    LostReportAck,
    MalformedReportAck,
    UnsupportedReport,
    StalledReport,
    SlowCompletion,
    BypassThenUnavailable,
    UnsupportedBunThenUnavailable,
    Queued,
    // Used by the source-included accounting unit tests; this support module
    // is also compiled separately for the integration target.
    #[allow(dead_code)]
    QueuedExpireReport,
    UntrackedRoot,
}
#[derive(Debug, Default)]
pub struct State {
    pub reports: HashMap<String, JobMeterReport>,
    pub report_requests: Vec<(String, Value)>,
    pub summary_reads: usize,
    pub attached: bool,
    pub ancestry_diagnostics: HashMap<String, String>,
    queued_poll_seen: bool,
    jobs: HashMap<String, JobSnapshot>,
    relations: HashMap<String, AccountingRelation>,
    contexts: HashMap<String, LineageContext>,
    untracked_root: Option<String>,
}
pub struct Fixture {
    pub root: PathBuf,
    pub socket: PathBuf,
    pub state: Arc<Mutex<State>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    queued: Option<QueuedControl>,
    attachments: Receiver<()>,
}

struct QueuedControl {
    reached: Receiver<()>,
    release: Sender<()>,
}

struct QueuedPeer {
    reached: Sender<()>,
    release: Receiver<()>,
}
impl Fixture {
    pub fn new(mode: PeerMode) -> Self {
        Self::new_with_tool(mode, fixture_executable())
    }
    pub fn new_with_tool(mode: PeerMode, tool: &str) -> Self {
        let root = arc_core::runtime_root().unwrap().join(format!(
            "ja-{}",
            arc_core::process_identity::random_job_id().unwrap()
        ));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        symlink(tool, root.join("cargo")).unwrap();
        symlink(tool, root.join("bun")).unwrap();
        let socket = root.join("ipc");
        let listener = UnixListener::bind(&socket).unwrap();
        fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).unwrap();
        let state = Arc::new(Mutex::new(State::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let (shared, stopping, endpoint) = (state.clone(), stop.clone(), socket.clone());
        let (attachment_tx, attachments) = mpsc::channel();
        let (queued, peer) = if matches!(mode, PeerMode::Queued | PeerMode::QueuedExpireReport) {
            let (reached_tx, reached_rx) = mpsc::channel();
            let (release_tx, release_rx) = mpsc::channel();
            (
                Some(QueuedControl {
                    reached: reached_rx,
                    release: release_tx,
                }),
                Some(QueuedPeer {
                    reached: reached_tx,
                    release: release_rx,
                }),
            )
        } else {
            (None, None)
        };
        let worker = thread::spawn(move || {
            while !stopping.load(Ordering::SeqCst) {
                let (mut stream, _) = match listener.accept() {
                    Ok(stream) => stream,
                    Err(e) => panic!("{e}"),
                };
                if stopping.load(Ordering::SeqCst) {
                    break;
                }
                // A timed-out client may already have closed this queued
                // connection; Darwin can then reject socket option changes.
                if stream
                    .set_read_timeout(Some(Duration::from_secs(60)))
                    .is_err()
                    || stream
                        .set_write_timeout(Some(Duration::from_secs(60)))
                        .is_err()
                {
                    continue;
                }
                let Ok(request) = arc_ipc::read_message::<_, Request>(&mut stream) else {
                    continue;
                };
                let response = handle(
                    &request,
                    mode,
                    &endpoint,
                    &mut shared.lock().unwrap(),
                    peer.as_ref(),
                    &attachment_tx,
                );
                if let Some(response) = response {
                    let _ = arc_ipc::write_response(&mut stream, &response);
                }
                if matches!(
                    mode,
                    PeerMode::BypassThenUnavailable | PeerMode::UnsupportedBunThenUnavailable
                ) && matches!(
                    request.operation.as_str(),
                    "traffic.job" | "traffic.bun.job"
                ) {
                    // Model a confirmed bypass/unsupported reply followed by a
                    // disconnected registration endpoint, without a timed stall.
                    break;
                }
            }
        });
        Self {
            root,
            socket,
            state,
            stop,
            worker: Some(worker),
            queued,
            attachments,
        }
    }
    pub fn command(&self, scheduled: bool, mode: &str) -> Command {
        let fixture = [fixture_executable()];
        self.command_argv(
            if scheduled {
                &["cargo", "build"]
            } else {
                &fixture
            },
            mode,
        )
    }
    pub fn command_argv(&self, argv: &[&str], mode: &str) -> Command {
        let cli = match option_env!("CARGO_BIN_EXE_arcctl") {
            Some(path) => path,
            None => panic!("integration fixture requires arcctl"),
        };
        let mut command = Command::new(cli);
        command.env_clear().args(["exec", "--"]);
        command.args(argv);
        command
            .env(arc_core::ARC_SOCKET_PATH, &self.socket)
            .env("PATH", &self.root)
            .env("HOME", &self.root)
            .env("TMPDIR", &self.root)
            .env("ARC_FIXTURE_ACCOUNTING", mode)
            .env("ARC_FIXTURE_OWNERS", self.root.join("owners"))
            .env("ARC_METER_TEST_MARKER", self.root.join("marker"))
            .env("ARC_METER_TEST_CLI", cli);
        // Coverage-instrumented child binaries must write into the runner's
        // profile tree, independent of the other inherited fixture settings.
        if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
            command.env("LLVM_PROFILE_FILE", profile);
        }
        command
    }

    /// Exercise the installed CLI either directly or under a live shell parent.
    pub fn production_command(&self, scheduled: bool, mode: &str, wrapped: bool) -> Command {
        let base = self.command(scheduled, mode);
        if !wrapped {
            return base;
        }
        let mut shell = Command::new("/bin/sh");
        shell.env_clear().args([
            "-c",
            "/usr/bin/env _=/unrelated/caller \"$@\"; result=$?; exit \"$result\"",
            "arc-fixture",
        ]);
        shell.arg(base.get_program()).args(base.get_args());
        for (key, value) in base.get_envs() {
            if let Some(value) = value {
                shell.env(key, value);
            }
        }
        shell
    }

    /// Reuse the production execution source with a test-owned clock. The
    /// fixture handshake is unavailable to the production arcctl binary.
    pub fn clocked_command(&self, scheduled: bool, mode: &str) -> Command {
        let base = self.command(scheduled, mode);
        let key = self.root.join("clock-key");
        fs::write(&key, b"arc-fixture-clock-v1").unwrap();
        fs::write(self.clock_path(), b"0").unwrap();
        let mut clocked = Command::new(fixture_executable());
        clocked.env_clear().arg("--clocked-fixture-exec");
        clocked.args(base.get_args().skip(2));
        for (key, value) in base.get_envs() {
            if let Some(value) = value {
                clocked.env(key, value);
            } else {
                clocked.env_remove(key);
            }
        }
        clocked
            .env("ARC_FIXTURE_CLOCK_KEY", key)
            .env("ARC_FIXTURE_CLOCK", self.clock_path())
            .env("ARC_METER_TEST_CLI", fixture_executable())
            .env("ARC_METER_TEST_CLOCKED", "1");
        clocked
    }

    pub fn clock_path(&self) -> PathBuf {
        self.root.join("clock")
    }

    pub fn wait_for_queued_poll(&self) {
        self.queued
            .as_ref()
            .unwrap()
            .reached
            .recv_timeout(Duration::from_secs(60))
            .expect("queued poll was not observed");
    }

    pub fn release_queued(&self) {
        self.queued.as_ref().unwrap().release.send(()).unwrap();
    }

    pub fn wait_for_attachment(&self) {
        self.attachments
            .recv_timeout(Duration::from_secs(60))
            .expect("fixture attachment watchdog");
    }

    pub fn assert_live_parent(&self, spawned_pid: u32, wrapped: bool) {
        let state = self.state.lock().unwrap();
        let observer = &state
            .jobs
            .values()
            .find(|job| job.phase == Phase::Running)
            .expect("attached job")
            .observer;
        if wrapped {
            let ancestors = arc_platform::job_lineage::ancestors(observer).unwrap();
            assert_eq!(
                ancestors.first().map(|parent| parent.pid),
                Some(spawned_pid)
            );
            assert_ne!(observer.pid, spawned_pid);
        } else {
            assert_eq!(observer.pid, spawned_pid);
        }
    }
}
fn fixture_executable() -> &'static str {
    match option_env!("CARGO_BIN_EXE_arc-exec-fixture") {
        Some(path) => path,
        None => panic!("integration fixture requires child"),
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        self.queued.take();
        // Wake a blocking accept without polling or touching another fixture.
        let _ = std::os::unix::net::UnixStream::connect(&self.socket);
        let result = self.worker.take().unwrap().join();
        if result.is_ok()
            && !std::thread::panicking()
            && !self.root.join("owned-cleanup-incomplete").exists()
        {
            let _ = fs::remove_dir_all(&self.root);
        }
        if !std::thread::panicking() {
            result.unwrap();
        }
    }
}
#[path = "support/state.rs"]
mod state;
use state::{attached, capture_relation, claimed, completed, queued_result, registered};

#[path = "support/handle.rs"]
mod handle;
use handle::handle;

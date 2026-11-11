//! Test-only adapters. Socket/transport, typed projection, command registration
//! and the quit/cancellation path all come from the production desktop crate.
use super::{execution, native};
use arc_core::process_identity::{probe_live, IdentityStatus, ProcessIdentity};
use arc_desktop::{
    bridge::DesktopBridge,
    dto::{JobDetail, JobLookup, Received},
    error::ErrorCode,
};
use arc_ipc::{
    jobs::{JobSnapshot, Phase, StatusRequest, StatusResult, OPERATION_JOB_STATUS},
    Request, Response,
};
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    os::unix::{
        fs::PermissionsExt,
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
mod client_process;
pub use client_process::{desktop_client, is_client_process, run_client_process};

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

fn workspace() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap()
}

fn binary(name: &str) -> PathBuf {
    // Same Cargo profile/target as this test, including cargo-llvm-cov's own
    // target. Do not fall back to another checkout or a stale normal binary.
    let exe = std::env::current_exe().unwrap();
    assert_eq!(exe.parent().unwrap().file_name().unwrap(), "deps");
    let path = exe.parent().unwrap().parent().unwrap().join(name);
    assert!(
        path.is_file(),
        "native prerequisite {}; build arc-daemon and arc-cli bins in the same Cargo target",
        path.display()
    );
    path
}

fn write_json(path: &Path, value: &Value) {
    fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

pub fn detail(reply: &Received<JobLookup>) -> &JobDetail {
    match &reply.data {
        JobLookup::Found { job } => job,
        JobLookup::NotFound {} => panic!("real recorded job missing"),
    }
}

pub struct RealDaemon {
    pub fixture: native::Fixture,
    pub bridge: DesktopBridge,
    pub evidence: PathBuf,
    daemon: Option<native::OwnedChild>,
}

impl RealDaemon {
    pub fn new(name: &str) -> Self {
        let evidence = workspace()
            .join("target/desktop-foundation/task8")
            .join(std::process::id().to_string())
            .join(name);
        fs::create_dir_all(&evidence).unwrap();
        let fixture = native::Fixture::with_binary(binary("arc-daemon"));
        assert_eq!(
            fs::metadata(&fixture.root).unwrap().permissions().mode() & 0o777,
            0o700
        );
        let bridge = DesktopBridge::from_socket_path(fixture.socket.clone());
        let mut this = Self {
            fixture,
            bridge,
            evidence,
            daemon: None,
        };
        this.start();
        this
    }

    pub fn start(&mut self) {
        assert!(self.daemon.is_none());
        let mut command = self.fixture.command();
        command.env("ARC_IPC_PROFILE", "test");
        self.profile_command(&mut command);
        self.daemon = Some(native::OwnedChild::new(command.spawn().unwrap()));
        self.fixture
            .wait_for(self.daemon.as_mut().unwrap(), "arc-daemon ready.");
        assert_eq!(
            fs::metadata(&self.fixture.socket)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        let hello = Request::new("desktop-fixture-handshake", arc_ipc::OPERATION_HELLO);
        let reply = arc_ipc::exchange(&mut self.fixture.connect(), &hello).unwrap();
        assert_eq!(reply.protocol_version, 1);
        assert_eq!(reply.request_id, hello.request_id);
        assert_eq!(reply.status, "ok");
        assert_eq!(reply.result.as_ref().unwrap()["protocol_version"], 1);
        let health = self.bridge.daemon_health().unwrap();
        assert_eq!(health.data.protocol_version, 1);
        let recent = self.bridge.recent_jobs().unwrap();
        assert!(!recent.data.liveness_fresh);
        assert_eq!(recent.data.limit, 50);
        self.save(&format!("handshake-{}", self.identity().pid), &json!({
            "atMs": now_ms(), "daemonIdentity": self.identity(), "handshake": reply,
            "health": health, "recent": recent, "privateRuntimeMode": "0700", "socketMode": "0600",
            "productionInstanceAnchorPreserved": true,
        }));
    }

    pub fn identity(&self) -> ProcessIdentity {
        ProcessIdentity::capture(self.daemon.as_ref().unwrap().child.id()).unwrap()
    }

    pub fn save(&self, name: &str, value: &Value) {
        write_json(&self.evidence.join(format!("{name}.json")), value);
    }

    pub fn profile_command(&self, command: &mut Command) {
        // Preserve inherited coverage collection; its merger owns those files.
        // Uninstrumented runs truthfully report an empty profile inventory.
        if std::env::var_os("LLVM_PROFILE_FILE").is_none() {
            let profiles = self.evidence.join("profiles");
            fs::create_dir_all(&profiles).unwrap();
            command.env("LLVM_PROFILE_FILE", profiles.join("arc-%p-%m.profraw"));
        }
    }

    pub fn profiles(&self, pids: &[u32]) -> Value {
        let pattern = std::env::var_os("LLVM_PROFILE_FILE")
            .map(PathBuf::from)
            .unwrap_or_else(|| self.evidence.join("profiles/arc-%p-%m.profraw"));
        let root = pattern.parent().unwrap();
        let inventory: Vec<_> = pids
            .iter()
            .map(|pid| {
                let files: Vec<_> = fs::read_dir(root).unwrap().filter_map(Result::ok)
                .filter(|entry| entry.file_name().to_string_lossy().contains(&format!("-{pid}-")))
                .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "profraw"))
                .map(|entry| {
                    let bytes = entry.metadata().unwrap().len();
                    assert!(bytes > 0);
                    json!({"path": entry.path().strip_prefix(workspace()).unwrap(), "bytes": bytes})
                }).collect();
                if std::env::var_os("ARC_DESKTOP_REQUIRE_PROFILES").is_some() {
                    assert!(
                        !files.is_empty(),
                        "missing instrumented native child profile for {pid}"
                    );
                }
                json!({"pid": pid, "files": files})
            })
            .collect();
        json!(inventory)
    }

    pub fn stop(&mut self, label: &str) {
        let identity = self.identity();
        assert_eq!(probe_live(&identity), IdentityStatus::VerifiedMatch);
        let (shutdown_us, rss) = self.daemon.as_mut().unwrap().stop(libc::SIGTERM);
        self.daemon.take();
        let report: Value =
            serde_json::from_slice(&fs::read(self.fixture.root.join("runtime.json")).unwrap())
                .unwrap();
        native::assert_report(&report);
        assert!(!self.fixture.socket.exists());
        self.save(
            label,
            &json!({
                "daemonIdentity": identity, "shutdownMicros": shutdown_us, "maxRssBytes": rss,
                "atMs": now_ms(), "runtime": report, "profiles": self.profiles(&[identity.pid]),
            }),
        );
        self.copy_logs(label);
        fs::rename(
            self.fixture.root.join("runtime.json"),
            self.fixture.root.join(format!("{label}-runtime.json")),
        )
        .unwrap();
    }

    fn copy_logs(&self, label: &str) {
        for name in ["stdout", "stderr", "runtime.json"] {
            let _ = fs::copy(
                self.fixture.root.join(name),
                self.evidence.join(format!("{label}-{name}")),
            );
        }
    }

    pub fn snapshot(&self, id: &str) -> JobSnapshot {
        let mut request = Request::new("desktop-native-witness", OPERATION_JOB_STATUS);
        request.payload = json!(StatusRequest::ById {
            job_id: arc_ipc::jobs::JobId::try_from(id.to_owned()).unwrap()
        });
        let response = arc_ipc::exchange(&mut self.fixture.connect(), &request).unwrap();
        assert_eq!(response.protocol_version, 1);
        assert_eq!(response.request_id, request.request_id);
        assert_eq!(response.status, "ok");
        let result: StatusResult = serde_json::from_value(response.result.unwrap()).unwrap();
        assert!(result.is_valid());
        match result {
            StatusResult::Found { job } => {
                assert_eq!(job.job_id.as_str(), id);
                *job
            }
            _ => panic!("record missing"),
        }
    }

    pub fn wait_running(&self, identity: &ProcessIdentity) -> Received<JobLookup> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let recent = self.bridge.recent_jobs().unwrap();
            assert!(recent.data.jobs.len() <= 50);
            if let Some(summary) = recent.data.jobs.first() {
                let reply = self.bridge.job_by_id(summary.job_id.as_str()).unwrap();
                if detail(&reply).phase == Phase::Running {
                    assert_eq!(
                        self.snapshot(summary.job_id.as_str()).child.as_ref(),
                        Some(identity)
                    );
                    return reply;
                }
            }
            assert!(
                Instant::now() < deadline,
                "native child never observed running"
            );
            thread::sleep(Duration::from_millis(5));
        }
    }

    pub fn fault_server(
        &self,
        fault: Fault,
        snapshot: Option<JobSnapshot>,
    ) -> (DesktopBridge, thread::JoinHandle<Value>) {
        let socket = self.fixture.root.join(format!("{}.sock", fault.name()));
        let listener = UnixListener::bind(&socket).unwrap();
        fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).unwrap();
        let server = thread::spawn(move || {
            let (mut stream, value) = execution::accept(&listener);
            let request: Request = serde_json::from_value(value).unwrap();
            assert_eq!(request.protocol_version, 1);
            assert_eq!(request.operation, OPERATION_JOB_STATUS);
            assert_eq!(request.payload["query"], "BY_ID");
            match fault {
                Fault::Json => arc_ipc::write_frame(&mut stream, b"{broken-json").unwrap(),
                Fault::Oversized => stream
                    .write_all(&(arc_ipc::MAX_FRAME_BYTES + 1).to_be_bytes())
                    .unwrap(),
                Fault::Shape => arc_ipc::write_response(
                    &mut stream,
                    &Response::ok(&request.request_id, json!({"kind":"FOUND","job":[]})),
                )
                .unwrap(),
                Fault::Correlation => arc_ipc::write_response(
                    &mut stream,
                    &Response::ok("wrong-correlation", json!({"kind":"NOT_FOUND"})),
                )
                .unwrap(),
                Fault::Protocol => {
                    let mut reply = Response::ok(&request.request_id, json!({"kind":"NOT_FOUND"}));
                    reply.protocol_version = 99;
                    arc_ipc::write_response(&mut stream, &reply).unwrap();
                }
                Fault::Selection => arc_ipc::write_response(
                    &mut stream,
                    &Response::ok(
                        &request.request_id,
                        json!(StatusResult::Found {
                            job: Box::new(snapshot.unwrap())
                        }),
                    ),
                )
                .unwrap(),
                Fault::Stall => {
                    // Keep the peer open until the production deadline closes
                    // its connection; no timing-dependent fixed server sleep.
                    let deadline = Instant::now() + Duration::from_secs(4);
                    stream.set_nonblocking(true).unwrap();
                    let mut byte = [0];
                    use std::io::Read;
                    loop {
                        match stream.read(&mut byte) {
                            Ok(0) => break,
                            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                                assert!(Instant::now() < deadline);
                                thread::sleep(Duration::from_millis(5));
                            }
                            result => panic!("unexpected stalled-client bytes {result:?}"),
                        }
                    }
                }
            }
            json!({"protocolVersion": request.protocol_version, "operation": request.operation,
                "query": request.payload, "fault": fault.name()})
        });
        (DesktopBridge::from_socket_path(socket), server)
    }
}

impl Drop for RealDaemon {
    fn drop(&mut self) {
        // Only a Child spawned here can be reaped/killed by OwnedChild. Never
        // signal a PID discovered via a socket or the production anchor.
        self.daemon.take();
        self.copy_logs("last");
    }
}

pub struct HeldJob {
    wrapper: native::OwnedChild,
    barrier: Option<UnixStream>,
    pub identity: ProcessIdentity,
    pub wrapper_identity: ProcessIdentity,
    pub handshake: Value,
}

impl HeldJob {
    pub fn launch(case: &RealDaemon, code: u8) -> Self {
        let control = case.fixture.root.join("child.sock");
        let listener = UnixListener::bind(&control).unwrap();
        let mut command = Command::new(case.fixture.binary("arcctl"));
        command
            .args(["exec", "--"])
            .arg(case.fixture.binary("arc-exec-fixture"))
            .env(arc_core::ARC_SOCKET_PATH, &case.fixture.socket)
            .env(arc_core::ARC_DB_PATH, &case.fixture.database)
            .env(
                "ARC_FIXTURE_MODE",
                if code == 0 { "success" } else { "echo" },
            )
            .env("ARC_FIXTURE_CONTROL", &control)
            .env("ARC_FIXTURE_MARKER", case.fixture.root.join("launches"))
            .env_remove("ARC_TRAFFIC_LINEAGE")
            .env_remove("ARC_FIXTURE_PIPE_ROLE")
            .env_remove("ARC_FIXTURE_HOLD_UNATTACHED")
            .env_remove("ARC_FIXTURE_LAUNCH")
            .env_remove("ARC_FIXTURE_FROZEN_TRAFFIC_V1")
            .current_dir(&case.fixture.root)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(fs::File::create(case.evidence.join("execution.stderr")).unwrap());
        case.profile_command(&mut command);
        let wrapper = native::OwnedChild::new(command.spawn().unwrap());
        let wrapper_identity = ProcessIdentity::capture(wrapper.child.id()).unwrap();
        let (barrier, handshake) = execution::accept(&listener);
        let identity: ProcessIdentity =
            serde_json::from_value(handshake["identity"].clone()).unwrap();
        assert_eq!(probe_live(&identity), IdentityStatus::VerifiedMatch);
        assert_eq!(handshake["pid"], identity.pid);
        assert_eq!(handshake["ppid"], wrapper_identity.pid);
        Self {
            wrapper,
            barrier: Some(barrier),
            identity,
            wrapper_identity,
            handshake,
        }
    }

    pub fn release_and_wait(&mut self) -> ExitStatus {
        self.barrier.take().unwrap().write_all(b"x").unwrap();
        execution::wait(&mut self.wrapper.child)
    }
}

impl Drop for HeldJob {
    fn drop(&mut self) {
        if let Some(mut barrier) = self.barrier.take() {
            let _ = barrier.write_all(b"x");
            drop(barrier);
            let deadline = Instant::now() + Duration::from_secs(3);
            while self.wrapper.child.try_wait().ok().flatten().is_none()
                && Instant::now() < deadline
            {
                thread::sleep(Duration::from_millis(5));
            }
            // A known fixture child must not be orphaned if an assertion fails.
            if probe_live(&self.identity) == IdentityStatus::VerifiedMatch {
                unsafe {
                    libc::kill(self.identity.pid as i32, libc::SIGTERM);
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
pub enum Fault {
    Json,
    Oversized,
    Shape,
    Correlation,
    Protocol,
    Selection,
    Stall,
}
impl Fault {
    pub fn name(self) -> &'static str {
        match self {
            Self::Json => "invalid-json",
            Self::Oversized => "oversized",
            Self::Shape => "shape",
            Self::Correlation => "correlation",
            Self::Protocol => "protocol",
            Self::Selection => "selection",
            Self::Stall => "stall",
        }
    }
    pub fn expected_code(self) -> ErrorCode {
        match self {
            Self::Protocol => ErrorCode::Protocol,
            Self::Stall => ErrorCode::Uncertain,
            _ => ErrorCode::InvalidReply,
        }
    }
}

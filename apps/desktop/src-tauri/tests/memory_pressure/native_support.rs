//! Owned processes, independent execution barriers and bounded read-only queries.
use super::{execution, native, router::Router};
use arc_core::process_identity::{probe_live, IdentityStatus, ProcessIdentity};
use arc_desktop::bridge::DesktopBridge;
use arc_ipc::{Request, Response};
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    os::unix::{
        fs::symlink,
        net::{UnixListener, UnixStream},
    },
    path::PathBuf,
    process::{Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

pub fn binary(name: &str) -> PathBuf {
    let exe = std::env::current_exe().unwrap();
    let path = exe.parent().unwrap().parent().unwrap().join(name);
    assert!(
        path.is_file(),
        "build workspace bins in the current Cargo target: {}",
        path.display()
    );
    path
}
pub fn eventually(mut condition: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(35);
    while !condition() {
        assert!(
            Instant::now() < end,
            "pressure acceptance synchronization timed out"
        );
        thread::sleep(Duration::from_millis(10));
    }
}
pub fn save(name: &str, value: &Value) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap()
        .to_owned();
    let root = std::env::var_os("ARC_PRESSURE_EVIDENCE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target/memory-pressure/task-6/native"));
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join(format!("{name}.json")),
        serde_json::to_vec_pretty(value).unwrap(),
    )
    .unwrap();
}
pub struct Case {
    pub fixture: native::Fixture,
    pub router: Router,
    pub daemon: native::OwnedChild,
    injected: bool,
}
impl Case {
    pub fn new(injected: bool) -> Self {
        let fixture = native::Fixture::with_binary(binary(if injected {
            "arc-pressure-daemon-fixture"
        } else {
            "arc-daemon"
        }));
        fs::write(fixture.root.join("pressure"), b"Normal").unwrap();
        symlink(binary("arc-exec-fixture"), fixture.root.join("cargo")).unwrap();
        let mut command = fixture.command();
        command.env("ARC_PRESSURE_CONTROL", fixture.root.join("pressure"));
        let mut daemon = native::OwnedChild::new(command.spawn().unwrap());
        let bridge = DesktopBridge::from_socket_path(fixture.socket.clone());
        let mut reported = false;
        eventually(|| {
            let health = bridge.daemon_health();
            if health.is_err() && fixture.socket.exists() && !reported {
                eprintln!(
                    "fixture health={health:?}, stdout={}, stderr={}",
                    fs::read_to_string(fixture.root.join("stdout")).unwrap_or_default(),
                    fs::read_to_string(fixture.root.join("stderr")).unwrap_or_default()
                );
                reported = true;
            }
            assert!(
                daemon.child.try_wait().unwrap().is_none(),
                "daemon startup failed: stdout={} stderr={} health={health:?}",
                fs::read_to_string(fixture.root.join("stdout")).unwrap_or_default(),
                fs::read_to_string(fixture.root.join("stderr")).unwrap_or_default()
            );
            health.is_ok()
        });
        let router = Router::new(bridge);
        assert_eq!(
            router.call("daemon_health", json!({})).unwrap()["data"]["protocol_version"],
            1
        );
        Self {
            fixture,
            router,
            daemon,
            injected,
        }
    }
    pub fn rpc(&self, operation: &str, payload: Value) -> Value {
        let response = self.response(operation, payload.clone());
        assert_eq!(
            response.status,
            "ok",
            "operation={operation} payload={payload} response={}",
            serde_json::to_string(&response).unwrap()
        );
        response.result.unwrap()
    }
    pub fn response(&self, operation: &str, payload: Value) -> Response {
        let mut request = Request::new("pressure-acceptance", operation);
        request.payload = payload;
        let response: Response = arc_ipc::exchange(&mut self.fixture.connect(), &request).unwrap();
        assert_eq!(response.request_id, request.request_id);
        response
    }
    pub fn current(&self) -> Value {
        self.rpc("policy.current", Value::Null)
    }
    pub fn publish_pressure(&self, level: &str) {
        assert!(self.injected);
        let temp = self.fixture.root.join("pressure-next");
        fs::write(&temp, level).unwrap();
        fs::rename(temp, self.fixture.root.join("pressure")).unwrap();
    }
    pub fn pressure(&self, level: &str, previous: u64) -> Value {
        self.publish_pressure(level);
        let expected = match level {
            "Normal" => arc_core::meter::MemoryPressure::Normal,
            "Warning" => arc_core::meter::MemoryPressure::Warning,
            "Critical" => arc_core::meter::MemoryPressure::Critical,
            _ => panic!("unknown injected pressure level"),
        };
        let expected = json!(expected);
        let mut accepted = None;
        eventually(|| {
            let value = self.current();
            let observation = &value["observation"];
            if observation["pressure"] == expected
                && observation["availability"] == "available"
                && observation["sequence"]
                    .as_u64()
                    .is_some_and(|n| n > previous)
            {
                assert!(
                    observation["age_ms"].as_u64().unwrap()
                        <= observation["stale_after_ms"].as_u64().unwrap()
                );
                assert_eq!(
                    observation["sampler"]["daemon_instance"]["pid"],
                    self.daemon.child.id()
                );
                assert!(observation["sampler"]["generation"].as_u64().unwrap() > 0);
                assert!(observation["sampler"]["monotonic_epoch"].as_u64().unwrap() > 0);
                accepted = Some(value);
                true
            } else {
                false
            }
        });
        accepted.unwrap()
    }
    pub fn rows(&self, sql: &str) -> Vec<Value> {
        let output = Command::new("/usr/bin/sqlite3")
            .args(["-readonly", "-json"])
            .arg(&self.fixture.database)
            .arg(sql)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        assert!(output.stderr.is_empty());
        if output.stdout.is_empty() {
            vec![]
        } else {
            serde_json::from_slice(&output.stdout).unwrap()
        }
    }
    pub fn job(&self, pid: u32) -> String {
        let mut id = None;
        eventually(|| {
            id = self
                .rows("SELECT job_id,observer_pid FROM job_lifecycle")
                .into_iter()
                .find(|row| row["observer_pid"] == pid)
                .and_then(|row| row["job_id"].as_str().map(str::to_owned));
            id.is_some()
        });
        id.unwrap()
    }
    pub fn cli(&self, args: &[&str]) -> std::process::Output {
        Command::new(binary("arcctl"))
            .args(args)
            .env(arc_core::ARC_SOCKET_PATH, &self.fixture.socket)
            .env(arc_core::ARC_DB_PATH, &self.fixture.database)
            .output()
            .unwrap()
    }
    pub fn stop(&mut self) {
        self.daemon.stop(libc::SIGTERM);
        let report: Value =
            serde_json::from_slice(&fs::read(self.fixture.root.join("runtime.json")).unwrap())
                .unwrap();
        native::assert_report(&report);
    }
}
pub struct Run {
    pub wrapper: native::OwnedChild,
    pub listener: UnixListener,
    pub marker: PathBuf,
    pub stream: Option<UnixStream>,
    pub handshake: Option<Value>,
    pub stdout: PathBuf,
    pub stderr: PathBuf,
}
impl Run {
    pub fn start(case: &Case, name: &str, subcommand: &str) -> Self {
        Self::start_with_barrier(case, name, subcommand, None)
    }
    pub fn start_with_barrier(
        case: &Case,
        name: &str,
        subcommand: &str,
        barrier: Option<(&str, &std::path::Path)>,
    ) -> Self {
        let root = &case.fixture.root;
        let socket = root.join(format!("{name}.sock"));
        let marker = root.join(format!("{name}.starts"));
        let stdout = root.join(format!("{name}.out"));
        let stderr = root.join(format!("{name}.err"));
        let listener = UnixListener::bind(&socket).unwrap();
        let mut command = Command::new(binary("arcctl"));
        command
            .args(["exec", "--", "cargo", subcommand])
            .env(arc_core::ARC_SOCKET_PATH, &case.fixture.socket)
            .env(arc_core::ARC_DB_PATH, &case.fixture.database)
            .env("PATH", format!("{}:/usr/bin:/bin", root.display()))
            .env_remove("ARC_TRAFFIC_LINEAGE")
            .env("ARC_FIXTURE_MODE", "exit17")
            .env("ARC_FIXTURE_CONTROL", socket)
            .env("ARC_FIXTURE_MARKER", &marker)
            .current_dir(root)
            .stdin(Stdio::null())
            .stdout(fs::File::create(&stdout).unwrap())
            .stderr(fs::File::create(&stderr).unwrap())
            .env_remove("ARC_PRESSURE_ACCEPTANCE_STAGE")
            .env_remove("ARC_PRESSURE_ACCEPTANCE_BARRIER");
        if let Some((stage, socket)) = barrier {
            command
                .env("ARC_PRESSURE_ACCEPTANCE_STAGE", stage)
                .env("ARC_PRESSURE_ACCEPTANCE_BARRIER", socket);
        }
        let wrapper = native::OwnedChild::new(command.spawn().unwrap());
        Self {
            wrapper,
            listener,
            marker,
            stream: None,
            handshake: None,
            stdout,
            stderr,
        }
    }
    pub fn count(&self) -> usize {
        fs::read_to_string(&self.marker)
            .unwrap_or_default()
            .lines()
            .count()
    }
    pub fn started(&mut self) -> ProcessIdentity {
        let (stream, handshake) = execution::accept(&self.listener);
        let identity: ProcessIdentity =
            serde_json::from_value(handshake["identity"].clone()).unwrap();
        assert_eq!(probe_live(&identity), IdentityStatus::VerifiedMatch);
        assert_eq!(handshake["ppid"], self.wrapper.child.id());
        assert_eq!(self.count(), 1);
        self.stream = Some(stream);
        self.handshake = Some(handshake);
        identity
    }
    pub fn finish(&mut self) -> ExitStatus {
        self.stream.as_mut().unwrap().write_all(b"x").unwrap();
        let status = execution::wait(&mut self.wrapper.child);
        assert_eq!(
            status.code(),
            Some(17),
            "{} {}",
            fs::read_to_string(&self.stdout).unwrap(),
            fs::read_to_string(&self.stderr).unwrap()
        );
        assert_eq!(self.count(), 1);
        status
    }
}

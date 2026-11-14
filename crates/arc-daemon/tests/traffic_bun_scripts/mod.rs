//! Shared fixture implementation; the desktop target supplies its registered router.
use crate::{cleanup, native};
use arc_core::process_identity::{probe_live, IdentityStatus, ProcessIdentity};
use serde_json::{json, Value};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    os::unix::{
        ffi::OsStrExt,
        fs::{symlink, OpenOptionsExt, PermissionsExt},
    },
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};
pub mod compatibility;
pub mod contention;
pub mod outputs;
pub mod preservation;
pub mod projects;
pub mod protocol;
pub mod recovery;
pub mod repository;
pub mod semantics;

pub type Read = fn(&Path, &str, Value) -> Value;
pub fn root() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    while !path.join("Cargo.lock").is_file() {
        assert!(path.pop());
    }
    path
}
pub fn binary(name: &str) -> PathBuf {
    let exe = std::env::current_exe().unwrap();
    let path = exe.parent().unwrap().parent().unwrap().join(name);
    assert!(
        path.is_file(),
        "required workspace binary: {}",
        path.display()
    );
    path
}
#[track_caller]
pub fn eventually(mut f: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(60);
    while !f() {
        assert!(
            Instant::now() < end,
            "native Bun-script synchronization timed out"
        );
        thread::sleep(Duration::from_millis(10));
    }
}
pub fn exchange(socket: &Path, operation: &str, payload: Value) -> Value {
    let mut request = arc_ipc::Request::new("bun-script-native", operation);
    request.payload = payload;
    let response = arc_ipc::exchange(&mut native::connect(socket), &request).unwrap();
    assert_eq!(response.status, "ok", "{response:?}");
    assert_eq!(response.request_id, request.request_id);
    assert_eq!(response.protocol_version, 1);
    response.result.unwrap()
}
pub fn direct_read(socket: &Path, command: &str, args: Value) -> Value {
    let operation = match command {
        "traffic_job_diagnostics" => {
            assert_eq!(
                exchange(socket, "traffic.diagnostics.capabilities", Value::Null),
                json!({"capability":"traffic.diagnostics.v1"})
            );
            "traffic.diagnostics.job"
        }
        "meter_summary" => "meter.summary",
        _ => panic!("unsupported fixture read"),
    };
    exchange(socket, operation, args)
}
pub fn rows(database: &Path, sql: &str) -> Vec<Value> {
    let output = Command::new("/usr/bin/sqlite3")
        .args(["-readonly", "-json", "-cmd", ".timeout 2000"])
        .arg(database)
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
pub fn sha(path: &Path) -> String {
    let out = Command::new("/usr/bin/shasum")
        .args(["-a", "256"])
        .arg(path)
        .output()
        .unwrap();
    assert!(out.status.success());
    String::from_utf8(out.stdout)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned()
}
pub fn executable(pid: u32) -> PathBuf {
    let mut buffer = [0_u8; 4096];
    // SAFETY: initialized output buffer, length passed in bytes.
    let n =
        unsafe { libc::proc_pidpath(pid as i32, buffer.as_mut_ptr().cast(), buffer.len() as u32) };
    assert!(n > 0, "native executable identity unavailable for {pid}");
    fs::canonicalize(std::ffi::OsStr::from_bytes(&buffer[..n as usize])).unwrap()
}
pub fn log(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_default()
}
pub fn write(path: &Path, data: impl AsRef<[u8]>) {
    fs::write(path, data).unwrap();
}
pub struct Case {
    // Processes are dropped before the fixture removes its own temporary directory.
    pub daemon: native::OwnedChild,
    pub fixture: native::Fixture,
    pub artifacts: PathBuf,
    pub read: Read,
    pub tools: projects::Tools,
    pub injected: bool,
}
impl Case {
    pub fn new(name: &str, read: Read, injected: bool) -> Self {
        assert_eq!(std::env::consts::OS, "macos");
        let tools = projects::Tools::installed();
        let fixture = native::Fixture::with_binary(binary(if injected {
            "arc-pressure-daemon-fixture"
        } else {
            "arc-daemon"
        }));
        write(&fixture.root.join("pressure"), "Normal");
        let artifacts = std::env::var_os("ARC_TRAFFIC_SCRIPTS_RUN_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| root().join("target/traffic-bun-scripts/native"))
            .join(format!(
                "{}-{name}-{}",
                if std::env::var("CARGO_PKG_NAME").as_deref() == Ok("arc-desktop") {
                    "desktop"
                } else {
                    "daemon"
                },
                fixture.root.file_name().unwrap().to_str().unwrap()
            ));
        fs::create_dir_all(&artifacts).unwrap();
        let mut cmd = fixture.command();
        cmd.env("ARC_PRESSURE_CONTROL", fixture.root.join("pressure"));
        let mut daemon = native::OwnedChild::new(cmd.spawn().unwrap());
        fixture.wait_for(&mut daemon, "arc-daemon ready.");
        let case = Self {
            daemon,
            fixture,
            artifacts,
            read,
            tools,
            injected,
        };
        let daemon_executable = executable(case.daemon.child.id());
        assert_eq!(
            daemon_executable,
            binary(if injected {
                "arc-pressure-daemon-fixture"
            } else {
                "arc-daemon"
            })
            .canonicalize()
            .unwrap()
        );
        let shims: Vec<_> = ["bun", "cargo", "vite", "tsc", "biome"]
            .into_iter()
            .map(|name| {
                let path = case.tools.shims.join(name);
                json!({"name":name,"sha256":sha(&path)})
            })
            .collect();
        case.save("startup", json!({"daemon":ProcessIdentity::capture(case.daemon.child.id()).unwrap(),"binary":daemon_executable,"sha256":sha(&daemon_executable),"pressureInjected":injected,"tools":case.tools.receipt(),"execObservers":shims}));
        case
    }
    pub fn save(&self, name: &str, value: Value) {
        write(
            &self.artifacts.join(format!("{name}.json")),
            serde_json::to_vec(&value).unwrap(),
        );
        println!(
            "BUN_SCRIPT_OBSERVATION {}",
            json!({"case":name,"directory":self.artifacts,"observations":value})
        );
    }
    pub fn rows(&self, sql: &str) -> Vec<Value> {
        rows(&self.fixture.database, sql)
    }
    pub fn rpc(&self, operation: &str, input: Value) -> Value {
        exchange(&self.fixture.socket, operation, input)
    }
    pub fn cli(&self, args: &[&str]) -> std::process::Output {
        Command::new(binary("arcctl"))
            .args(args)
            .env(arc_core::ARC_SOCKET_PATH, &self.fixture.socket)
            .env(arc_core::ARC_DB_PATH, &self.fixture.database)
            .output()
            .unwrap()
    }
    pub fn job(&self, pid: u32) -> String {
        let mut id = None;
        eventually(|| {
            id = self
                .rows("SELECT job_id,observer_pid FROM job_lifecycle")
                .into_iter()
                .find(|r| r["observer_pid"] == pid)
                .and_then(|r| r["job_id"].as_str().map(str::to_owned));
            id.is_some()
        });
        id.unwrap()
    }
    pub fn row(&self, id: &str) -> Value {
        self.rows("SELECT j.id,j.job_class,j.exit_code,l.classification_reason,l.child_pid,l.observer_pid,l.outcome_kind,l.signal,l.revision,t.state,t.generation,t.root_job_id,t.queued_at_ms,t.granted_at_ms,p.job_id AS permit_job,p.state AS permit_state,p.generation AS permit_generation,p.cpu_millis_est,p.memory_bytes_est,p.released_at FROM jobs j JOIN job_lifecycle l ON l.job_id=j.id LEFT JOIN traffic_entries t ON t.job_id=j.id LEFT JOIN permits p ON p.job_id=coalesce(t.root_job_id,t.job_id)")
            .into_iter().find(|r| r["id"] == id).unwrap()
    }
    pub fn active(&self) -> Value {
        self.rows("SELECT count(*) AS permits,coalesce(sum(cpu_millis_est),0) AS cpu,coalesce(sum(memory_bytes_est),0) AS memory FROM permits WHERE state='ACTIVE'").remove(0)
    }
    pub fn reconcile(&self, id: &str, class: &str, reason: &str) -> Value {
        let cli = self.cli(&["job", "diagnostics", id, "--json"]);
        assert!(cli.status.success(), "{cli:?}");
        let cli: Value = serde_json::from_slice(&cli.stdout).unwrap();
        let bridge = (self.read)(
            &self.fixture.socket,
            "traffic_job_diagnostics",
            json!({"job_id":id}),
        );
        let mut a = cli.clone();
        let mut b = bridge.clone();
        a.as_object_mut().unwrap().remove("recorded_at_ms");
        b.as_object_mut().unwrap().remove("recorded_at_ms");
        assert_eq!(
            a, b,
            "independent CLI and registered/read transport must agree"
        );
        assert_eq!(cli["liveness_fresh"], false);
        let row = self.row(id);
        assert_eq!(row["job_class"], class);
        assert_eq!(row["classification_reason"], reason);
        assert_eq!(cli["job_id"], id);
        assert_eq!(cli["classification"]["classification"], class);
        assert_eq!(cli["classification"]["reason"], reason);
        let admission = &cli["admission"];
        if !row["state"].is_null() && row["state"] != "RESERVED" {
            assert_eq!(admission["state"], row["state"]);
        }
        assert_eq!(admission["permit_active"], row["permit_state"] == "ACTIVE");
        if !row["permit_generation"].is_null() {
            assert_eq!(admission["permit_generation"], row["permit_generation"]);
            assert_eq!(admission["root_job_id"], row["permit_job"]);
            assert_eq!(
                admission["reservation"]["cpu_millis"],
                row["cpu_millis_est"]
            );
            assert_eq!(
                admission["reservation"]["memory_bytes"],
                row["memory_bytes_est"]
            );
        }
        if row["outcome_kind"] == "EXITED" {
            assert_eq!(
                admission["outcome"],
                json!({"kind":"EXITED","code":row["exit_code"]})
            );
        }
        if row["outcome_kind"] == "SIGNALED" {
            assert_eq!(admission["outcome"]["signal"], row["signal"]);
        }
        let status = self.cli(&["job", "status", id]);
        assert!(status.status.success());
        assert!(String::from_utf8_lossy(&status.stdout).contains(id));
        json!({"sqlite":row,"diagnostic":cli,"registeredOrDirectRead":bridge,"status":String::from_utf8(status.stdout).unwrap()})
    }
    pub fn terminal(&self, run: &Run, class: &str, reason: &str, code: i32) -> Value {
        let id = self.job(run.wrapper.child.id());
        eventually(|| self.row(&id)["state"] == "COMPLETED");
        let result = self.reconcile(&id, class, reason);
        assert_eq!(result["sqlite"]["exit_code"], code);
        assert_eq!(result["diagnostic"]["admission"]["permit_active"], false);
        let meter = self.cli(&["meter", "summary", "--job", &id, "--json"]);
        assert!(meter.status.success(), "{meter:?}");
        let meter: Value = serde_json::from_slice(&meter.stdout).unwrap();
        let bridge = (self.read)(&self.fixture.socket, "meter_summary", json!({"job_id":id}));
        assert_eq!(meter, bridge);
        let report = &meter["accounting"]["report"];
        assert!(
            report.is_object(),
            "wait4 accounting must survive script admission: {meter}"
        );
        assert_eq!(report["outcome"], json!({"kind":"EXITED","code":code}));
        assert!(report["cpu_core_seconds"]["total"]
            .as_f64()
            .is_some_and(|n| n >= 0.0));
        json!({"job":result,"meter":meter})
    }
    pub fn done(mut self) {
        assert_eq!(self.active()["permits"], 0);
        self.daemon.stop(libc::SIGTERM);
        let report: Value =
            serde_json::from_slice(&fs::read(self.fixture.root.join("runtime.json")).unwrap())
                .unwrap();
        native::assert_report(&report);
        self.save("shutdown", report);
    }
}
pub struct Run {
    pub wrapper: native::OwnedChild,
    pub identity: ProcessIdentity,
    pub project: PathBuf,
    pub fifo: Option<File>,
}
impl Run {
    pub fn start(case: &Case, project: &Path, argv: &[&str]) -> Self {
        Self::start_with(case, project, argv, |_| {})
    }
    pub fn start_with(
        case: &Case,
        project: &Path,
        argv: &[&str],
        configure: impl FnOnce(&mut Command),
    ) -> Self {
        let mut cmd = Command::new(binary("arcctl"));
        cmd.args(["exec", "--"])
            .args(argv)
            .current_dir(project)
            .env(arc_core::ARC_SOCKET_PATH, &case.fixture.socket)
            .env(arc_core::ARC_DB_PATH, &case.fixture.database)
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    case.tools.shims.display(),
                    std::env::var("PATH").unwrap()
                ),
            )
            .env("TBS_PROJECT", project)
            .env("TBS_BODY", project.join("bodies"))
            .env("TBS_RELEASE", project.join("release"))
            .env("TBS_ARCCTL", binary("arcctl"))
            .env("CARGO_TARGET_DIR", project.join("target"))
            .env_remove("ARC_TRAFFIC_LINEAGE")
            .env_remove("ARC_PRESSURE_ACCEPTANCE_STAGE")
            .env_remove("ARC_PRESSURE_ACCEPTANCE_BARRIER")
            .stdin(Stdio::null())
            .stdout(File::create(project.join("stdout")).unwrap())
            .stderr(File::create(project.join("stderr")).unwrap());
        configure(&mut cmd);
        let wrapper = native::OwnedChild::new(cmd.spawn().unwrap());
        let identity = ProcessIdentity::capture(wrapper.child.id()).unwrap();
        Self {
            wrapper,
            identity,
            project: project.to_owned(),
            fifo: None,
        }
    }
    pub fn count(&self, tool: &str) -> usize {
        log(&self.project.join("launches"))
            .lines()
            .filter(|l| l.split_whitespace().next() == Some(tool))
            .count()
    }
    pub fn entered(&mut self, case: &Case, fifo: bool) -> Value {
        if fifo {
            eventually(|| {
                assert!(
                    self.wrapper.child.try_wait().unwrap().is_none(),
                    "tsc exited before reading fixture: {}",
                    log(&self.project.join("stderr"))
                );
                match OpenOptions::new()
                    .write(true)
                    .custom_flags(libc::O_NONBLOCK)
                    .open(self.project.join("gate.ts"))
                {
                    Ok(file) => {
                        self.fifo = Some(file);
                        true
                    }
                    Err(e) if e.raw_os_error() == Some(libc::ENXIO) => false,
                    Err(e) => panic!("typecheck FIFO writer: {e}"),
                }
            });
        } else {
            eventually(|| {
                assert!(
                    self.wrapper.child.try_wait().unwrap().is_none(),
                    "tool exited before barrier: {}",
                    log(&self.project.join("stderr"))
                );
                !log(&self.project.join("bodies")).is_empty()
            });
        }
        let id = case.job(self.wrapper.child.id());
        eventually(|| case.row(&id)["state"] == "RUNNING");
        let row = case.row(&id);
        let tool = ProcessIdentity::capture(row["child_pid"].as_u64().unwrap() as u32).unwrap();
        assert_eq!(probe_live(&tool), IdentityStatus::VerifiedMatch);
        assert_eq!(self.count("bun") + self.count("cargo"), 1);
        if fifo {
            assert_eq!(self.count("tsc"), 1, "one real compiler entry");
        }
        let launches = log(&self.project.join("launches"));
        let processes: Vec<_> = launches.lines().map(|line| {
            let fields: Vec<_> = line.split_whitespace().collect();
            assert_eq!(fields.len(), 3);
            let identity = ProcessIdentity::capture(fields[1].parse().unwrap()).unwrap();
            assert_eq!(probe_live(&identity), IdentityStatus::VerifiedMatch);
            let executable = executable(identity.pid);
            json!({"entry":fields[0],"identity":identity,"parentAtExec":fields[2],"executable":executable,"sha256":sha(&executable)})
        }).collect();
        json!({"jobId":id,"wrapper":self.identity,"child":tool,"executable":executable(tool.pid),"nativeProcesses":processes,"fifoSourceRead":fifo,"launches":launches,"bodies":log(&self.project.join("bodies"))})
    }
    pub fn release(&mut self) {
        if let Some(mut fifo) = self.fifo.take() {
            fifo.write_all(b"export const gate: number = 42;\n")
                .unwrap();
        }
        write(&self.project.join("release"), "release");
    }
    pub fn wait(&mut self) -> ExitStatus {
        eventually(|| self.wrapper.child.try_wait().unwrap().is_some());
        self.wrapper.child.wait().unwrap()
    }
    pub fn finish(&mut self, code: i32) {
        self.release();
        assert_eq!(
            self.wait().code(),
            Some(code),
            "{}\n{}",
            log(&self.project.join("stdout")),
            log(&self.project.join("stderr"))
        );
    }
}
impl Drop for Run {
    fn drop(&mut self) {
        if probe_live(&self.identity) == IdentityStatus::VerifiedMatch {
            cleanup::report_errors(cleanup::kill_tree(&self.identity, 0));
        }
    }
}

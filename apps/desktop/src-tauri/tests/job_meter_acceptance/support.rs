use super::{daemon_bridge_support::RealDaemon, execution, meter_router::Router, native};
use arc_core::process_identity::{probe_live, IdentityStatus, ProcessIdentity};
use arc_ipc::{Request, Response};
use serde_json::{json, Value};
use std::{
    fs,
    os::unix::fs::symlink,
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Stdio},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub fn workspace() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap()
}
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
pub fn eventually(mut condition: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(10);
    while !condition() {
        assert!(
            Instant::now() < end,
            "native acceptance observation timed out"
        );
        thread::sleep(Duration::from_millis(5));
    }
}
pub fn save(name: &str, value: Value) {
    let dir = std::env::var_os("ARC_JOB_METER_EVIDENCE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            workspace()
                .join("target/job-meter/task8")
                .join(std::process::id().to_string())
        });
    fs::create_dir_all(&dir).unwrap();
    let value = json!({"test":name,"observedAtMs":now(),"testProcess":ProcessIdentity::capture(std::process::id()).unwrap(),"observed":value});
    fs::write(
        dir.join(format!("{name}.json")),
        serde_json::to_vec_pretty(&value).unwrap(),
    )
    .unwrap();
    eprintln!("job-meter-case {name}: measured receipt saved");
}
pub fn rpc(case: &RealDaemon, operation: &str, payload: Value) -> Response {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let mut request = Request::new(
        format!("jm-{}", NEXT.fetch_add(1, Ordering::Relaxed)),
        operation,
    );
    request.payload = payload;
    let response = arc_ipc::exchange(&mut case.fixture.connect(), &request).unwrap();
    assert_eq!(response.request_id, request.request_id);
    assert_eq!(response.protocol_version, 1);
    response
}
pub fn ok(case: &RealDaemon, operation: &str, payload: Value) -> Value {
    let r = rpc(case, operation, payload);
    assert_eq!(r.status, "ok", "{} {r:?} error={:?}", operation, r.error);
    r.result.unwrap()
}
pub fn rows(case: &RealDaemon, sql: &str) -> Vec<Value> {
    let result = Command::new("/usr/bin/sqlite3")
        .args(["-readonly", "-json"])
        .arg(&case.fixture.database)
        .arg(sql)
        .output()
        .unwrap();
    assert!(result.status.success(), "{result:?}");
    assert!(result.stderr.is_empty());
    if result.stdout.is_empty() {
        vec![]
    } else {
        serde_json::from_slice(&result.stdout).unwrap()
    }
}
pub fn job_id(case: &RealDaemon, observer: u32) -> String {
    rows(case, "SELECT job_id,observer_pid FROM job_lifecycle")
        .into_iter()
        .find(|r| r["observer_pid"] == observer)
        .unwrap()["job_id"]
        .as_str()
        .unwrap()
        .into()
}
pub fn summary(case: &RealDaemon, id: &str) -> Value {
    ok(case, "meter.summary", json!({"job_id":id}))
}
pub fn report(summary: &Value) -> &Value {
    assert_eq!(summary["accounting"]["state"], "recorded", "{summary}");
    &summary["accounting"]["report"]
}
pub fn reconcile(case: &RealDaemon, id: &str) -> Value {
    let sqlite = rows(
        case,
        "SELECT job_id,report_json,terminal_at_ms,relation,root_job_id FROM job_meter_summaries",
    )
    .into_iter()
    .find(|r| r["job_id"] == id)
    .unwrap();
    assert!(
        sqlite["report_json"].is_string(),
        "no persisted report: {sqlite}; binding={:?}; lifecycle={:?}",
        rows(case, "SELECT * FROM job_meter_summaries"),
        rows(case, "SELECT * FROM job_lifecycle")
    );
    let persisted: Value = serde_json::from_str(sqlite["report_json"].as_str().unwrap()).unwrap();
    let cli = Command::new(case.fixture.binary("arcctl"))
        .args(["meter", "summary", "--job", id, "--json"])
        .env(arc_core::ARC_SOCKET_PATH, &case.fixture.socket)
        .output()
        .unwrap();
    assert!(cli.status.success(), "{cli:?}");
    assert!(cli.stderr.is_empty());
    let cli: Value = serde_json::from_slice(&cli.stdout).unwrap();
    let desktop = Router::new(case.bridge.clone())
        .call("meter_summary", json!({"job_id":id}))
        .unwrap();
    assert_eq!(cli["job_id"], id);
    assert_eq!(desktop["data"], cli);
    assert_eq!(report(&cli), &persisted);
    assert_eq!(
        rows(case, "PRAGMA integrity_check")[0]["integrity_check"],
        "ok"
    );
    json!({"jobId":id,"sqliteReadOnly":sqlite,"sqliteReport":persisted,"cli":cli,"desktop":desktop,
        "command":["arcctl","meter","summary","--job",id,"--json"],
        "desktopCommand":"meter_summary (configure_with_ports production registration; real authenticated daemon transport)",
        "sqliteCliDesktopAgree":true})
}
pub fn setup(name: &str) -> RealDaemon {
    let case = RealDaemon::new(name);
    symlink(
        case.fixture.binary("arc-exec-fixture"),
        case.fixture.root.join("cargo"),
    )
    .unwrap();
    case
}
pub fn command(case: &RealDaemon, scheduled: bool, mode: &str) -> Command {
    let mut c = Command::new(case.fixture.binary("arcctl"));
    c.args(["exec", "--"]);
    if scheduled {
        c.args(["cargo", "build"]);
    } else {
        c.arg(case.fixture.binary("arc-exec-fixture"));
    }
    c.env(arc_core::ARC_SOCKET_PATH, &case.fixture.socket)
        .env(arc_core::ARC_DB_PATH, &case.fixture.database)
        .env(
            "PATH",
            format!("{}:/usr/bin:/bin", case.fixture.root.display()),
        )
        .env("ARC_FIXTURE_ACCOUNTING", mode)
        .env("ARC_METER_TEST_CLI", case.fixture.binary("arcctl"))
        .env_remove("ARC_TRAFFIC_LINEAGE")
        .current_dir(&case.fixture.root)
        .stdin(Stdio::null());
    c
}
pub struct Run {
    pub wrapper: native::OwnedChild,
    pub identity: ProcessIdentity,
    pub marker: PathBuf,
    stdout: PathBuf,
    stderr: PathBuf,
}
impl Run {
    pub fn launch(case: &RealDaemon, mut command: Command) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let base = case
            .fixture
            .root
            .join(format!("run-{}", NEXT.fetch_add(1, Ordering::Relaxed)));
        let marker = base.with_extension("marker");
        let stdout = base.with_extension("stdout");
        let stderr = base.with_extension("stderr");
        command
            .env("ARC_METER_TEST_MARKER", &marker)
            .stdout(fs::File::create(&stdout).unwrap())
            .stderr(fs::File::create(&stderr).unwrap());
        let wrapper = native::OwnedChild::new(command.spawn().unwrap());
        let identity = ProcessIdentity::capture(wrapper.child.id()).unwrap();
        Self {
            wrapper,
            identity,
            marker,
            stdout,
            stderr,
        }
    }
    pub fn finish(&mut self) -> ExitStatus {
        execution::wait(&mut self.wrapper.child)
    }
    pub fn count(&self) -> usize {
        fs::read_to_string(&self.marker).unwrap().lines().count()
    }
    pub fn output(&self) -> String {
        fs::read_to_string(&self.stdout).unwrap()
    }
    pub fn errors(&self) -> String {
        fs::read_to_string(&self.stderr).unwrap()
    }
}
pub fn signal_owned(identity: &ProcessIdentity, signal: i32) {
    assert_eq!(probe_live(identity), IdentityStatus::VerifiedMatch);
    assert_eq!(unsafe { libc::kill(identity.pid as i32, signal) }, 0);
}

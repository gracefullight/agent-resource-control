use super::{daemon_bridge_support::RealDaemon, execution, meter_router::Router, native};
use arc_core::process_identity::{probe_live, IdentityStatus, ProcessIdentity};
use arc_ipc::{Request, Response};
use serde_json::{json, Value};
use std::{
    fs,
    os::unix::fs::symlink,
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Output, Stdio},
    sync::atomic::{AtomicU64, Ordering},
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
#[track_caller]
pub fn eventually(mut condition: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(60);
    while !condition() {
        assert!(
            Instant::now() < end,
            "meter fixture observation never became ready"
        );
        std::thread::yield_now();
    }
}
pub fn save(name: &str, value: Value) {
    let value = json!({"test":name,"observedAtMs":now(),"testProcess":ProcessIdentity::capture(std::process::id()).unwrap(),"observed":value});
    let destination = std::env::var_os("ARC_JOB_METER_EVIDENCE_DIR").map(PathBuf::from);
    if let Some(path) = save_observation(name, &value, destination.as_deref()) {
        eprintln!(
            "job-meter-case {name}: measured receipt saved at {}",
            path.display()
        );
    } else {
        eprintln!("job-meter-case {name}: {value}");
    }
}

fn save_observation(name: &str, value: &Value, destination: Option<&Path>) -> Option<PathBuf> {
    let dir = destination?;
    fs::create_dir_all(dir).unwrap();
    let path = dir.join(format!("{name}.json"));
    fs::write(&path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
    Some(path)
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
pub fn output(mut command: Command, observation: &str) -> Output {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = native::OwnedChild::new(
        command
            .spawn()
            .unwrap_or_else(|error| panic!("cannot start {observation}: {error}")),
    );
    child
        .wait_with_output()
        .unwrap_or_else(|error| panic!("{observation} did not complete: {error}"))
}

pub fn rows(case: &RealDaemon, sql: &str) -> Vec<Value> {
    let mut command = Command::new("/usr/bin/sqlite3");
    command
        .env_clear()
        .env("HOME", &case.fixture.root)
        .env("TMPDIR", &case.fixture.root)
        .args(["-init", "/dev/null", "-readonly", "-json"])
        .arg(&case.fixture.database)
        .arg(sql);
    let result = output(command, "SQLite reconciliation query");
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
    let mut command = Command::new(case.fixture.binary("arcctl"));
    command
        .env_clear()
        .env("HOME", &case.fixture.root)
        .env("TMPDIR", &case.fixture.root)
        .args(["meter", "summary", "--job", id, "--json"])
        .env(arc_core::ARC_SOCKET_PATH, &case.fixture.socket)
        .env(arc_core::ARC_DB_PATH, &case.fixture.database);
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile);
    }
    let cli = output(command, "arcctl meter summary");
    assert!(cli.status.success(), "{cli:?}");
    assert!(cli.stderr.is_empty());
    let mut cli: Value = serde_json::from_slice(&cli.stdout).unwrap();
    // The CLI adds the job's process tree beside the v1 summary (MTR-002).
    let tree = cli
        .as_object_mut()
        .unwrap()
        .remove(arc_ipc::meter::job_tree::SUMMARY_KEY)
        .expect("the daemon advertises meter.job-tree.v1");
    assert!(tree.is_object() || tree.is_null(), "{tree}");
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
    setup_case(RealDaemon::new(name))
}
pub fn setup_isolated(name: &str) -> RealDaemon {
    setup_case(RealDaemon::isolated(name))
}
pub fn setup_clocked(name: &str) -> RealDaemon {
    setup_case(RealDaemon::clocked(name))
}
fn setup_case(case: RealDaemon) -> RealDaemon {
    symlink(
        case.fixture.binary("arc-exec-fixture"),
        case.fixture.root.join("cargo"),
    )
    .unwrap();
    case
}
pub fn command(case: &RealDaemon, scheduled: bool, mode: &str) -> Command {
    let mut c = Command::new(case.fixture.binary("arcctl"));
    c.env_clear();
    c.args(["exec", "--"]);
    if scheduled {
        c.args(["cargo", "build"]);
    } else {
        c.arg(case.fixture.binary("arc-exec-fixture"));
    }
    c.env(arc_core::ARC_SOCKET_PATH, &case.fixture.socket)
        .env(arc_core::ARC_DB_PATH, &case.fixture.database)
        .env("HOME", &case.fixture.root)
        .env("TMPDIR", &case.fixture.root)
        .env(
            "PATH",
            format!("{}:/usr/bin:/bin", case.fixture.root.display()),
        )
        .env("ARC_FIXTURE_ACCOUNTING", mode)
        .env("ARC_METER_TEST_CLI", case.fixture.binary("arcctl"))
        .current_dir(&case.fixture.root)
        .stdin(Stdio::null());
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        c.env("LLVM_PROFILE_FILE", profile);
    }
    c
}

/// Use the production execution source with a test-owned clock. The caller
/// selects the daemon; reporting still uses SQLite and the desktop bridge.
pub fn clocked_command(case: &RealDaemon, scheduled: bool, mode: &str) -> (Command, PathBuf) {
    let base = command(case, scheduled, mode);
    let key = case.fixture.root.join("clock-key");
    let clock = case.fixture.root.join("clock");
    fs::write(&key, b"arc-fixture-clock-v1").unwrap();
    fs::write(&clock, b"0").unwrap();
    let mut c = Command::new(case.fixture.binary("arc-exec-fixture"));
    c.env_clear().arg("--clocked-fixture-exec");
    c.args(base.get_args().skip(2));
    for (name, value) in base.get_envs() {
        if let Some(value) = value {
            c.env(name, value);
        } else {
            c.env_remove(name);
        }
    }
    c.env("HOME", &case.fixture.root)
        .env("TMPDIR", &case.fixture.root)
        .env("ARC_FIXTURE_CLOCK_KEY", key)
        .env("ARC_FIXTURE_CLOCK", &clock)
        .current_dir(&case.fixture.root)
        .stdin(Stdio::null());
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        c.env("LLVM_PROFILE_FILE", profile);
    }
    (c, clock)
}

pub fn set_clock(path: &Path, seconds: u64) {
    let pending = path.with_extension("pending");
    fs::write(&pending, seconds.to_string()).unwrap();
    fs::rename(pending, path).unwrap();
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
        let owners = base.with_extension("owners");
        command
            .env("ARC_FIXTURE_OWNERS", &owners)
            .env("ARC_METER_TEST_MARKER", &marker)
            .stdout(fs::File::create(&stdout).unwrap())
            .stderr(fs::File::create(&stderr).unwrap());
        let wrapper = native::OwnedChild::spawn_with_descendants(command, owners).unwrap();
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

#[cfg(test)]
mod observation_tests {
    use super::*;

    #[test]
    fn observations_are_saved_only_when_a_destination_is_supplied() {
        let root = std::env::temp_dir().join(format!(
            "arc-job-meter-observation-{}",
            arc_core::process_identity::random_job_id().unwrap()
        ));
        fs::create_dir(&root).unwrap();
        let value = json!({"outcome":"recorded","count":1});
        assert!(save_observation("example", &value, None).is_none());
        assert!(fs::read_dir(&root).unwrap().next().is_none());
        let path = save_observation("example", &value, Some(&root)).unwrap();
        assert_eq!(path, root.join("example.json"));
        assert_eq!(
            serde_json::from_slice::<Value>(&fs::read(path).unwrap()).unwrap(),
            value
        );
        fs::remove_dir_all(root).unwrap();
    }
}

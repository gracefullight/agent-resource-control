//! Real offline Go executions exercise caller environments and nested admission.
use super::owned_tree::OwnedTree;
use super::*;
use crate::support::{next, reply, Socket};
use arc_ipc::{jobs::StartRequest, Response};
use std::{
    path::{Path, PathBuf},
    process::Output,
};

const GO_TEST: &str = r#"package lineage
import (
    "encoding/json"
    "fmt"
    "os"
    "os/exec"
    "sort"
    "testing"
    "time"
)
func register(t *testing.T) {
    t.Helper()
    cmd := exec.Command(os.Getenv("ARC_TEST_OWNER_REGISTER"), "--exact", "native_recovery::owned_tree_registration_tests::owned_tree_registration_helper", "--nocapture")
    cmd.Env = append(os.Environ(), "ARC_TEST_OWNER_HELPER=1")
    if output, err := cmd.CombinedOutput(); err != nil { t.Fatalf("ownership registration: %v: %s", err, output) }
}
func record(t *testing.T, label string) {
    t.Helper()
    register(t)
    value, present := os.LookupEnv("ARC_TRAFFIC_LINEAGE")
    env := os.Environ(); sort.Strings(env)
    cwd, err := os.Getwd(); if err != nil { t.Fatal(err) }
    f, err := os.OpenFile(os.Getenv("ARC_TEST_REPORT"), os.O_CREATE|os.O_APPEND|os.O_WRONLY, 0600)
    if err != nil { t.Fatal(err) }; defer f.Close()
    if err = json.NewEncoder(f).Encode(map[string]interface{}{"label":label,"present":present,"value":value,"environment":env,"cwd":cwd}); err != nil { t.Fatal(err) }
    pids, err := os.OpenFile(os.Getenv("ARC_TEST_PIDS"), os.O_CREATE|os.O_APPEND|os.O_WRONLY, 0600)
    if err != nil { t.Fatal(err) }; defer pids.Close()
    if _, err = fmt.Fprintln(pids, os.Getpid()); err != nil { t.Fatal(err) }
}
func TestProbe(t *testing.T) { record(t, "probe") }
func TestParent(t *testing.T) {
    record(t, "parent")
    cmd := exec.Command(os.Getenv("ARC_TEST_CLI"), "exec", "--", os.Getenv("ARC_TEST_GO"), "test", "-p", "2", "-count", "1", "-run", "^TestChild$", ".")
    if forged, ok := os.LookupEnv("ARC_TEST_FORGE"); ok { cmd.Env = append(os.Environ(), "ARC_TRAFFIC_LINEAGE="+forged) }
    out, err := cmd.CombinedOutput(); if err != nil { t.Fatalf("nested: %v: %s", err, out) }
}
func TestChild(t *testing.T) {
    record(t, "child")
    if os.Getenv("ARC_TEST_HOLD_READY") == "1" {
        if err := os.WriteFile(os.Getenv("ARC_TEST_EARLY"), []byte("held"), 0600); err != nil { t.Fatal(err) }
        for { if _, err := os.Stat(os.Getenv("ARC_TEST_RELEASE")); err == nil { break }; time.Sleep(10*time.Millisecond) }
    }
    if err := os.WriteFile(os.Getenv("ARC_TEST_READY"), []byte("ready"), 0600); err != nil { t.Fatal(err) }
    until := time.Now().Add(40*time.Second)
    for { if _, err := os.Stat(os.Getenv("ARC_TEST_RELEASE")); err == nil { break }; if time.Now().After(until) { t.Fatal("release timeout") }; time.Sleep(10*time.Millisecond) }
}
"#;

pub(super) struct Project {
    pub(super) root: PathBuf,
    go: PathBuf,
    cli: PathBuf,
    database: PathBuf,
}
impl Project {
    pub(super) fn new(root: &Path, database: PathBuf) -> Self {
        let project = root.join("lineage-project");
        fs::create_dir_all(&project).unwrap();
        for dir in ["build", "modules", "path", "tmp"] {
            fs::create_dir_all(project.join("cache").join(dir)).unwrap();
        }
        fs::write(
            project.join("go.mod"),
            "module example.org/lineage\n\ngo 1.20\n",
        )
        .unwrap();
        fs::write(project.join("lineage_test.go"), GO_TEST).unwrap();
        let mut probe = Command::new("go");
        probe
            .args(["env", "GOROOT"])
            .env("GOTOOLCHAIN", "local")
            .env("GOENV", "off")
            .env("GOPROXY", "off")
            .env(super::owned_tree::REGISTRY_ENV, root.join("go-env-owners"))
            .current_dir(&project)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let probe = OwnedTree::spawn(probe, root)
            .wait_with_output()
            .expect("installed Go toolchain is required");
        assert!(probe.status.success(), "{probe:?}");
        let go = PathBuf::from(String::from_utf8(probe.stdout).unwrap().trim()).join("bin/go");
        Self {
            root: project,
            go: fs::canonicalize(go).unwrap(),
            cli: PathBuf::from(env!("CARGO_BIN_EXE_arcctl")),
            database,
        }
    }
    fn report(&self) -> PathBuf {
        self.root.join("report.jsonl")
    }
    pub(super) fn command(
        &self,
        socket: &Path,
        managed: bool,
        selector: &str,
        lineage: Option<&str>,
    ) -> Command {
        let mut command = Command::new(if managed { &self.cli } else { &self.go });
        if managed {
            command.args(["exec", "--"]).arg(&self.go);
        }
        command
            .args(["test", "-p", "2", "-count", "1", "-run", selector, "."])
            .current_dir(&self.root)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env(arc_core::ARC_SOCKET_PATH, socket)
            .env(arc_core::ARC_DB_PATH, &self.database)
            .env("ARC_TEST_CLI", &self.cli)
            .env("ARC_TEST_GO", &self.go)
            .env("ARC_TEST_REPORT", self.report())
            .env("ARC_TEST_PIDS", self.root.join("body-pids"))
            .env("ARC_TEST_READY", self.root.join("ready"))
            .env("ARC_TEST_RELEASE", self.root.join("release"))
            .env("ARC_TEST_EARLY", self.root.join("early"))
            .env(
                super::owned_tree::REGISTRY_ENV,
                self.root
                    .join(format!("owners-{}", selector.trim_matches(['^', '$']))),
            )
            .env("ARC_TEST_SENTINEL", "literal $HOME ' two words 日本語")
            .env("GOCACHE", self.root.join("cache/build"))
            .env("GOMODCACHE", self.root.join("cache/modules"))
            .env("GOPATH", self.root.join("cache/path"))
            .env("GOTMPDIR", self.root.join("cache/tmp"))
            .env("GOPROXY", "off")
            .env("GOSUMDB", "off")
            .env("GOTOOLCHAIN", "local")
            .env("GOENV", "off")
            .env("GOWORK", "off")
            .env("GOFLAGS", "")
            .env("GOVCS", "*:off")
            .env("GOTELEMETRY", "off")
            .env("GOMAXPROCS", "2")
            .env("CGO_ENABLED", "0")
            .env_remove("GOROOT")
            .env_remove("GOCACHEPROG")
            .env_remove("GOEXPERIMENT")
            .env_remove("ARC_TEST_HOLD_READY")
            .env_remove("ARC_TEST_FORGE");
        if let Some(value) = lineage {
            command.env("ARC_TRAFFIC_LINEAGE", value);
        } else {
            command.env_remove("ARC_TRAFFIC_LINEAGE");
        }
        command
    }
    pub(super) fn records(&self) -> Vec<Value> {
        fs::read_to_string(self.report())
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }
    pub(super) fn reset(&self) {
        for name in ["report.jsonl", "body-pids", "ready", "release", "early"] {
            match fs::remove_file(self.root.join(name)) {
                Ok(()) => (),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                Err(e) => panic!("fixture cleanup: {e}"),
            }
        }
    }
}

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "status={}\nstdout={}\nstderr={}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
fn compare_probe(project: &Project, socket: &Path, lineage: Option<&str>) {
    project.reset();
    let direct = OwnedTree::spawn(
        project.command(socket, false, "^TestProbe$", lineage),
        project.root.parent().unwrap(),
    )
    .wait_with_output()
    .unwrap();
    assert_success(&direct);
    let expected = project.records();
    assert_eq!(expected.len(), 1);
    assert_eq!(expected[0]["present"], lineage.is_some());
    assert_eq!(expected[0]["value"], lineage.unwrap_or(""));
    project.reset();
    let managed = OwnedTree::spawn(
        project.command(socket, true, "^TestProbe$", lineage),
        project.root.parent().unwrap(),
    )
    .wait_with_output()
    .unwrap();
    assert_success(&managed);
    assert_eq!(
        project.records(),
        expected,
        "the complete child environment and cwd must match direct Go execution"
    );
    assert_eq!(managed.status.code(), direct.status.code());
}

#[test]
fn native_environment_real_go_preserves_lineage_and_nested_permit_and_legacy_fallback() {
    let _serial = serialization::serial();
    let mut daemon = Daemon::new();
    let project = Project::new(&daemon.fixture.root, daemon.fixture.database.clone());
    let supplied = "f".repeat(64);
    for lineage in [None, Some(""), Some(supplied.as_str())] {
        let old: Vec<_> = daemon
            .rows("SELECT job_id FROM traffic_entries")
            .into_iter()
            .map(|row| row["job_id"].clone())
            .collect();
        compare_probe(&project, &daemon.fixture.socket, lineage);
        let entries: Vec<_> = daemon.rows("SELECT t.job_id,t.state,l.classification_reason,j.job_class,p.state AS permit_state FROM traffic_entries t JOIN job_lifecycle l USING(job_id) JOIN jobs j ON j.id=t.job_id JOIN permits p USING(job_id)")
            .into_iter().filter(|row| !old.contains(&row["job_id"])).collect();
        assert_eq!(
            entries.len(),
            1,
            "one managed execution creates one scheduled job and permit"
        );
        assert_eq!(entries[0]["state"], "COMPLETED");
        assert_eq!(entries[0]["job_class"], "TEST");
        assert_eq!(entries[0]["classification_reason"], "GO_TEST");
        assert_eq!(entries[0]["permit_state"], "RELEASED");
        assert!(daemon
            .rows("SELECT state FROM permits")
            .iter()
            .all(|row| row["state"] == "RELEASED"));
        nested(&daemon, &project, lineage, None);
        let legacy = Socket::new();
        let listener = legacy.listener.try_clone().unwrap();
        let peer = thread::spawn(move || legacy_peer(&listener));
        compare_probe(&project, &legacy.root.join("s"), lineage);
        peer.join().unwrap();
    }
    let forged = "e".repeat(64);
    nested(&daemon, &project, Some(&supplied), Some(&forged));
    unrelated_claim_waits(&daemon, &project, &supplied);
    daemon.stop();
}

fn nested(daemon: &Daemon, project: &Project, lineage: Option<&str>, forged: Option<&str>) {
    project.reset();
    let prior_permits = daemon.rows("SELECT job_id FROM permits").len();
    let old: Vec<_> = daemon
        .rows("SELECT job_id FROM traffic_entries")
        .into_iter()
        .map(|row| row["job_id"].clone())
        .collect();
    let mut command = project.command(&daemon.fixture.socket, true, "^TestParent$", lineage);
    if let Some(value) = forged {
        command.env("ARC_TEST_FORGE", value);
    }
    let mut wrapper = OwnedTree::spawn(command, project.root.parent().unwrap());
    let mut current = Vec::new();
    eventually(|| {
        if !project.root.join("ready").exists() {
            return false;
        }
        current = daemon.rows("SELECT t.job_id,t.root_job_id,t.context,t.inherited_context,t.state,l.child_pid FROM traffic_entries t JOIN job_lifecycle l USING(job_id)")
            .into_iter().filter(|row| !old.contains(&row["job_id"])).collect();
        current.len() == 2
            && current
                .iter()
                .all(|row| row["state"] == "RUNNING" && row["child_pid"].is_number())
    });
    let nested = current
        .iter()
        .find(|row| row["root_job_id"].is_string() && row["root_job_id"] != row["job_id"])
        .expect("verified nested root");
    let root = current
        .iter()
        .find(|row| row["job_id"] == nested["root_job_id"])
        .expect("active root entry");
    let root_context = root["context"].as_str().expect("active root context");
    assert_eq!(
        nested["inherited_context"], root_context,
        "nested enqueue must carry the live sidecar context"
    );
    if let Some(forged) = forged {
        assert_ne!(
            root_context, forged,
            "the caller value must conflict with the active sidecar"
        );
        assert_ne!(nested["inherited_context"], forged);
    }
    assert_eq!(
        daemon.rows("SELECT state FROM permits WHERE state='ACTIVE'"),
        vec![json!({"state":"ACTIVE"})]
    );
    assert_eq!(
        daemon.rows("SELECT job_id FROM permits").len(),
        prior_permits + 1,
        "nested execution reuses the root permit"
    );
    let records = project.records();
    assert_eq!(
        records.len(),
        2,
        "parent and nested body execute exactly once"
    );
    assert_eq!(records[0]["label"], "parent");
    assert_eq!(records[1]["label"], "child");
    assert_eq!(records[0]["present"], lineage.is_some());
    assert_eq!(records[0]["value"], lineage.unwrap_or(""));
    assert_eq!(records[1]["present"], forged.or(lineage).is_some());
    assert_eq!(records[1]["value"], forged.or(lineage).unwrap_or(""));
    fs::write(project.root.join("release"), "release").unwrap();
    assert_success(&wrapper.wait_with_output().unwrap());
    assert_eq!(project.records().len(), 2);
    assert_eq!(
        daemon.rows("SELECT job_id FROM permits").len(),
        prior_permits + 1
    );
    assert!(daemon
        .rows("SELECT state FROM permits")
        .iter()
        .all(|row| row["state"] == "RELEASED"));
}

fn unrelated_claim_waits(daemon: &Daemon, project: &Project, claim: &str) {
    project.reset();
    let old: Vec<_> = daemon
        .rows("SELECT job_id FROM traffic_entries")
        .into_iter()
        .map(|row| row["job_id"].clone())
        .collect();
    let first = project.command(&daemon.fixture.socket, true, "^TestChild$", Some(claim));
    let mut holder = OwnedTree::spawn(first, project.root.parent().unwrap());
    let mut active = Vec::new();
    eventually(|| {
        active = daemon.rows("SELECT t.job_id,t.state,t.context,l.child_pid FROM traffic_entries t JOIN job_lifecycle l USING(job_id)")
            .into_iter().filter(|row| !old.contains(&row["job_id"])).collect();
        project.root.join("ready").exists()
            && active.len() == 1
            && active[0]["state"] == "RUNNING"
            && active[0]["child_pid"].is_number()
    });
    let active_context = active[0]["context"]
        .as_str()
        .expect("holder's actual active context");
    let second = project.command(
        &daemon.fixture.socket,
        true,
        "^TestProbe$",
        Some(active_context),
    );
    let mut waiting = OwnedTree::spawn(second, project.root.parent().unwrap());
    eventually(|| {
        assert_eq!(
            project.records().len(),
            1,
            "an unrelated claimant must not start while the slot is held"
        );
        let current: Vec<_> = daemon
            .rows("SELECT job_id,state,inherited_context FROM traffic_entries")
            .into_iter()
            .filter(|row| !old.contains(&row["job_id"]))
            .collect();
        current.len() == 2
            && current
                .iter()
                .any(|row| row["state"] == "QUEUED" && row["inherited_context"] == active_context)
    });
    assert_eq!(
        daemon.rows("SELECT state FROM permits WHERE state='ACTIVE'"),
        vec![json!({"state":"ACTIVE"})]
    );
    assert!(!holder.has_exited().unwrap());
    assert!(!waiting.has_exited().unwrap());
    fs::write(project.root.join("release"), "release").unwrap();
    assert_success(&holder.wait_with_output().unwrap());
    assert_success(&waiting.wait_with_output().unwrap());
    let records = project.records();
    assert_eq!(records.len(), 2);
    assert_eq!(records[0]["label"], "child");
    assert_eq!(records[1]["label"], "probe");
    assert!(daemon
        .rows("SELECT state FROM permits")
        .iter()
        .all(|row| row["state"] == "RELEASED"));
}

// A protocol-1 peer explicitly rejects the additive capability, then accepts
// the legacy UNKNOWN/OBSERVE lifecycle without any Traffic reservation.
fn legacy_peer(listener: &UnixListener) {
    let (mut stream, request) = next(listener);
    assert_eq!(request.protocol_version, 1);
    assert_eq!(
        request.operation,
        arc_ipc::traffic::native::OPERATION_CAPABILITIES
    );
    arc_ipc::write_response(
        &mut stream,
        &Response::error(request.request_id, arc_ipc::ERROR_UNSUPPORTED_CAPABILITY),
    )
    .unwrap();
    let (mut stream, request) = next(listener);
    let StartRequest::Register {
        job_id,
        observer,
        classification,
    } = arc_ipc::jobs::decode_start(&request.payload).unwrap()
    else {
        panic!("legacy register")
    };
    assert_eq!(classification.classification, arc_ipc::JobClass::Unknown);
    assert_eq!(classification.reason, arc_ipc::Reason::UnresolvedCommand);
    let mut job = json!({"job_id":job_id,"phase":"REGISTERED","classification":classification,"observer":observer,"observer_verified_at_ms":1,"child":null,"child_verified_at_ms":null,"observation":{"target":"OBSERVER","status":"VERIFIED_MATCH","observed_at_ms":1,"reason":"REGISTERED_NOT_CLAIMED"},"reason":"REGISTERED_NOT_CLAIMED","outcome":null,"registered_at_ms":1,"launch_claimed_at_ms":null,"completed_at_ms":null,"updated_at_ms":1,"revision":1});
    reply(
        &mut stream,
        &request,
        json!({"disposition":"CREATED","job":job}),
    );
    let (mut stream, request) = next(listener);
    assert!(matches!(
        arc_ipc::jobs::decode_start(&request.payload).unwrap(),
        StartRequest::Claim { .. }
    ));
    job["phase"] = json!("LAUNCHING");
    job["launch_claimed_at_ms"] = json!(1);
    job["reason"] = json!("CLAIMED_NOT_ATTACHED");
    job["observation"]["reason"] = job["reason"].clone();
    reply(
        &mut stream,
        &request,
        json!({"disposition":"APPLIED","job":job}),
    );
    let (mut stream, request) = next(listener);
    let StartRequest::Attach { child, .. } = arc_ipc::jobs::decode_start(&request.payload).unwrap()
    else {
        panic!("legacy attach")
    };
    job["phase"] = json!("RUNNING");
    job["child"] = json!(child);
    job["child_verified_at_ms"] = json!(1);
    job["reason"] = json!("CHILD_VERIFIED");
    job["observation"]["target"] = json!("CHILD");
    job["observation"]["reason"] = job["reason"].clone();
    reply(
        &mut stream,
        &request,
        json!({"disposition":"APPLIED","job":job}),
    );
    let (mut stream, request) = next(listener);
    let completion = arc_ipc::jobs::decode_complete(&request.payload).unwrap();
    assert_eq!(completion.job_id, job_id);
    assert_eq!(completion.observer, observer);
    assert_eq!(
        completion.outcome,
        arc_ipc::jobs::Outcome::Exited { code: 0 }
    );
    job["phase"] = json!("COMPLETED");
    job["outcome"] = json!(completion.outcome);
    job["completed_at_ms"] = json!(1);
    job["reason"] = json!("OBSERVED_EXIT");
    job["observation"]["reason"] = job["reason"].clone();
    reply(
        &mut stream,
        &request,
        json!({"disposition":"APPLIED","job":job}),
    );
    let (mut stream, request) = next(listener);
    assert_eq!(request.operation, arc_ipc::meter::OPERATION_METER_REPORT);
    arc_ipc::write_response(
        &mut stream,
        &Response::error(request.request_id, arc_ipc::ERROR_UNSUPPORTED_CAPABILITY),
    )
    .unwrap();
}

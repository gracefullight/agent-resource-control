//! Production daemon, native CLI, read-only SQLite; no socket result mocks.
use super::{native, native_process, serialization, support::output};
use arc_core::process_identity::{probe_live, IdentityStatus, ProcessIdentity};
use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, Write},
    os::unix::{
        fs::{symlink, MetadataExt, PermissionsExt},
        net::{UnixListener, UnixStream},
        process::ExitStatusExt,
    },
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

struct Daemon {
    process: native::OwnedChild,
    fixture: native::Fixture,
}
impl Daemon {
    fn new() -> Self {
        let binary = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("arc-pressure-daemon-fixture");
        assert!(
            binary.is_file(),
            "build the workspace native fixture: {}",
            binary.display()
        );
        let fixture = native::Fixture::with_binary(binary);
        fs::write(fixture.root.join("pressure"), b"Normal").unwrap();
        let mut command = fixture.command();
        command.env("ARC_PRESSURE_CONTROL", fixture.root.join("pressure"));
        let mut process = native::OwnedChild::new(command.spawn().unwrap());
        fixture.wait_for(&mut process, "arc-daemon ready.");
        Self { process, fixture }
    }
    fn cli(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_arcctl"));
        command
            .env(arc_core::ARC_SOCKET_PATH, &self.fixture.socket)
            .env(arc_core::ARC_DB_PATH, &self.fixture.database)
            .current_dir(&self.fixture.root)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }
    fn control(&self, action: &str) -> Value {
        let mut command = self.cli();
        command.args(["daemon", action, "--json"]);
        let result = output(command);
        assert!(result.status.success(), "{result:?}");
        serde_json::from_slice(&result.stdout).unwrap()
    }
    fn rows(&self, sql: &str) -> Vec<Value> {
        let result = Command::new("/usr/bin/sqlite3")
            .args(["-readonly", "-json"])
            .arg(&self.fixture.database)
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
    fn stop(&mut self) {
        self.process.stop(libc::SIGTERM);
        let report =
            serde_json::from_slice(&fs::read(self.fixture.root.join("runtime.json")).unwrap())
                .unwrap();
        native::assert_report(&report);
    }
    fn restart(&mut self) {
        self.stop();
        fs::rename(
            self.fixture.root.join("runtime.json"),
            self.fixture.root.join("runtime-before-restart.json"),
        )
        .unwrap();
        let mut command = self.fixture.command();
        command.env("ARC_PRESSURE_CONTROL", self.fixture.root.join("pressure"));
        self.process = native::OwnedChild::new(command.spawn().unwrap());
        self.fixture
            .wait_for(&mut self.process, "arc-daemon ready.");
    }
    fn status(&self, id: &str) -> std::process::Output {
        let mut command = self.cli();
        command.args(["job", "status", id]);
        output(command)
    }
}
fn eventually(mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !predicate() {
        assert!(
            Instant::now() < deadline,
            "native Governor reconciliation deadline"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn unreserved_missing_and_denied_executables_persist_spawn_failed_without_requeue_or_execution() {
    let _serial = serialization::serial();
    for (reason, os_code, denied) in [
        ("NOT_FOUND", libc::ENOENT, false),
        ("PERMISSION_DENIED", libc::EACCES, true),
    ] {
        let mut daemon = Daemon::new();
        let paused = daemon.control("pause");
        assert_eq!(paused["switch"]["enabled"], false);
        let marker = daemon.fixture.root.join("unexpected-starts");
        if denied {
            let executable = daemon.fixture.root.join("cargo");
            fs::write(
                &executable,
                format!("#!/bin/sh\necho started >> '{}'\n", marker.display()),
            )
            .unwrap();
            fs::set_permissions(executable, fs::Permissions::from_mode(0o600)).unwrap();
        }
        let mut command = daemon.cli();
        command
            .args(["exec", "--", "cargo", "test"])
            .env("PATH", &daemon.fixture.root)
            .env_remove("ARC_TRAFFIC_LINEAGE");
        let result = output(command);
        assert_eq!(result.status.code(), Some(1), "{result:?}");
        assert!(
            String::from_utf8_lossy(&result.stderr).contains("SPAWN_FAILED"),
            "{result:?}"
        );
        let rows = daemon.rows("SELECT l.job_id,l.outcome_kind,l.spawn_reason,l.os_code,l.child_pid,t.state FROM job_lifecycle l JOIN traffic_entries t USING(job_id)");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["outcome_kind"], "SPAWN_FAILED", "{rows:?}");
        assert_eq!(rows[0]["spawn_reason"], reason);
        assert_eq!(rows[0]["os_code"], os_code);
        assert!(rows[0]["child_pid"].is_null());
        assert_eq!(rows[0]["state"], "COMPLETED");
        assert!(daemon
            .rows("SELECT job_id FROM traffic_entries WHERE state='QUEUED'")
            .is_empty());
        assert!(daemon
            .rows("SELECT job_id FROM permits WHERE state='ACTIVE'")
            .is_empty());
        assert!(!marker.exists(), "failed exec must not run the target");
        let id = rows[0]["job_id"].as_str().unwrap();
        let status = daemon.status(id);
        assert!(status.status.success(), "{status:?}");
        assert!(
            String::from_utf8_lossy(&status.stdout).contains("SPAWN_FAILED"),
            "{status:?}"
        );
        daemon.stop();
    }
}

#[test]
fn production_cli_pause_replay_after_resume_keeps_historical_receipt_separate_from_current_state() {
    let _serial = serialization::serial();
    let mut daemon = Daemon::new();
    let id = format!("0-{}", "a".repeat(32));
    let mut command = daemon.cli();
    command.args([
        "daemon",
        "pause",
        "--json",
        "--expected-revision",
        "0",
        "--mutation-id",
        &id,
    ]);
    let original = output(command);
    assert!(original.status.success(), "{original:?}");
    let original: Value = serde_json::from_slice(&original.stdout).unwrap();
    assert_eq!(original["switch"]["enabled"], false);
    assert_eq!(original["switch"]["revision"], 1);
    let resumed = daemon.control("resume");
    assert_eq!(resumed["switch"]["enabled"], true);
    assert_eq!(resumed["switch"]["revision"], 2);
    for json_output in [false, true] {
        let mut command = daemon.cli();
        command.args([
            "daemon",
            "pause",
            "--expected-revision",
            "0",
            "--mutation-id",
            &id,
        ]);
        if json_output {
            command.arg("--json");
        }
        let replay = output(command);
        assert!(replay.status.success(), "{replay:?}");
        if json_output {
            let replay: Value = serde_json::from_slice(&replay.stdout).unwrap();
            assert_eq!(replay["switch"], resumed["switch"]);
            assert_eq!(replay["mutation_result"]["switch"], original["switch"]);
            assert_eq!(replay["mutation_result"]["replayed"], true);
        } else {
            let text = String::from_utf8(replay.stdout).unwrap();
            assert!(text.contains("Active · revision 2"), "{text}");
            assert!(!text.contains("Paused · revision 1"), "{text}");
        }
        assert_eq!(
            daemon.rows(
                "SELECT enabled,policy_revision FROM policy_states WHERE key='governor.enabled'"
            ),
            vec![json!({"enabled":1,"policy_revision":2})]
        );
    }
    assert_eq!(daemon.control("governor")["switch"], resumed["switch"]);
    daemon.stop();
}

fn checkpoint(listener: &UnixListener) -> (UnixStream, Value) {
    listener.set_nonblocking(true).unwrap();
    let mut connection = None;
    eventually(|| match listener.accept() {
        Ok((stream, _)) => {
            connection = Some(stream);
            true
        }
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => false,
        Err(error) => panic!("CLI checkpoint accept: {error}"),
    });
    let stream = connection.unwrap();
    // macOS accepts can inherit O_NONBLOCK from the listener. A read timeout
    // alone does not restore blocking reads of the complete checkpoint frame.
    stream.set_nonblocking(false).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut line = String::new();
    std::io::BufReader::new(&stream)
        .read_line(&mut line)
        .unwrap();
    (stream, serde_json::from_str(&line).unwrap())
}

// The test owns this fixture identity even after its wrapper is killed. Never
// signal a reused PID, and do not leave a live orphan behind if an assertion fails.
struct Orphan(ProcessIdentity);
impl Drop for Orphan {
    fn drop(&mut self) {
        if probe_live(&self.0) == IdentityStatus::VerifiedMatch {
            unsafe { libc::kill(self.0.pid as i32, libc::SIGKILL) };
        }
    }
}

#[test]
fn wrapper_loss_before_attachment_recovers_the_release_bound_child_with_and_without_restart() {
    let _serial = serialization::serial();
    for (converted, expired_then_resumed) in [(false, false), (true, false), (false, true)] {
        for restart in [false, true] {
            let mut daemon = Daemon::new();
            let root = &daemon.fixture.root;
            symlink(env!("CARGO_BIN_EXE_arc-exec-fixture"), root.join("cargo")).unwrap();
            let child_listener = UnixListener::bind(root.join("target.sock")).unwrap();
            let barrier = UnixListener::bind(root.join("checkpoint.sock")).unwrap();
            let marker = root.join("target.starts");
            let mut command = daemon.cli();
            command
                .args(["exec", "--", "cargo", "test"])
                .env("PATH", root)
                .env_remove("ARC_TRAFFIC_LINEAGE")
                .env("ARC_FIXTURE_MODE", "exit17")
                .env("ARC_FIXTURE_MARKER", &marker)
                .env("ARC_FIXTURE_CONTROL", root.join("target.sock"))
                .env(
                    "ARC_PRESSURE_ACCEPTANCE_STAGE",
                    if expired_then_resumed {
                        "ready,spawned"
                    } else if converted {
                        "reserved,spawned"
                    } else {
                        "spawned"
                    },
                )
                .env(
                    "ARC_PRESSURE_ACCEPTANCE_BARRIER",
                    root.join("checkpoint.sock"),
                )
                .stdout(fs::File::create(root.join("wrapper.out")).unwrap())
                .stderr(fs::File::create(root.join("wrapper.err")).unwrap());
            let paused = if converted {
                None
            } else {
                Some(daemon.control("pause"))
            };
            let mut wrapper = native::OwnedChild::new(command.spawn().unwrap());
            let paused = if converted {
                let (mut stream, reserved) = checkpoint(&barrier);
                assert_eq!(reserved["mode"], "RESERVED");
                let id = reserved["request"]["job_id"].as_str().unwrap();
                let witness_path = daemon.fixture.root.join(format!(".traffic-witness-{id}"));
                let inode = fs::metadata(&witness_path).unwrap().ino();
                assert_eq!(
                    daemon.rows("SELECT state FROM permits"),
                    vec![json!({"state":"ACTIVE"})]
                );
                let paused = daemon.control("pause");
                assert_eq!(
                    daemon.rows("SELECT state FROM permits"),
                    vec![json!({"state":"RELEASED"})]
                );
                assert_eq!(
                    fs::metadata(witness_path).unwrap().ino(),
                    inode,
                    "conversion preserves the witness file"
                );
                stream.write_all(b"x").unwrap();
                paused
            } else {
                paused.unwrap()
            };
            let confirmed = if expired_then_resumed {
                let (mut stream, ready) = checkpoint(&barrier);
                assert_eq!(ready["release"]["state"], "UNRESERVED");
                let expiry = Instant::now() + Duration::from_millis(2100);
                eventually(|| Instant::now() >= expiry);
                assert!(!marker.exists());
                let resumed = daemon.control("resume");
                assert_eq!(resumed["switch"]["enabled"], true);
                stream.write_all(b"x").unwrap();
                resumed
            } else {
                paused
            };
            let (_spawn_barrier, spawned) = checkpoint(&barrier);
            let id = spawned["job_id"].as_str().unwrap();
            let (mut target, hello) = native_process::accept(&child_listener);
            let identity: ProcessIdentity =
                serde_json::from_value(hello["identity"].clone()).unwrap();
            let child = Orphan(identity);
            assert_eq!(hello["ppid"], wrapper.child.id());
            assert_eq!(spawned["child_pid"], child.0.pid);
            assert_eq!(probe_live(&child.0), IdentityStatus::VerifiedMatch);
            let before = daemon.rows("SELECT l.job_id,l.child_pid,g.release_id,g.mode,t.inherited_context FROM job_lifecycle l JOIN governor_jobs g USING(job_id) JOIN traffic_entries t USING(job_id)");
            assert_eq!(before[0]["job_id"], id);
            assert!(
                before[0]["child_pid"].is_null(),
                "wrapper must die before attachment"
            );
            let witness_path = daemon.fixture.root.join(format!(".traffic-witness-{id}"));
            let bytes = fs::read(&witness_path).unwrap();
            assert_eq!(&bytes[..8], b"ARCWIT03");
            let expected_key = if expired_then_resumed {
                assert_eq!(before[0]["mode"], "SCHEDULED");
                assert!(before[0]["release_id"].is_null());
                before[0]["inherited_context"].as_str().unwrap().to_owned()
            } else {
                assert_eq!(before[0]["mode"], "UNRESERVED");
                let release = before[0]["release_id"].as_str().unwrap();
                format!("{release}{release}")
            };
            assert_eq!(
                &bytes[40..104],
                expected_key.as_bytes(),
                "witness must match the current recovery authority"
            );
            assert_eq!(
                u64::from_be_bytes(bytes[24..32].try_into().unwrap()),
                child.0.pid as u64
            );
            wrapper.child.kill().unwrap();
            assert_eq!(
                native_process::wait(&mut wrapper.child).signal(),
                Some(libc::SIGKILL)
            );
            if restart {
                daemon.restart();
            }
            eventually(|| {
                let status = daemon.status(id);
                assert!(status.status.success(), "{status:?}");
                daemon.rows("SELECT child_pid FROM job_lifecycle")[0]["child_pid"] == child.0.pid
            });
            let recovered = daemon.rows("SELECT l.child_pid,l.child_start_token,l.child_boot_id,l.outcome_kind,t.state FROM job_lifecycle l JOIN traffic_entries t USING(job_id)");
            assert_eq!(recovered[0]["child_start_token"], child.0.start_token);
            assert_eq!(recovered[0]["child_boot_id"], child.0.boot_id);
            assert_eq!(recovered[0]["state"], "RUNNING");
            assert!(recovered[0]["outcome_kind"].is_null());
            assert_eq!(daemon.control("governor")["switch"], confirmed["switch"]);
            assert_eq!(probe_live(&child.0), IdentityStatus::VerifiedMatch);
            assert!(
                !witness_path.exists(),
                "authenticated recovery consumes the witness"
            );
            assert_eq!(fs::read_to_string(&marker).unwrap().lines().count(), 1);
            target.write_all(b"x").unwrap();
            eventually(|| probe_live(&child.0) != IdentityStatus::VerifiedMatch);
            eventually(|| {
                let status = daemon.status(id);
                assert!(status.status.success(), "{status:?}");
                daemon.rows("SELECT state FROM traffic_entries")[0]["state"] == "ABANDONED"
            });
            let after = daemon.rows("SELECT j.state,l.child_pid,l.outcome_kind FROM jobs j JOIN job_lifecycle l ON j.id=l.job_id");
            assert_eq!(
                after[0]["state"], "OUTCOME_UNKNOWN",
                "dead wrappers cannot report an observed exit code"
            );
            assert_eq!(after[0]["child_pid"], child.0.pid);
            assert!(after[0]["outcome_kind"].is_null());
            let permits = daemon.rows("SELECT state FROM permits");
            assert_eq!(
                permits,
                if converted || expired_then_resumed {
                    vec![json!({"state":"RELEASED"})]
                } else {
                    vec![]
                }
            );
            assert_eq!(fs::read_to_string(&marker).unwrap().lines().count(), 1);
            eprintln!(
                "governor-client-recovery={}",
                json!({"converted":converted,"expiredThenResumed":expired_then_resumed,"restart":restart,"jobId":id,"child":child.0,"recovered":recovered,"after":after,"permits":permits,"executions":1})
            );
            daemon.stop();
        }
    }
}

use super::{
    protocol,
    runtime::{Fixture, OwnedChild},
    support,
};
use arc_ipc::jobs::{JobId, Outcome, Phase, StatusRequest, StatusResult, OPERATION_JOB_STATUS};
use serde_json::json;
use std::{
    ffi::OsString,
    fs,
    io::Write,
    os::unix::{ffi::OsStringExt, net::UnixListener, process::ExitStatusExt},
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
#[path = "../support/native_exec.rs"]
mod native_exec;
pub use native_exec::{accept, wait};

pub fn binary(name: &str) -> PathBuf {
    std::path::Path::new(env!("CARGO_BIN_EXE_arc-daemon")).with_file_name(name)
}
pub fn command(f: &Fixture) -> Command {
    let mut c = Command::new(binary("arcctl"));
    c.env(arc_core::ARC_SOCKET_PATH, &f.socket);
    c
}
pub fn child_command(f: &Fixture, managed: bool) -> Command {
    let mut c = if managed {
        let mut c = command(f);
        c.args(["exec", "--"]);
        c.arg(binary("arc-exec-fixture"));
        c
    } else {
        Command::new(binary("arc-exec-fixture"))
    };
    c.env("ARC_FIXTURE_MODE", "echo")
        .env("PRIVATE_ENV", "environment-secret-sentinel")
        .current_dir(&f.root);
    c
}
pub fn recent(f: &Fixture) -> JobId {
    let r = protocol::call(f, OPERATION_JOB_STATUS, json!(StatusRequest::Recent {}));
    let StatusResult::Recent { jobs, .. } = serde_json::from_value(r.result.unwrap()).unwrap()
    else {
        panic!()
    };
    jobs[0].job_id.clone()
}
#[test]
fn exact_native_argv_environment_cwd_streams_and_exit17_match_direct_execution() {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let args = vec![
        OsString::from(""),
        "two words".into(),
        "--leading".into(),
        "한글".into(),
        "; | $(touch forbidden)".into(),
        OsString::from_vec(vec![0xff, 0xfe]),
        "argv-secret-sentinel".into(),
    ];
    let mut outputs = Vec::new();
    for managed in [false, true] {
        let mut c = child_command(&f, managed);
        c.args(&args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = c.spawn().unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"stdin\0bytes\n")
            .unwrap();
        wait(&mut child);
        outputs.push(child.wait_with_output().unwrap());
    }
    assert_eq!(outputs[0].status.code(), Some(17));
    assert_eq!(outputs[0].status, outputs[1].status);
    assert_eq!(outputs[0].stdout, outputs[1].stdout);
    assert_eq!(outputs[0].stderr, outputs[1].stderr);
    assert!(!f.root.join("forbidden").exists());
    let job = protocol::status(&f, &recent(&f));
    assert_eq!(job.outcome, Some(Outcome::Exited { code: 17 }));
    assert!(job.child.is_some());
    assert_eq!(
        job.classification.classification,
        arc_ipc::JobClass::Unknown
    );
    let db = rusqlite::Connection::open(&f.database).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM permits", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
    drop(db);
    daemon.stop(libc::SIGTERM);
    f.report();
    for entry in fs::read_dir(&f.root).unwrap() {
        let p = entry.unwrap().path();
        if p.is_file() {
            let bytes = fs::read(&p).unwrap();
            for secret in ["argv-secret-sentinel", "environment-secret-sentinel"] {
                assert!(
                    !bytes.windows(secret.len()).any(|w| w == secret.as_bytes()),
                    "secret persisted: {p:?}"
                );
            }
        }
    }
}
#[test]
fn spawn_failure_quick_exit_help_and_status_have_static_diagnostics() {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    for args in [
        vec!["exec", "--", "/missing/secret-executable"],
        vec!["exec", "--", "cargo test"],
    ] {
        let out = command(&f).args(args).output().unwrap();
        assert_eq!(out.status.code(), Some(1));
        assert!(out.stdout.is_empty());
        assert_eq!(out.stderr,b"SPAWN_FAILED\nNo child was started. Check the executable and execution permissions.\n");
        assert_eq!(protocol::status(&f, &recent(&f)).phase, Phase::SpawnFailed);
    }
    let out = command(&f)
        .args(["exec", "--", "/usr/bin/true"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(out.stderr.is_empty());
    assert!(out.stdout.is_empty());
    let job = recent(&f);
    let out = command(&f)
        .args(["job", "status", job.as_str()])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(String::from_utf8(out.stdout)
        .unwrap()
        .contains("State: succeeded"));
    for args in [
        vec!["exec"],
        vec!["exec", "--bad-secret"],
        vec!["exec", "secret"],
    ] {
        let out = command(&f).args(args).output().unwrap();
        assert_eq!(out.status.code(), Some(2));
        assert!(out.stdout.is_empty());
        assert_eq!(
            out.stderr,
            b"INVALID_REQUEST\nUse arcctl exec -- <PROGRAM> [ARG...]. No child was started.\n"
        );
    }
    let out = command(&f).args(["exec", "--help"]).output().unwrap();
    assert!(out.status.success());
    assert!(out.stderr.is_empty());
    let help = String::from_utf8(out.stdout).unwrap();
    assert!(help.contains("Stopping the daemon does not stop"));
    assert!(help.contains("Usage: arcctl exec -- <PROGRAM> [ARG...]"));
    assert!(!help.contains("ARG...>"));
    f.assert_cli();
    daemon.stop(libc::SIGTERM);
    f.report();
    let out = super::historical::after_shutdown(command(&f))
        .args(["exec", "--", "/usr/bin/true"])
        .output()
        .unwrap();
    assert_eq!(
        out.stderr,
        b"NOT_STARTED\nNo child was started. Check the ARC daemon connection.\n"
    );
    let out = command(&f).args(["job", "status"]).output().unwrap();
    assert_eq!(out.stderr,b"STATUS_UNAVAILABLE\nJob status is unavailable. No work was launched by this status request.\n");
}
#[test]
fn daemon_shutdown_and_crash_preserve_child_and_restart_reconciles_without_replay() {
    let _serial = support::serial();
    for signal in [libc::SIGTERM, libc::SIGKILL] {
        let f = Fixture::new();
        let mut daemon = f.spawn("test");
        let marker = f.root.join("invocations");
        let control = f.root.join("child.sock");
        let listener = UnixListener::bind(&control).unwrap();
        let mut c = child_command(&f, true);
        c.env("ARC_FIXTURE_CONTROL", &control)
            .env("ARC_FIXTURE_MARKER", &marker)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut wrapper = OwnedChild::new(c.spawn().unwrap());
        let (mut barrier, msg) = accept(&listener);
        let identity: arc_core::process_identity::ProcessIdentity =
            serde_json::from_value(msg["identity"].clone()).unwrap();
        let job_id = recent(&f);
        let end = Instant::now() + Duration::from_secs(5);
        while protocol::status(&f, &job_id).phase != Phase::Running {
            assert!(Instant::now() < end);
            thread::yield_now();
        }
        if signal == libc::SIGTERM {
            daemon.stop(signal);
            f.report();
        } else {
            daemon.child.kill().unwrap();
            wait(&mut daemon.child);
        }
        assert_eq!(
            arc_core::process_identity::probe_live(&identity),
            arc_core::process_identity::IdentityStatus::VerifiedMatch
        );
        let mut restarted = restart(&f);
        let job = protocol::status(&f, &job_id);
        assert_eq!(job.phase, Phase::Running);
        assert_eq!(job.child, Some(identity));
        barrier.write_all(b"x").unwrap();
        let status = wait(&mut wrapper.child);
        assert_eq!(status.code(), Some(17));
        let completed = protocol::status(&f, &job_id);
        assert_eq!(
            completed.observation.observed_at_ms,
            job.observation.observed_at_ms
        );
        assert_eq!(completed.outcome, Some(Outcome::Exited { code: 17 }));
        assert_eq!(fs::read_to_string(&marker).unwrap().lines().count(), 1);
        restarted.stop(libc::SIGTERM);
        f.report();
    }
}
#[test]
fn wrapper_directed_signal_preserves_native_signal_status_and_no_second_child() {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let control = f.root.join("child.sock");
    let listener = UnixListener::bind(&control).unwrap();
    let marker = f.root.join("invocations");
    let mut c = child_command(&f, true);
    c.env("ARC_FIXTURE_CONTROL", &control)
        .env("ARC_FIXTURE_MARKER", &marker)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut wrapper = OwnedChild::new(c.spawn().unwrap());
    let (_barrier, _) = accept(&listener);
    unsafe {
        assert_eq!(libc::kill(wrapper.child.id() as i32, libc::SIGTERM), 0);
    }
    assert_eq!(wait(&mut wrapper.child).signal(), Some(libc::SIGTERM));
    assert_eq!(
        protocol::status(&f, &recent(&f)).outcome,
        Some(Outcome::Signaled {
            signal: libc::SIGTERM as u8,
            core_dumped: false
        })
    );
    let job_id = recent(&f);
    let job = protocol::status(&f, &job_id);
    let out = command(&f)
        .args(["job", "status", job_id.as_str()])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(out.stderr.is_empty());
    let status = String::from_utf8(out.stdout).unwrap();
    assert!(status.contains("State: terminated by signal\n"));
    assert!(status.contains(&format!("Signal: {}\n", libc::SIGTERM)));
    assert!(status.contains("Core dumped: false\n"), "{status}");
    assert!(!status.contains("Exit code:"));
    assert!(status.contains(&format!(
        "Completed (Unix ms): {}\n",
        job.completed_at_ms.unwrap()
    )));
    assert_eq!(protocol::status(&f, &job_id), job);
    assert_eq!(fs::read_to_string(marker).unwrap().lines().count(), 1);
    daemon.stop(libc::SIGTERM);
    f.report();
}

pub fn restart(f: &Fixture) -> OwnedChild {
    fs::rename(
        f.root.join("runtime.json"),
        f.root.join("prior-runtime.json"),
    )
    .unwrap();
    f.spawn("test")
}

use super::{
    cli::{accept, binary, child_command, command, restart, wait},
    fault::{Fault, MODES},
    runtime::{Fixture, OwnedChild},
    support,
};
use std::{
    ffi::OsString,
    fs,
    io::Write,
    os::unix::{ffi::OsStringExt, net::UnixListener, process::ExitStatusExt},
    process::Stdio,
    time::{Duration, Instant},
};

pub fn receipt(name: &str, value: serde_json::Value) {
    let root = std::path::PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../target/exec-fail-open/native"
    ));
    let root = root.join(std::process::id().to_string());
    fs::create_dir_all(&root).unwrap();
    let mut value = value;
    let digest = std::process::Command::new("/usr/bin/shasum")
        .args(["-a", "256"])
        .arg(binary("arcctl"))
        .output()
        .unwrap();
    assert!(digest.status.success());
    value["candidateBinaryPath"] = binary("arcctl").to_string_lossy().as_ref().into();
    value["candidateSha256"] = String::from_utf8(digest.stdout)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .into();
    value["test"] = std::thread::current().name().unwrap().into();
    value["observerPid"] = std::process::id().into();
    value["nativeMacos"] = cfg!(target_os = "macos").into();
    value["recordedUnixMs"] = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64)
        .into();
    fs::write(
        root.join(format!("{name}.json")),
        serde_json::to_vec_pretty(&value).unwrap(),
    )
    .unwrap();
}
pub fn arc_stderr(bytes: &[u8]) -> Vec<u8> {
    let text = std::str::from_utf8(bytes).unwrap();
    let lines = ["TRACKING_UNAVAILABLE", "Command started. ARC tracking is unavailable. A lifecycle record may be absent.", "Do not launch another copy to recover tracking.", "TRACKING_UNCERTAIN", "Command started. Lifecycle recording is unconfirmed.", "A missing record does not show whether the command ran. Do not launch another copy to recover tracking."];
    let recovery = "After daemon recovery, inspect arcctl job status ";
    text.split_inclusive('\n')
        .filter(|raw| {
            let l = raw.trim_end_matches(['\r', '\n']);
            !lines.contains(&l)
                && l.strip_prefix("Reference: ")
                    .is_none_or(|id| arc_ipc::jobs::JobId::try_from(id.to_string()).is_err())
                && l.strip_prefix(recovery)
                    .and_then(|id| id.strip_suffix('.'))
                    .is_none_or(|id| arc_ipc::jobs::JobId::try_from(id.to_string()).is_err())
        })
        .collect::<String>()
        .into_bytes()
}
#[test]
fn outages_preserve_native_bytes_exit17_one_invocation_budget_and_no_replay() {
    let _serial = support::serial();
    for mode in MODES {
        let f = Fixture::new();
        let mut daemon = f.spawn("test");
        let fault = Fault::new(&f, mode);
        let args = [
            OsString::from(""),
            "two words".into(),
            "--leading".into(),
            "한글".into(),
            "; | $(touch forbidden)".into(),
            OsString::from_vec(vec![0xff, 0xfe]),
            "argv-secret-sentinel".into(),
        ];
        let marker = f.root.join("calls");
        let mut outputs = Vec::new();
        let mut elapsed = Duration::ZERO;
        let mut wrapper_pid = 0;
        let mut target_pid = 0;
        for wrapped in [false, true] {
            let control = f.root.join(format!("control-{wrapped}.sock"));
            let listener = UnixListener::bind(&control).unwrap();
            let mut c = child_command(&f, wrapped);
            c.args(&args)
                .env(arc_core::ARC_SOCKET_PATH, &fault.path)
                .env("ARC_FIXTURE_CONTROL", &control)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            if wrapped {
                c.env("ARC_FIXTURE_MARKER", &marker);
            }
            // Parent monotonic start includes wrapper startup and handshake.
            let start = Instant::now();
            let mut child = c.spawn().unwrap();
            let (mut barrier, info) = accept(&listener);
            if wrapped {
                elapsed = start.elapsed();
                wrapper_pid = child.id();
                target_pid = info["pid"].as_u64().unwrap();
            }
            child
                .stdin
                .take()
                .unwrap()
                .write_all(b"stdin\0bytes\n")
                .unwrap();
            barrier.write_all(b"x").unwrap();
            wait(&mut child);
            outputs.push(child.wait_with_output().unwrap());
        }
        let report = fault.finish();
        let counts = report.counts;
        let events = report.events;
        assert!(
            elapsed <= Duration::from_millis(2000),
            "{mode}: {elapsed:?}"
        );
        assert_eq!(outputs[0].status.code(), Some(17));
        assert_eq!(outputs[0].status, outputs[1].status);
        live_fallback_assertions(&outputs[1], &marker);
        assert_eq!(outputs[0].stdout, outputs[1].stdout);
        assert_eq!(outputs[0].stderr, arc_stderr(&outputs[1].stderr));
        let diagnostic = String::from_utf8_lossy(&outputs[1].stderr);
        assert!(
            diagnostic.contains(if matches!(mode, "absent" | "refused") {
                "TRACKING_UNAVAILABLE"
            } else {
                "TRACKING_UNCERTAIN"
            }),
            "{diagnostic}"
        );
        assert!(!diagnostic.contains("secret"));
        assert!(!f.root.join("forbidden").exists());
        assert_eq!(fs::read_to_string(&marker).unwrap().lines().count(), 1);
        if matches!(
            mode,
            "absent"
                | "refused"
                | "disconnect-before-read"
                | "disconnect-register"
                | "stall-register"
                | "partial-register"
                | "committed-register"
                | "busy-register"
        ) {
            assert!(!counts.contains_key("CLAIM"));
        }
        assert!(!counts.contains_key("ATTACH"));
        assert!(!counts.contains_key("job.complete"));
        if mode.starts_with("committed-") {
            let action = if mode == "committed-register" {
                "REGISTER"
            } else {
                "CLAIM"
            };
            assert_eq!(counts[&format!("{action}-committed")], 3);
        }
        let db = rusqlite::Connection::open(&f.database).unwrap();
        let commits: Vec<serde_json::Value> = db.prepare("SELECT l.job_id, l.revision, l.launch_claimed_at_ms, l.child_pid, l.reason, j.state FROM job_lifecycle l JOIN jobs j ON j.id=l.job_id").unwrap().query_map([], |r| {
            Ok(serde_json::json!({"jobId":r.get::<_,String>(0)?,"revision":r.get::<_,i64>(1)?,"claimed":r.get::<_,Option<i64>>(2)?.is_some(),"childPid":r.get::<_,Option<i64>>(3)?,"reason":r.get::<_,String>(4)?,"phase":r.get::<_,String>(5)?}))
        }).unwrap().map(|r| r.unwrap()).collect();
        drop(db);
        daemon.stop(libc::SIGTERM);
        f.report();
        let mut recovered = restart(&f);
        let status = f.cli("status");
        assert!(status.status.success());
        assert!(String::from_utf8(status.stdout)
            .unwrap()
            .contains("Sweeper off."));
        for _ in 0..2 {
            assert!(command(&f)
                .args(["job", "status"])
                .output()
                .unwrap()
                .status
                .success());
        }
        let db = rusqlite::Connection::open(&f.database).unwrap();
        assert_eq!(
            db.query_row("SELECT count(*) FROM permits", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            0
        );
        let jobs: i64 = db
            .query_row("SELECT count(*) FROM job_lifecycle", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            jobs,
            i64::from(
                mode.contains("claim") || mode.starts_with("committed-") || mode == "shared-budget"
            )
        );
        drop(db);
        assert_eq!(fs::read_to_string(&marker).unwrap().lines().count(), 1);
        recovered.stop(libc::SIGTERM);
        f.report();
        for entry in fs::read_dir(&f.root).unwrap() {
            let p = entry.unwrap().path();
            if !p.is_file() {
                continue;
            }
            let bytes = fs::read(&p).unwrap();
            for secret in ["argv-secret-sentinel", "environment-secret-sentinel"] {
                assert!(
                    !bytes.windows(secret.len()).any(|b| b == secret.as_bytes()),
                    "persisted secret"
                );
            }
        }
        receipt(
            mode,
            serde_json::json!({"scenario":mode,"parentSpawnToInvocationMs":elapsed.as_secs_f64()*1000.0,"boundary":"parent Instant before wrapper spawn to child control handshake; includes startup and observer overhead","subdivisions":null,"subdivisionReason":"internal metadata and spawn times are not exported","wrapperPid":wrapper_pid,"childPid":target_pid,"invocations":1,"observedExit":17,"recoveryInvocations":1,"permits":0,"arcCleanup":false,"counts":counts,"events":events,"proxyClock":"std::time::Instant relative to proxy thread origin; not t_meta","commitRows":commits,"samples":{"startNs":0,"endNs":elapsed.as_nanos() as u64,"clock":"std::time::Instant relative to parent-before-wrapper-spawn"},"passed":true}),
        );
    }
}
#[test]
fn rejection_invalid_reply_arguments_and_local_config_never_launch() {
    let _serial = support::serial();
    for mode in [
        "reject-register",
        "reject-claim",
        "invalid-register",
        "invalid-claim",
    ] {
        let f = Fixture::new();
        let mut daemon = f.spawn("test");
        let fault = Fault::new(&f, mode);
        let marker = f.root.join("calls");
        let out = child_command(&f, true)
            .env(arc_core::ARC_SOCKET_PATH, &fault.path)
            .env("ARC_FIXTURE_MARKER", &marker)
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(1));
        assert!(!marker.exists());
        assert!(out.stdout.is_empty());
        assert!(out.stderr.starts_with(if mode.starts_with("reject") {
            b"NOT_STARTED\nARC rejected"
        } else {
            b"IPC_ERROR\nNOT_STARTED"
        }));
        assert!(!String::from_utf8_lossy(&out.stderr).contains("secret"));
        fault.finish();
        daemon.stop(libc::SIGTERM);
        f.report();
    }
    let f = Fixture::new();
    for args in [
        vec!["exec"],
        vec!["exec", "--", ""],
        vec!["exec", "bad-grammar"],
    ] {
        let out = command(&f).args(args).output().unwrap();
        assert_eq!(out.status.code(), Some(2));
        assert!(out.stderr.starts_with(b"INVALID_REQUEST"));
    }
    let out = child_command(&f, true)
        .env(arc_core::ARC_SOCKET_PATH, "x".repeat(200))
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(!String::from_utf8_lossy(&out.stderr).contains("Command started"));
    receipt(
        "nonlaunch",
        serde_json::json!({"scenario":"nonlaunch","passed":true,"invocations":0}),
    );
}
#[test]
fn cancellation_during_register_claim_and_near_deadline_never_launches() {
    let _serial = support::serial();
    for (mode, near_deadline) in [
        ("stall-register", false),
        ("stall-claim", false),
        ("stall-register", true),
    ] {
        let f = Fixture::new();
        let mut daemon = f.spawn("test");
        let fault = Fault::new(&f, mode);
        let marker = f.root.join("calls");
        let mut c = child_command(&f, true);
        c.env(arc_core::ARC_SOCKET_PATH, &fault.path)
            .env("ARC_FIXTURE_MARKER", &marker)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = c.spawn().unwrap();
        // Observe request admission through the real socket by waiting for the
        // proxy barrier, rather than estimating when the wrapper started.
        fault.wait_for_request();
        if near_deadline {
            std::thread::sleep(Duration::from_millis(1650));
        }
        assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGTERM) }, 0);
        wait(&mut child);
        let out = child.wait_with_output().unwrap();
        assert_eq!(out.status.code(), Some(1));
        assert!(!marker.exists());
        assert_eq!(
            out.stderr,
            b"NOT_STARTED\nExecution cancelled. No child was started.\n"
        );
        fault.finish();
        daemon.stop(libc::SIGTERM);
        f.report();
    }
    receipt(
        "cancellation",
        serde_json::json!({"scenario":"cancellation","passed":true,"invocations":0}),
    );
}
#[test]
fn directed_signals_preserve_native_status_during_fallback() {
    let _serial = support::serial();
    for mode in ["absent", "committed-claim"] {
        for signal in [libc::SIGTERM, libc::SIGHUP] {
            let f = Fixture::new();
            let mut daemon = f.spawn("test");
            let fault = Fault::new(&f, mode);
            let control = f.root.join("child.sock");
            let listener = UnixListener::bind(&control).unwrap();
            let marker = f.root.join("calls");
            let mut c = child_command(&f, true);
            c.env(arc_core::ARC_SOCKET_PATH, &fault.path)
                .env("ARC_FIXTURE_CONTROL", &control)
                .env("ARC_FIXTURE_MARKER", &marker)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            let mut wrapper = OwnedChild::new(c.spawn().unwrap());
            let (_barrier, _) = accept(&listener);
            assert_eq!(unsafe { libc::kill(wrapper.child.id() as i32, signal) }, 0);
            assert_eq!(wait(&mut wrapper.child).signal(), Some(signal));
            assert_eq!(fs::read_to_string(marker).unwrap().lines().count(), 1);
            fault.finish();
            daemon.stop(libc::SIGTERM);
            f.report();
        }
    }
    receipt(
        "signals",
        serde_json::json!({"scenario":"signals","passed":true,"invocations":1,"signals":["SIGTERM","SIGHUP"]}),
    );
}

fn live_fallback_assertions(output: &std::process::Output, marker: &std::path::Path) {
    assert_eq!(output.status.code(), Some(17));
    assert_eq!(fs::read_to_string(marker).unwrap().lines().count(), 1);
    assert!(String::from_utf8_lossy(&output.stderr).contains("TRACKING_"));
}
#[test]
fn historical_arcctl_fails_live_fallback_assertions() {
    let _serial = support::serial();
    let f = Fixture::new();
    let marker = f.root.join("negative-control-calls");
    let output = std::process::Command::new(super::historical::negative_control_binary())
        .args(["exec", "--"])
        .arg(binary("arc-exec-fixture"))
        .env(arc_core::ARC_SOCKET_PATH, &f.socket)
        .env("ARC_FIXTURE_MODE", "echo")
        .env("ARC_FIXTURE_MARKER", &marker)
        .output()
        .unwrap();
    // This is an expected failing fixture, never a passing live run.
    assert!(std::panic::catch_unwind(|| live_fallback_assertions(&output, &marker)).is_err());
    assert_eq!(output.status.code(), Some(1));
    assert!(!marker.exists());
    receipt(
        "negative-control",
        serde_json::json!({"scenario":"negative-control","expectedFailure":true,"historicalExit":1,"invocations":0,"passed":true}),
    );
}

use super::*;
use std::{
    io::{Read, Write},
    os::unix::{net::UnixListener, process::ExitStatusExt},
};

fn class_execution(program: &str, subcommand: &str, class: &str, reason: &str) {
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let project = projects(&f.root, subcommand);
    let mut first = spawn_build(&f, &project[0], true);
    eventually(|| project[0].join("started").exists());
    symlink(binary("arc-exec-fixture"), f.root.join(program)).unwrap();
    let args = if program == "bun" {
        vec![
            subcommand,
            "--test-name-pattern",
            "watch",
            "two words",
            "argv-secret-sentinel",
        ]
    } else {
        vec![subcommand, "--offline", "two words", "argv-secret-sentinel"]
    };
    let configure = |command: &mut Command, marker: &Path| {
        command
            .args(&args)
            .current_dir(&project[1])
            .env("ARC_FIXTURE_MODE", "echo")
            .env("ARC_FIXTURE_MARKER", marker)
            .env("PRIVATE_ENV", "environment-secret-sentinel")
            .env_remove("ARC_TRAFFIC_LINEAGE")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
    };
    let mut direct = Command::new(f.root.join(program));
    configure(&mut direct, &f.root.join("direct-marker"));
    let mut direct = direct.spawn().unwrap();
    direct
        .stdin
        .take()
        .unwrap()
        .write_all(b"stdin\0bytes\n")
        .unwrap();
    let expected = direct.wait_with_output().unwrap();
    assert_eq!(expected.status.code(), Some(17));

    let marker = f.root.join("managed-marker");
    let mut command = Command::new(binary("arcctl"));
    command
        .args(["exec", "--", program])
        .env(arc_core::ARC_SOCKET_PATH, &f.socket)
        .env(
            "PATH",
            format!("{}:{}", f.root.display(), std::env::var("PATH").unwrap()),
        );
    configure(&mut command, &marker);
    let mut wrapper = OwnedChild::new(command.spawn().unwrap());
    eventually(|| entry(&f, wrapper.child.id()).is_some_and(|e| e.0 == "QUEUED"));
    assert!(!marker.exists());
    let (actual_class, actual_reason): (String, String) = database(&f).query_row(
        "SELECT j.job_class,l.classification_reason FROM jobs j JOIN job_lifecycle l ON l.job_id=j.id WHERE l.observer_pid=?1",
        [wrapper.child.id()], |row| Ok((row.get(0)?, row.get(1)?))).unwrap();
    assert_eq!(
        (actual_class.as_str(), actual_reason.as_str()),
        (class, reason)
    );
    fs::write(project[0].join("release"), "release").unwrap();
    finish(&mut first);
    eventually(|| {
        marker.exists() && entry(&f, wrapper.child.id()).is_some_and(|e| e.0 == "RUNNING")
    });
    let child_pid = entry(&f, wrapper.child.id()).unwrap().1.unwrap();
    assert_eq!(unsafe { libc::getpgid(child_pid as i32) }, child_pid as i32);
    wrapper
        .child
        .stdin
        .take()
        .unwrap()
        .write_all(b"stdin\0bytes\n")
        .unwrap();
    eventually(|| wrapper.child.try_wait().unwrap().is_some());
    assert_eq!(wrapper.child.wait().unwrap().code(), Some(17));
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    wrapper
        .child
        .stdout
        .take()
        .unwrap()
        .read_to_end(&mut stdout)
        .unwrap();
    wrapper
        .child
        .stderr
        .take()
        .unwrap()
        .read_to_end(&mut stderr)
        .unwrap();
    assert_eq!(stdout, expected.stdout);
    assert!(stderr.ends_with(&expected.stderr));
    assert!(String::from_utf8_lossy(&stderr).contains("QUEUED\nWaiting to start."));
    assert_eq!(fs::read_to_string(marker).unwrap().lines().count(), 1);
    assert_eq!(active(&f).1, 0);
    daemon.stop(libc::SIGTERM);
    for path in [&f.database, &f.root.join("stdout"), &f.root.join("stderr")] {
        let bytes = fs::read(path).unwrap();
        for secret in [
            b"argv-secret-sentinel".as_slice(),
            b"environment-secret-sentinel".as_slice(),
        ] {
            assert!(!bytes.windows(secret.len()).any(|window| window == secret));
        }
    }
    f.report();
}

#[test]
fn scheduled_test_and_lint_preserve_native_streams_and_admission() {
    let _serial = support::serial();
    class_execution("cargo", "test", "TEST", "CARGO_TEST");
    class_execution("cargo", "clippy", "LINT", "CARGO_CLIPPY");
    class_execution("bun", "test", "TEST", "BUN_TEST");
}

include!("semantics/environment.rs");
include!("semantics/lineage_recovery.rs");

#[test]
fn scheduled_child_signal_is_returned_as_native_signal_status() {
    let _serial = support::serial();
    for program in ["cargo", "bun"] {
        let f = Fixture::new();
        let mut daemon = f.spawn("test");
        symlink(binary("arc-exec-fixture"), f.root.join(program)).unwrap();
        let output = Command::new(binary("arcctl"))
            .args(["exec", "--", program, "test"])
            .env(arc_core::ARC_SOCKET_PATH, &f.socket)
            .env(
                "PATH",
                format!("{}:{}", f.root.display(), std::env::var("PATH").unwrap()),
            )
            .env("ARC_FIXTURE_MODE", "term")
            .env_remove("ARC_TRAFFIC_LINEAGE")
            .output()
            .unwrap();
        assert_eq!(output.status.signal(), Some(libc::SIGTERM));
        let (outcome, signal): (String, i64) = database(&f)
            .query_row(
                "SELECT outcome_kind,signal FROM job_lifecycle WHERE outcome_kind='SIGNALED'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            (outcome.as_str(), signal),
            ("SIGNALED", libc::SIGTERM as i64)
        );
        assert_eq!(active(&f).1, 0);
        daemon.stop(libc::SIGTERM);
        f.report();
    }
}

#[test]
fn bun_watch_hot_unresolved_scripts_and_remote_bypass_a_full_test_window() {
    use arc_ipc::{
        jobs::{JobId, ProcessIdentity},
        traffic::{JobRequest, JobResult},
    };
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let id = JobId::try_from("d123456789abcdef0123456789abcdef".to_owned()).unwrap();
    let hold = JobRequest::Enqueue {
        job_id: id.clone(),
        observer: observer.clone(),
        classification: cargo_class("TEST", "CARGO_TEST"),
        lineage_context: None,
    };
    let JobResult::Granted {
        permit_generation, ..
    } = traffic_result(traffic(&f, &hold))
    else {
        panic!("expected holder grant")
    };
    for program in ["bun", "ssh"] {
        symlink(binary("arc-exec-fixture"), f.root.join(program)).unwrap();
    }
    for args in [
        vec!["bun", "test", "--watch"],
        vec!["bun", "test", "--hot"],
        vec!["bun", "test", "--mystery"],
        vec!["bun", "run", "test"],
        vec!["ssh", "host.example", "bun test"],
    ] {
        let marker = f
            .root
            .join(format!("bypass-{}", args.join("_").replace(' ', "_")));
        let before = active(&f);
        let started = Instant::now();
        let output = Command::new(binary("arcctl"))
            .args(["exec", "--"])
            .args(&args)
            .env(arc_core::ARC_SOCKET_PATH, &f.socket)
            .env("PATH", &f.root)
            .env("ARC_FIXTURE_MODE", "echo")
            .env("ARC_FIXTURE_MARKER", &marker)
            .env_remove("ARC_TRAFFIC_LINEAGE")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(17), "{args:?}: {output:?}");
        assert!(started.elapsed() < Duration::from_secs(3));
        assert!(!String::from_utf8_lossy(&output.stderr).contains("QUEUED"));
        assert_eq!(fs::read_to_string(marker).unwrap().lines().count(), 1);
        assert_eq!(active(&f), before);
    }
    assert_eq!(
        traffic_result(traffic(
            &f,
            &JobRequest::Cancel {
                job_id: id,
                observer,
                permit_generation: Some(permit_generation)
            }
        )),
        JobResult::Cancelled {}
    );
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn bun_lost_grant_attach_and_completion_replies_reuse_one_permit() {
    use arc_ipc::{
        jobs::{JobId, Outcome, ProcessIdentity},
        traffic::{JobRequest, JobResult, OPERATION_BUN_JOB},
    };
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let id = JobId::try_from("b123456789abcdef0123456789abcdef".to_owned()).unwrap();
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let enqueue = JobRequest::Enqueue {
        job_id: id.clone(),
        observer: observer.clone(),
        classification: cargo_class("TEST", "BUN_TEST"),
        lineage_context: None,
    };
    let send = |input: &JobRequest| {
        let mut request = arc_ipc::Request::new("bun-retry", OPERATION_BUN_JOB);
        request.payload = serde_json::to_value(input).unwrap();
        arc_ipc::exchange(&mut f.connect(), &request).unwrap()
    };
    let lose = |input: &JobRequest| {
        let mut request = arc_ipc::Request::new("bun-lost", OPERATION_BUN_JOB);
        request.payload = serde_json::to_value(input).unwrap();
        let mut stream = f.connect();
        arc_ipc::write_message(&mut stream, &request).unwrap();
        stream
    };
    let lost = lose(&enqueue);
    eventually(|| active(&f).1 == 1);
    drop(lost);
    let first = traffic_result(send(&enqueue));
    assert_eq!(traffic_result(send(&enqueue)), first);
    let JobResult::Granted {
        permit_generation, ..
    } = first
    else {
        panic!("expected Bun grant")
    };
    let mut child = OwnedChild::new(Command::new("/bin/sleep").arg("1").spawn().unwrap());
    let attach = JobRequest::Attach {
        job_id: id.clone(),
        observer: observer.clone(),
        child: ProcessIdentity::capture(child.child.id()).unwrap(),
        permit_generation,
    };
    let lost = lose(&attach);
    eventually(|| {
        database(&f)
            .query_row(
                "SELECT state='RUNNING' FROM traffic_entries WHERE job_id=?1",
                [id.as_str()],
                |r| r.get::<_, bool>(0),
            )
            .unwrap_or(false)
    });
    drop(lost);
    assert_eq!(traffic_result(send(&attach)), JobResult::Running {});
    assert!(child.child.wait().unwrap().success());
    let complete = JobRequest::Complete {
        job_id: id.clone(),
        observer,
        permit_generation,
        outcome: Outcome::Exited { code: 0 },
    };
    let lost = lose(&complete);
    eventually(|| active(&f).1 == 0);
    drop(lost);
    let completed = traffic_result(send(&complete));
    assert!(matches!(
        completed,
        JobResult::Completed {
            outcome: Outcome::Exited { code: 0 },
            ..
        }
    ));
    assert_eq!(traffic_result(send(&complete)), completed);
    assert_eq!(
        database(&f)
            .query_row("SELECT count(*) FROM permits", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn cargo_and_bun_nested_jobs_reuse_the_parent_permit_in_both_directions() {
    let _serial = support::serial();
    for parent in ["cargo", "bun"] {
        let f = Fixture::new();
        let mut daemon = f.spawn("test");
        let cargo = f.root.join("nested_cargo");
        let bun = f.root.join("nested_bun");
        fs::create_dir_all(cargo.join("src")).unwrap();
        fs::create_dir(&bun).unwrap();
        fs::write(
            cargo.join("Cargo.toml"),
            "[package]\nname='nested_test'\nversion='0.1.0'\nedition='2021'\n",
        )
        .unwrap();
        let cargo_source = if parent == "cargo" {
            r#"
#[test] fn root() {
    use std::{fs::OpenOptions, io::Write, process::Command};
    writeln!(OpenOptions::new().create(true).append(true).open(std::env::var("ROOT_MARKER").unwrap()).unwrap(), "root").unwrap();
    assert!(Command::new(std::env::var("TEST_ARCCTL").unwrap()).args(["exec", "--", "bun", "test", "--timeout=60000"]).current_dir(std::env::var("BUN_DIR").unwrap()).status().unwrap().success());
}
"#
        } else {
            r#"
#[test] fn child() {
    use std::{fs::OpenOptions, io::Write, thread, time::Duration};
    writeln!(OpenOptions::new().create(true).append(true).open(std::env::var("CHILD_MARKER").unwrap()).unwrap(), "child").unwrap();
    while !std::path::Path::new(&std::env::var("CHILD_RELEASE").unwrap()).exists() { thread::sleep(Duration::from_millis(10)); }
}
"#
        };
        fs::write(cargo.join("src/lib.rs"), cargo_source).unwrap();
        let bun_source = if parent == "bun" {
            r#"
import {test, expect} from 'bun:test'; import {appendFileSync} from 'node:fs';
test('root', async () => {
  appendFileSync(process.env.ROOT_MARKER!, 'root\n');
  const child = Bun.spawn([process.env.TEST_ARCCTL!, 'exec', '--', 'cargo', 'test', '--offline'], {cwd:process.env.CARGO_DIR!, stdout:'ignore', stderr:'ignore'});
  expect(await child.exited).toBe(0);
}, 60000);
"#
        } else {
            r#"
import {test} from 'bun:test'; import {appendFileSync,existsSync} from 'node:fs';
test('child', async () => {
  appendFileSync(process.env.CHILD_MARKER!, 'child\n');
  while (!existsSync(process.env.CHILD_RELEASE!)) await Bun.sleep(10);
}, 60000);
"#
        };
        fs::write(bun.join("nested.test.ts"), bun_source).unwrap();
        let root_marker = f.root.join("root-marker");
        let child_marker = f.root.join("child-marker");
        let release = f.root.join("child-release");
        let mut command = Command::new(binary("arcctl"));
        command.args(["exec", "--", parent, "test"]);
        if parent == "cargo" {
            command.arg("--offline");
        } else {
            command.arg("--timeout=60000");
        }
        let child = command
            .current_dir(if parent == "cargo" { &cargo } else { &bun })
            .env(arc_core::ARC_SOCKET_PATH, &f.socket)
            .env("TEST_ARCCTL", binary("arcctl"))
            .env("CARGO_DIR", &cargo)
            .env("BUN_DIR", &bun)
            .env("CARGO_TARGET_DIR", cargo.join("target"))
            .env("ROOT_MARKER", &root_marker)
            .env("CHILD_MARKER", &child_marker)
            .env("CHILD_RELEASE", &release)
            .env_remove("ARC_TRAFFIC_LINEAGE")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::from(
                File::create(f.root.join("nested-stderr")).unwrap(),
            ))
            .spawn()
            .unwrap();
        let mut root = BuildChild::new(OwnedChild::new(child), &f, true);
        eventually(|| child_marker.exists());
        eventually(|| {
            database(&f).query_row("SELECT count(*) FROM traffic_entries WHERE root_job_id IS NOT NULL AND state='RUNNING'", [], |r| r.get::<_, i64>(0)).unwrap() == 1
        });
        let db = database(&f);
        let (nested, permits): (i64, i64) = db.query_row("SELECT (SELECT count(*) FROM traffic_entries WHERE root_job_id IS NOT NULL AND state='RUNNING'), (SELECT count(*) FROM permits WHERE state='ACTIVE')", [], |r| Ok((r.get(0)?,r.get(1)?))).unwrap();
        assert_eq!((nested, permits), (1, 1));
        drop(db);
        fs::write(release, "release").unwrap();
        finish(&mut root);
        assert_eq!(fs::read_to_string(root_marker).unwrap().lines().count(), 1);
        assert_eq!(fs::read_to_string(child_marker).unwrap().lines().count(), 1);
        assert_eq!(active(&f).1, 0);
        daemon.stop(libc::SIGTERM);
        f.report();
    }
}

#[test]
fn bun_probe_rejects_malformed_or_mismatched_replies_and_lost_reply_fails_open_once() {
    let _serial = support::serial();
    for mode in [
        "wrong-id",
        "wrong-version",
        "extra-field",
        "wrong-capability",
        "lost",
    ] {
        let f = Fixture::new();
        let listener = UnixListener::bind(&f.socket).unwrap();
        let peer = thread::spawn(move || {
            for _ in 0..if mode == "lost" { 3 } else { 1 } {
                let (mut stream, _) = listener.accept().unwrap();
                let request: arc_ipc::Request = arc_ipc::read_message(&mut stream).unwrap();
                assert_eq!(
                    request.operation,
                    arc_ipc::traffic::OPERATION_BUN_CAPABILITIES
                );
                assert!(request.payload.is_null());
                if mode == "lost" {
                    continue;
                }
                let mut value = serde_json::json!({"capability":"traffic.bun.v1"});
                if mode == "extra-field" {
                    value["extra"] = serde_json::json!(true);
                }
                if mode == "wrong-capability" {
                    value["capability"] = serde_json::json!("traffic.v1");
                }
                let mut response = arc_ipc::Response::ok(&request.request_id, value);
                if mode == "wrong-id" {
                    response.request_id = "another-request".into();
                }
                if mode == "wrong-version" {
                    response.protocol_version = 2;
                }
                arc_ipc::write_response(&mut stream, &response).unwrap();
            }
        });
        symlink(binary("arc-exec-fixture"), f.root.join("bun")).unwrap();
        let marker = f.root.join("probe-marker");
        let output = Command::new(binary("arcctl"))
            .args(["exec", "--", "bun", "test"])
            .env(arc_core::ARC_SOCKET_PATH, &f.socket)
            .env("PATH", &f.root)
            .env("ARC_FIXTURE_MODE", "echo")
            .env("ARC_FIXTURE_MARKER", &marker)
            .env_remove("ARC_TRAFFIC_LINEAGE")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        peer.join().unwrap();
        if mode == "lost" {
            assert_eq!(output.status.code(), Some(17));
            assert_eq!(fs::read_to_string(marker).unwrap().lines().count(), 1);
            assert!(String::from_utf8_lossy(&output.stderr).contains("TRACKING_UNCERTAIN"));
        } else {
            assert!(!output.status.success());
            assert!(!marker.exists());
        }
    }
}

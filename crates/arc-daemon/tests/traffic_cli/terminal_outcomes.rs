use super::*;
use arc_ipc::jobs::{JobId, StatusRequest, StatusResult, OPERATION_JOB_STATUS};
use arc_ipc::traffic::JobRequest;

pub(super) fn stored_record(f: &Fixture, id: &str) -> Vec<rusqlite::types::Value> {
    let db = database(f);
    let mut statement = db
        .prepare("SELECT j.*, l.* FROM jobs j JOIN job_lifecycle l ON l.job_id=j.id WHERE j.id=?1")
        .unwrap();
    let column_count = statement.column_count();
    statement
        .query_row([id], |row| {
            (0..column_count).map(|index| row.get(index)).collect()
        })
        .unwrap()
}

pub(super) fn status(f: &Fixture, id: Option<&str>) -> String {
    let mut command = Command::new(binary("arcctl"));
    command.args(["job", "status"]);
    if let Some(id) = id {
        command.arg(id);
    }
    let output = command
        .env(arc_core::ARC_SOCKET_PATH, &f.socket)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    String::from_utf8(output.stdout).unwrap()
}

pub(super) fn assert_state(text: &str, id: &str, state: &str) {
    let record = text
        .split("Job: ")
        .find(|block| block.starts_with(id))
        .unwrap_or_else(|| panic!("missing {id}: {text}"));
    assert!(record.contains(&format!("State: {state}\n")), "{record}");
}

pub(super) fn restart_daemon(f: &Fixture) -> OwnedChild {
    fs::rename(
        f.root.join("runtime.json"),
        f.root.join("prior-runtime.json"),
    )
    .unwrap();
    f.spawn("test")
}

fn run_exit_and_restart(scheduled: bool, code: u8) {
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let marker = f.root.join("launches");
    let mut command = Command::new(binary("arcctl"));
    command.args(["exec", "--"]);
    if scheduled {
        symlink(binary("arc-exec-fixture"), f.root.join("cargo")).unwrap();
        command.args(["cargo", "build"]).env("PATH", &f.root);
    } else {
        command.arg(binary("arc-exec-fixture"));
    }
    let output = command
        .env(arc_core::ARC_SOCKET_PATH, &f.socket)
        .env(
            "ARC_FIXTURE_MODE",
            if code == 0 { "success" } else { "echo" },
        )
        .env("ARC_FIXTURE_MARKER", &marker)
        .env_remove("ARC_TRAFFIC_LINEAGE")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(i32::from(code)), "{output:?}");
    assert_eq!(output.stderr, b"fixture-stderr\n");
    assert_eq!(fs::read_to_string(&marker).unwrap().lines().count(), 1);
    let id: String = database(&f)
        .query_row("SELECT job_id FROM job_lifecycle", [], |row| row.get(0))
        .unwrap();
    assert_eq!(active(&f).1, 0);
    if scheduled {
        let (state, released): (String, bool) = database(&f)
            .query_row(
                "SELECT state, released_at IS NOT NULL FROM permits WHERE job_id=?1",
                [&id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_ne!(state, "ACTIVE");
        assert!(released);
    }
    daemon.stop(libc::SIGTERM);
    f.report();
    let mut restarted = restart_daemon(&f);
    let (state, stored_code, kind): (String, i64, String) = database(&f).query_row(
        "SELECT j.state,j.exit_code,l.outcome_kind FROM jobs j JOIN job_lifecycle l ON l.job_id=j.id WHERE j.id=?1",
        [&id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap();
    let expected = if code == 0 { "succeeded" } else { "failed" };
    assert_eq!(state, expected.to_ascii_uppercase());
    assert_eq!(stored_code, i64::from(code));
    assert_eq!(kind, "EXITED");
    let recent = status(&f, None);
    let detail = status(&f, Some(&id));
    assert_state(&recent, &id, expected);
    assert_state(&detail, &id, expected);
    assert!(detail.contains(&format!("Exit code: {code}\n")), "{detail}");

    // The old protocol-1 response still contains a closed COMPLETED phase.
    let mut request = arc_ipc::Request::new("old-client", OPERATION_JOB_STATUS);
    request.payload = serde_json::to_value(StatusRequest::ById {
        job_id: JobId::try_from(id).unwrap(),
    })
    .unwrap();
    let response = arc_ipc::exchange(&mut f.connect(), &request).unwrap();
    let wire = response.result.unwrap();
    assert_eq!(wire["job"]["phase"], "COMPLETED");
    let decoded: StatusResult = serde_json::from_value(wire).unwrap();
    assert!(decoded.is_valid());
    restarted.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn scheduled_exit_17_persists_failed_across_restart() {
    let _serial = support::serial();
    run_exit_and_restart(true, 17);
}

#[test]
fn lifecycle_exit_17_persists_failed_across_restart() {
    let _serial = support::serial();
    run_exit_and_restart(false, 17);
}

#[test]
fn zero_exit_succeeds_after_restart_for_both_paths() {
    let _serial = support::serial();
    run_exit_and_restart(true, 0);
    run_exit_and_restart(false, 0);
}

#[test]
fn recent_fifty_lifecycle_exits_are_resolved_with_the_shared_deadline() {
    use arc_ipc::jobs::{CompleteRequest, Outcome, ProcessIdentity, StartRequest};
    let _serial = support::serial();
    let f = Fixture::new();
    let mut db = arc_storage::open_and_migrate(&f.database).unwrap();
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let classification = serde_json::from_value(serde_json::json!({
        "classification":"UNKNOWN", "locality":"UNKNOWN",
        "reason":"UNSUPPORTED_SYNTAX", "uncertainty":"OBSERVE"
    }))
    .unwrap();
    for n in 1..=51 {
        let job_id = JobId::try_from(format!("{n:032x}")).unwrap();
        for request in [
            StartRequest::Register {
                job_id: job_id.clone(),
                observer: observer.clone(),
                classification,
            },
            StartRequest::Claim {
                job_id: job_id.clone(),
                observer: observer.clone(),
            },
        ] {
            let evidence = arc_storage::jobs::StartEvidence::gather(&request);
            arc_storage::jobs::start(&mut db, &request, &evidence, 100).unwrap();
        }
        arc_storage::jobs::complete(
            &mut db,
            &CompleteRequest {
                job_id,
                observer: observer.clone(),
                outcome: Outcome::Exited {
                    code: if n % 2 == 0 { 0 } else { 17 },
                },
            },
            101,
        )
        .unwrap();
    }
    drop(db);
    let mut daemon = f.spawn("test");
    let text = status(&f, None);
    assert_eq!(text.matches("Job: ").count(), 50, "{text}");
    assert!(
        text.contains("Truncated: showing the 50 most recent records."),
        "{text}"
    );
    for n in 2..=51 {
        assert_state(
            &text,
            &format!("{n:032x}"),
            if n % 2 == 0 { "succeeded" } else { "failed" },
        );
    }
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn slow_completed_lookup_shares_one_deadline_across_recent_rows() {
    use std::{
        os::unix::net::{UnixListener, UnixStream},
        sync::{
            atomic::{AtomicBool, AtomicUsize, Ordering},
            Arc,
        },
    };
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    for n in 0..2 {
        let output = Command::new(binary("arcctl"))
            .args(["exec", "--"])
            .arg(binary("arc-exec-fixture"))
            .env(arc_core::ARC_SOCKET_PATH, &f.socket)
            .env("ARC_FIXTURE_MODE", "echo")
            .env(
                "ARC_FIXTURE_MARKER",
                f.root.join(format!("slow-marker-{n}")),
            )
            .env_remove("ARC_TRAFFIC_LINEAGE")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(17));
    }
    let ids: Vec<String> = database(&f)
        .prepare("SELECT id FROM jobs ORDER BY id")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let proxy_path = f.root.join("slow-lifecycle.sock");
    let listener = UnixListener::bind(&proxy_path).unwrap();
    listener.set_nonblocking(true).unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let lookups = Arc::new(AtomicUsize::new(0));
    let server_stop = stop.clone();
    let server_lookups = lookups.clone();
    let backend = f.socket.clone();
    let server = thread::spawn(move || {
        while !server_stop.load(Ordering::SeqCst) {
            let (mut stream, _) = match listener.accept() {
                Ok(pair) => pair,
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(1));
                    continue;
                }
                Err(e) => panic!("proxy accept: {e}"),
            };
            stream.set_nonblocking(false).unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let request: arc_ipc::Request = arc_ipc::read_message(&mut stream).unwrap();
            assert_eq!(request.operation, OPERATION_JOB_STATUS);
            if request.payload["query"] == "BY_ID" {
                server_lookups.fetch_add(1, Ordering::SeqCst);
                thread::sleep(Duration::from_millis(400));
            }
            let response =
                arc_ipc::exchange(&mut UnixStream::connect(&backend).unwrap(), &request).unwrap();
            let _ = arc_ipc::write_response(&mut stream, &response);
        }
    });
    let started = Instant::now();
    let output = Command::new(binary("arcctl"))
        .args(["job", "status"])
        .env(arc_core::ARC_SOCKET_PATH, &proxy_path)
        .output()
        .unwrap();
    assert!(started.elapsed() < Duration::from_secs(5));
    stop.store(true, Ordering::SeqCst);
    server.join().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(lookups.load(Ordering::SeqCst), 1);
    let text = String::from_utf8(output.stdout).unwrap();
    for id in ids {
        assert_state(&text, &id, "outcome unknown");
    }
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn failed_completion_retry_releases_one_permit() {
    use arc_ipc::jobs::{Outcome, ProcessIdentity};
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let id = JobId::try_from("b123456789abcdef0123456789abcdef".to_owned()).unwrap();
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let enqueue = JobRequest::Enqueue {
        job_id: id.clone(),
        observer: observer.clone(),
        classification: cargo_class("BUILD", "CARGO_BUILD"),
        lineage_context: None,
    };
    let arc_ipc::traffic::JobResult::Granted {
        permit_generation, ..
    } = traffic_result(traffic(&f, &enqueue))
    else {
        panic!("grant")
    };
    let complete = JobRequest::Complete {
        job_id: id.clone(),
        observer,
        permit_generation,
        outcome: Outcome::Exited { code: 17 },
    };
    let mut request =
        arc_ipc::Request::new("lost-completion", arc_ipc::traffic::OPERATION_TRAFFIC_JOB);
    request.payload = serde_json::to_value(&complete).unwrap();
    let mut lost = f.connect();
    arc_ipc::write_message(&mut lost, &request).unwrap();
    eventually(|| active(&f).1 == 0);
    drop(lost);
    let first = traffic_result(traffic(&f, &complete));
    assert_eq!(traffic_result(traffic(&f, &complete)), first);
    assert!(matches!(
        first,
        arc_ipc::traffic::JobResult::Completed {
            outcome: Outcome::Exited { code: 17 },
            ..
        }
    ));
    let (state, code, permits, active_permits): (String, i64, i64, i64) = database(&f).query_row(
        "SELECT state, exit_code, (SELECT count(*) FROM permits), (SELECT count(*) FROM permits WHERE state='ACTIVE') FROM jobs WHERE id=?1",
        [id.as_str()], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))).unwrap();
    assert_eq!(
        (state.as_str(), code, permits, active_permits),
        ("FAILED", 17, 1, 0)
    );
    daemon.stop(libc::SIGTERM);
    f.report();
}

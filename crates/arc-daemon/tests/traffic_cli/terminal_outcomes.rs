use super::*;
use arc_ipc::jobs::{JobId, StatusRequest, StatusResult, OPERATION_JOB_STATUS};
use arc_ipc::traffic::JobRequest;

#[path = "status_clock.rs"]
mod status_clock;
#[path = "terminal_barrier.rs"]
mod terminal_barrier;

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
    command
        .env(arc_core::ARC_SOCKET_PATH, &f.socket)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = OwnedChild::new(command.spawn().unwrap());
    let output = child.wait_with_output().expect("status command watchdog");
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    String::from_utf8(output.stdout).unwrap()
}

fn forward_profile(command: &mut Command) {
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile);
    }
}

fn start_clock_fixture_daemon(f: &Fixture) -> OwnedChild {
    let mut command = Command::new(binary("arc-daemon"));
    command.env_clear();
    f.configure(&mut command);
    command.env("ARC_IPC_PROFILE", "test");
    forward_profile(&mut command);
    let mut daemon = OwnedChild::new(command.spawn().unwrap());
    f.wait_for(&mut daemon, "arc-daemon ready.");
    daemon
}

fn stop_clock_fixture_daemon(daemon: &mut OwnedChild) {
    assert_eq!(
        unsafe { libc::kill(daemon.child.id() as i32, libc::SIGTERM) },
        0
    );
    let output = daemon.wait_with_output().expect("daemon exit watchdog");
    assert!(output.status.success(), "{output:?}");
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
    let prepared = if scheduled {
        let barrier = terminal_barrier::Barrier::prepare(&f, &mut command);
        command.args(["cargo", "build"]).env("PATH", &f.root);
        Some(barrier)
    } else {
        command.arg(binary("arc-exec-fixture"));
        None
    };
    command
        .env(arc_core::ARC_SOCKET_PATH, &f.socket)
        .env(
            "ARC_FIXTURE_MODE",
            if code == 0 { "success" } else { "echo" },
        )
        .env("ARC_FIXTURE_MARKER", &marker)
        .env_remove("ARC_TRAFFIC_LINEAGE")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = OwnedChild::new(command.spawn().unwrap());
    // Drop the release before the child guard on unwind, so a blocked target
    // can exit and its owned identity can be cleaned up.
    let mut barrier = prepared;
    if let Some(barrier) = &mut barrier {
        barrier.wait_for_attachment(&f, &mut child);
        barrier.release();
    }
    let output = child.wait_with_output().expect("terminal fixture watchdog");
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
    let mut daemon = start_clock_fixture_daemon(&f);
    let clock = f.root.join("status-clock");
    fs::write(&clock, "0").unwrap();
    let mut status = status_clock::spawn(&f, &f.socket, &clock);
    let output = status
        .wait_with_output()
        .expect("recent-fifty status watchdog");
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    assert!(
        f.root.join("clock-initial").exists(),
        "clock was not interposed"
    );
    let text = String::from_utf8(output.stdout).unwrap();
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
    stop_clock_fixture_daemon(&mut daemon);
    f.report();
}

#[test]
fn slow_completed_lookup_shares_one_deadline_across_recent_rows() {
    use std::{
        io::Read,
        os::fd::AsRawFd,
        os::unix::net::{UnixListener, UnixStream},
    };
    #[derive(Debug)]
    struct RecordedJob {
        id: String,
        state: String,
        exit_code: Option<i64>,
        outcome: Option<String>,
        finished_at: Option<i64>,
    }
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = start_clock_fixture_daemon(&f);
    for n in 0..2 {
        let mut command = Command::new(binary("arcctl"));
        command
            .env_clear()
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
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        forward_profile(&mut command);
        let mut child = OwnedChild::new(command.spawn().unwrap());
        let output = child
            .wait_with_output()
            .expect("fixture execution watchdog");
        assert_eq!(output.status.code(), Some(17));
    }
    let db = database(&f);
    let count: i64 = db
        .query_row("SELECT count(*) FROM jobs", [], |row| row.get(0))
        .unwrap();
    assert_eq!(count, 2, "both executions must have recorded jobs");
    let records: Vec<RecordedJob> = db
        .prepare("SELECT j.id,j.state,j.exit_code,l.outcome_kind,j.finished_at FROM jobs j JOIN job_lifecycle l ON l.job_id=j.id ORDER BY l.registered_at_ms DESC,l.job_id DESC")
        .unwrap()
        .query_map([], |row| Ok(RecordedJob {
            id: row.get(0)?,
            state: row.get(1)?,
            exit_code: row.get(2)?,
            outcome: row.get(3)?,
            finished_at: row.get(4)?,
        }))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(records.len(), 2, "both jobs must have lifecycle records");
    assert_ne!(records[0].id, records[1].id, "jobs must be distinct");
    for record in &records {
        assert_eq!(record.state, "FAILED");
        assert_eq!(record.exit_code, Some(17));
        assert_eq!(record.outcome.as_deref(), Some("EXITED"));
        assert!(record.finished_at.is_some(), "job must be completed");
    }
    let ids: Vec<String> = records.into_iter().map(|record| record.id).collect();
    drop(db);
    let proxy_path = f.root.join("slow-lifecycle.sock");
    let listener = UnixListener::bind(&proxy_path).unwrap();
    let backend = f.socket.clone();
    let clock = f.root.join("slow-status-clock");
    fs::write(&clock, "0").unwrap();
    let server_clock = clock.clone();
    let first_id = ids[0].clone();
    let (finished, status_finished) = std::sync::mpsc::channel();
    let server = thread::spawn(move || {
        let accept = || {
            let mut poll = libc::pollfd {
                fd: listener.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            };
            loop {
                let result = unsafe { libc::poll(&mut poll, 1, 60_000) };
                if result < 0
                    && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted
                {
                    continue;
                }
                assert!(result > 0, "status proxy never received the next request");
                break;
            }
            let (stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(60)))
                .unwrap();
            stream
        };
        let mut recent = accept();
        let request: arc_ipc::Request = arc_ipc::read_message(&mut recent).unwrap();
        assert_eq!(request.operation, OPERATION_JOB_STATUS);
        assert_eq!(request.payload["query"], "RECENT");
        let mut daemon = UnixStream::connect(&backend).unwrap();
        daemon
            .set_read_timeout(Some(Duration::from_secs(60)))
            .unwrap();
        daemon
            .set_write_timeout(Some(Duration::from_secs(60)))
            .unwrap();
        recent
            .set_write_timeout(Some(Duration::from_secs(60)))
            .unwrap();
        let response = arc_ipc::exchange(&mut daemon, &request).unwrap();
        arc_ipc::write_response(&mut recent, &response).unwrap();
        drop(recent);

        let mut lookup = accept();
        let request: arc_ipc::Request = arc_ipc::read_message(&mut lookup).unwrap();
        assert_eq!(request.operation, OPERATION_JOB_STATUS);
        assert_eq!(request.payload["query"], "BY_ID");
        assert_eq!(request.payload["job_id"], first_id);
        // Advance only after the first lookup is observed. A partial frame
        // wakes the poll so the client rechecks the controlled deadline.
        let pending = server_clock.with_extension("pending");
        fs::write(&pending, "250").unwrap();
        fs::rename(pending, server_clock).unwrap();
        use std::io::Write;
        match lookup.write_all(&[0]) {
            Ok(()) => (),
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::ConnectionReset
                ) =>
            {
                // The client observed the advanced clock before the wakeup.
            }
            Err(error) => panic!("cannot wake status lookup: {error}"),
        }
        match lookup.read(&mut [0u8]) {
            Ok(0) => (),
            Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => (),
            result => panic!("client did not close expired lookup: {result:?}"),
        }
        status_finished
            .recv_timeout(Duration::from_secs(60))
            .expect("status client never exited after expiry");
        listener.set_nonblocking(true).unwrap();
        match listener.accept() {
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => (),
            result => panic!("unexpected additional status connection: {result:?}"),
        }
    });
    let mut child = status_clock::spawn(&f, &proxy_path, &clock);
    let output = child.wait_with_output().expect("expired status watchdog");
    finished.send(()).unwrap();
    server.join().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    assert!(
        f.root.join("clock-initial").exists(),
        "initial clock was not observed"
    );
    assert!(
        f.root.join("clock-advanced").exists(),
        "advanced clock was not observed"
    );
    let text = String::from_utf8(output.stdout).unwrap();
    let actual: Vec<_> = text
        .lines()
        .filter_map(|line| line.strip_prefix("Job: ").map(str::to_owned))
        .collect();
    assert_eq!(actual, ids);
    for id in ids {
        assert_state(&text, &id, "outcome unknown");
    }
    stop_clock_fixture_daemon(&mut daemon);
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

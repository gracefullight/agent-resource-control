//! Forced termination on both sides of the real SQLite worker's COMMIT.
//!
//! The owned recovery fixture runs production startup and pauses its writer
//! twice for one lifecycle start: inside the commit hook, and after the start
//! committed but before its result is sent. Each test checks the stage it was
//! told about from an independent connection, then sends SIGKILL to the daemon
//! while the writer is still held there. Nothing here waits on elapsed time.
use super::{
    cli::wait,
    protocol,
    runtime::{self, Fixture, OwnedChild},
    support,
};
use arc_ipc::{
    jobs::{
        StartDisposition, StartResult, StatusRequest, StatusResult, OPERATION_JOB_START,
        OPERATION_JOB_STATUS,
    },
    DoctorReport, Request, Response, OPERATION_DOCTOR,
};
use serde_json::json;
use std::{
    io::{BufRead, BufReader, ErrorKind, Write},
    os::unix::{
        net::{UnixListener, UnixStream},
        process::ExitStatusExt,
    },
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

const WATCHDOG: Duration = Duration::from_secs(60);

fn fixture() -> Fixture {
    Fixture::with_binary(PathBuf::from(env!(
        "CARGO_BIN_EXE_arc-recovery-daemon-fixture"
    )))
}

/// The socket on which the fixture's paused writer announces each stage.
fn barrier(f: &Fixture) -> UnixListener {
    let listener = UnixListener::bind(f.root.join("commit.sock")).unwrap();
    listener.set_nonblocking(true).unwrap();
    listener
}

fn spawn_with_barrier(f: &Fixture, listener: &UnixListener) -> OwnedChild {
    let mut command = f.isolated_command("test");
    command
        .arg("--fixture-commit-barrier")
        .arg(f.root.join("commit.sock"));
    let mut daemon = OwnedChild::new(command.spawn().unwrap());
    f.wait_for(&mut daemon, "arc-daemon ready.");
    // Startup and migration never reach the fixture; only a start does.
    assert_idle(listener);
    daemon
}

fn assert_idle(listener: &UnixListener) {
    match listener.accept() {
        Err(error) if error.kind() == ErrorKind::WouldBlock => (),
        other => panic!("commit barrier reached without a lifecycle start: {other:?}"),
    }
}

/// Send one registration without reading its reply.
fn send(f: &Fixture, n: u32) -> UnixStream {
    let mut request = Request::new("recovery-fixture", OPERATION_JOB_START);
    request.payload = json!(protocol::register(n));
    let mut stream = f.connect();
    runtime::send(&mut stream, &request);
    stream
}

fn acknowledged(mut stream: UnixStream) -> StartResult {
    let response: Response = arc_ipc::read_message(&mut stream).unwrap();
    assert_eq!(response.status, "ok");
    serde_json::from_value(response.result.unwrap()).unwrap()
}

/// Block until the writer announces `stage`. It stays paused while the
/// returned stream is open and unreleased.
fn reached(listener: &UnixListener, stage: &str) -> UnixStream {
    let deadline = Instant::now() + WATCHDOG;
    let (gate, _) = loop {
        match listener.accept() {
            Ok(accepted) => break accepted,
            Err(error) if error.kind() == ErrorKind::WouldBlock => {
                assert!(Instant::now() < deadline, "writer never reached {stage}");
                thread::sleep(Duration::from_millis(1));
            }
            Err(error) => panic!("commit barrier: {error}"),
        }
    };
    gate.set_nonblocking(false).unwrap();
    gate.set_read_timeout(Some(WATCHDOG)).unwrap();
    let mut announced = String::new();
    BufReader::new(&gate).read_line(&mut announced).unwrap();
    assert_eq!(announced.trim_end(), stage);
    gate
}

fn release(mut gate: UnixStream) {
    gate.write_all(b"\n").unwrap();
}

/// What any other process can read from the database at this instant.
fn observer(f: &Fixture) -> rusqlite::Connection {
    let connection = rusqlite::Connection::open(&f.database).unwrap();
    connection.busy_timeout(Duration::ZERO).unwrap();
    connection
}

fn rows(connection: &rusqlite::Connection, n: u32) -> i64 {
    connection
        .query_row(
            "SELECT count(*) FROM jobs WHERE id = ?1",
            [protocol::id(n).as_str()],
            |row| row.get(0),
        )
        .unwrap()
}

fn committed(f: &Fixture, n: u32) -> bool {
    rows(&observer(f), n) == 1
}

/// Whether the daemon's writer is inside an open write transaction.
fn writer_in_transaction(f: &Fixture) -> bool {
    let connection = observer(f);
    match connection.execute_batch("BEGIN IMMEDIATE") {
        Ok(()) => {
            connection.execute_batch("ROLLBACK").unwrap();
            false
        }
        Err(error) if error.sqlite_error_code() == Some(rusqlite::ErrorCode::DatabaseBusy) => true,
        Err(error) => panic!("write lock probe: {error}"),
    }
}

/// Run one start through both stages and its reply, checking that the two
/// stages bracket exactly the commit that makes the row visible.
fn commit_through_barrier(f: &Fixture, listener: &UnixListener, n: u32) {
    let stream = send(f, n);
    let before = reached(listener, "before-commit");
    assert!(writer_in_transaction(f));
    assert!(!committed(f, n));
    release(before);
    let after = reached(listener, "after-commit");
    assert!(!writer_in_transaction(f));
    assert!(committed(f, n));
    release(after);
    assert_eq!(acknowledged(stream).disposition, StartDisposition::Created);
}

/// SIGKILL the daemon whose writer `gate` still holds, then prove the client
/// was never answered.
fn kill_at(daemon: &mut OwnedChild, gate: UnixStream, client: UnixStream) {
    daemon.child.kill().unwrap();
    assert_eq!(wait(&mut daemon.child).signal(), Some(libc::SIGKILL));
    drop(gate);
    runtime::assert_closed(client);
}

fn retire_report(f: &Fixture, name: &str) {
    std::fs::rename(f.root.join("runtime.json"), f.root.join(name)).unwrap();
}

/// Restart the owned fixture daemon on the database and WAL the killed writer
/// left behind, and check integrity through the daemon and independently.
fn restart(f: &Fixture) -> OwnedChild {
    retire_report(f, "crashed-runtime.json");
    let restarted = f.spawn_isolated("test");
    let request = Request::new("recovery-fixture", OPERATION_DOCTOR);
    let response = arc_ipc::exchange(&mut f.connect(), &request).unwrap();
    assert_eq!(response.status, "ok");
    let report: DoctorReport = serde_json::from_value(response.result.unwrap()).unwrap();
    assert_eq!(report.integrity_check, "ok");
    let connection = observer(f);
    for (pragma, expected) in [("integrity_check", "ok"), ("journal_mode", "wal")] {
        let value: String = connection
            .query_row(&format!("PRAGMA {pragma}"), [], |row| row.get(0))
            .unwrap();
        assert_eq!(value, expected, "{pragma}");
    }
    restarted
}

/// The restarted daemon's own answer for one job.
fn recovered(f: &Fixture, n: u32) -> bool {
    let request = StatusRequest::ById {
        job_id: protocol::id(n),
    };
    let response = protocol::call(f, OPERATION_JOB_STATUS, json!(request));
    assert_eq!(response.status, "ok");
    let found = match serde_json::from_value(response.result.unwrap()).unwrap() {
        StatusResult::Found { .. } => true,
        StatusResult::NotFound {} => false,
        other => panic!("status by id: {other:?}"),
    };
    assert_eq!(found, committed(f, n));
    found
}

#[test]
fn forced_daemon_death_before_commit_rolls_back_and_integrity_ok() {
    let _serial = support::serial();
    let f = fixture();
    let listener = barrier(&f);
    let mut daemon = spawn_with_barrier(&f, &listener);
    // Job 700 commits in the daemon that is about to die, so it is still in
    // that daemon's WAL when job 701 is cut off inside its commit hook.
    commit_through_barrier(&f, &listener, 700);
    let client = send(&f, 701);
    let before = reached(&listener, "before-commit");
    assert!(writer_in_transaction(&f));
    assert!(committed(&f, 700));
    assert!(!committed(&f, 701));
    kill_at(&mut daemon, before, client);

    let mut restarted = restart(&f);
    assert!(recovered(&f, 700));
    assert!(!recovered(&f, 701));
    // Nothing of the lost transaction blocks registering the same job again.
    let retried = protocol::start(&f, &protocol::register(701));
    assert_eq!(retried.disposition, StartDisposition::Created);
    restarted.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn forced_daemon_death_after_commit_survives_and_integrity_ok() {
    let _serial = support::serial();
    let f = fixture();
    let listener = barrier(&f);
    // A first daemon commits job 703 and stops normally, which checkpoints the
    // schema and that job into the main database file.
    let mut prior = spawn_with_barrier(&f, &listener);
    commit_through_barrier(&f, &listener, 703);
    prior.stop(libc::SIGTERM);
    f.report();
    retire_report(&f, "prior-runtime.json");

    let mut daemon = spawn_with_barrier(&f, &listener);
    let client = send(&f, 702);
    let before = reached(&listener, "before-commit");
    assert!(writer_in_transaction(&f));
    assert!(!committed(&f, 702));
    release(before);
    let after = reached(&listener, "after-commit");
    assert!(!writer_in_transaction(&f));
    assert!(committed(&f, 702));
    // The commit exists only in the WAL: the main file alone still lacks it.
    let main_only = f.root.join("main-only.sqlite3");
    std::fs::copy(&f.database, &main_only).unwrap();
    let copy = rusqlite::Connection::open(&main_only).unwrap();
    assert_eq!((rows(&copy, 703), rows(&copy, 702)), (1, 0));
    drop(copy);
    let wal = std::fs::metadata(f.root.join("arc.sqlite3-wal")).unwrap();
    assert!(wal.len() > 0);
    kill_at(&mut daemon, after, client);

    let mut restarted = restart(&f);
    assert!(recovered(&f, 703));
    assert!(recovered(&f, 702));
    // The unanswered client retries by key and finds its committed job.
    let retried = protocol::start(&f, &protocol::register(702));
    assert_eq!(retried.disposition, StartDisposition::Unchanged);
    restarted.stop(libc::SIGTERM);
    f.report();
}

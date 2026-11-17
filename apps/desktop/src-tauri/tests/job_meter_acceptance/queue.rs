//! Queue accounting through the production daemon and arcctl binaries.
use super::{daemon_bridge_support::RealDaemon, native, serialization, support::*};
use arc_core::process_identity::{probe_live, IdentityStatus};
use arc_ipc::jobs::Phase;
use serde_json::{json, Value};
use std::{
    fs,
    io::{self, Read, Write},
    os::unix::{
        net::{UnixListener, UnixStream},
        process::CommandExt,
    },
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

const WATCHDOG: Duration = Duration::from_secs(60);

#[derive(Debug)]
enum WaitFailure {
    PeerExited(ExitStatus),
    CheckFailed(io::Error),
    TimedOut,
}

fn check_progress(child: &mut Child, deadline: Instant, now: Instant) -> Result<(), WaitFailure> {
    match child.try_wait().map_err(WaitFailure::CheckFailed)? {
        Some(status) => Err(WaitFailure::PeerExited(status)),
        None if now >= deadline => Err(WaitFailure::TimedOut),
        None => Ok(()),
    }
}

fn keep_waiting(run: &mut Run, deadline: Instant, observation: &str) {
    if let Err(failure) = check_progress(&mut run.wrapper.child, deadline, Instant::now()) {
        let reason = match failure {
            WaitFailure::PeerExited(status) => format!("wrapper exited ({status})"),
            WaitFailure::CheckFailed(error) => format!("cannot check wrapper exit: {error}"),
            WaitFailure::TimedOut => "watchdog expired".to_owned(),
        };
        panic!(
            "{reason} while waiting for {observation} (wrapper {}): {}",
            run.identity.pid,
            run.errors()
        );
    }
    thread::yield_now();
}

// The stream is nonblocking so every incomplete readiness read checks both
// peer exit and the same watchdog. A partial message never resets the guard.
fn read_readiness(
    stream: &mut UnixStream,
    mut pending: impl FnMut() -> io::Result<()>,
) -> io::Result<()> {
    let mut ready = [0; 5];
    let mut received = 0;
    while received < ready.len() {
        match stream.read(&mut ready[received..]) {
            Ok(0) => {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    format!("control socket closed after {received} readiness bytes"),
                ));
            }
            Ok(count) => received += count,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) =>
            {
                pending()?;
            }
            Err(error) => return Err(error),
        }
    }
    if &ready != b"ready" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unexpected native readiness: {ready:?}"),
        ));
    }
    Ok(())
}

fn await_control(listener: &UnixListener, run: &mut Run) -> UnixStream {
    listener.set_nonblocking(true).unwrap();
    let deadline = Instant::now() + WATCHDOG;
    let mut stream = loop {
        match listener.accept() {
            Ok((stream, _)) => {
                stream.set_nonblocking(true).unwrap();
                break stream;
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) =>
            {
                keep_waiting(run, deadline, "native control connection");
            }
            Err(error) => panic!("native control accept: {error}"),
        }
    };
    let deadline = Instant::now() + WATCHDOG;
    read_readiness(&mut stream, || {
        keep_waiting(run, deadline, "native control readiness");
        Ok(())
    })
    .unwrap_or_else(|error| panic!("native control readiness failed: {error}; {}", run.errors()));
    stream.set_nonblocking(false).unwrap();
    stream
}

// Keep the SQLite subprocess under the same state watchdog as its caller. A
// stuck query must not hide an exited wrapper or leave an owned process behind.
fn state_rows(case: &RealDaemon, run: &mut Run, deadline: Instant, expected: &str) -> Vec<Value> {
    let stdout = case.fixture.root.join("queue-state-stdout");
    let stderr = case.fixture.root.join("queue-state-stderr");
    let mut command = Command::new("/usr/bin/sqlite3");
    command
        .env_clear()
        .env("HOME", &case.fixture.root)
        .env("TMPDIR", &case.fixture.root)
        .args(["-init", "/dev/null", "-readonly", "-json"])
        .arg(&case.fixture.database)
        .arg("SELECT t.job_id,t.state,l.observer_pid FROM traffic_entries t JOIN job_lifecycle l ON l.job_id=t.job_id")
        .stdin(Stdio::null())
        .stdout(fs::File::create(&stdout).unwrap())
        .stderr(fs::File::create(&stderr).unwrap());
    let mut query = native::OwnedChild::new(command.spawn().unwrap());
    let observation = format!("SQLite traffic state {expected}");
    let status = loop {
        if let Some(status) = query.child.try_wait().expect("check SQLite query exit") {
            break status;
        }
        keep_waiting(run, deadline, &observation);
    };
    let output = fs::read(&stdout).unwrap();
    let errors = fs::read_to_string(&stderr).unwrap();
    assert!(status.success(), "SQLite query failed: {status}; {errors}");
    assert!(errors.is_empty(), "SQLite query stderr: {errors}");
    if output.is_empty() {
        vec![]
    } else {
        serde_json::from_slice(&output).unwrap()
    }
}

fn await_state(case: &RealDaemon, run: &mut Run, expected: &str) -> Value {
    let deadline = Instant::now() + WATCHDOG;
    let observation = format!("SQLite traffic state {expected}");
    loop {
        let entries = state_rows(case, run, deadline, expected);
        if let Some(entry) = entries
            .into_iter()
            .find(|entry| entry["observer_pid"] == run.identity.pid && entry["state"] == expected)
        {
            return entry;
        }
        keep_waiting(run, deadline, &observation);
    }
}

#[test]
fn pending_waits_detect_peer_exit_and_watchdog_without_elapsed_time() {
    let mut command = Command::new("/bin/cat");
    command
        .env_clear()
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = native::OwnedChild::new(command.spawn().unwrap());
    let now = Instant::now();
    let deadline = now + WATCHDOG;
    assert!(check_progress(&mut child.child, deadline, now).is_ok());
    assert!(matches!(
        check_progress(&mut child.child, deadline, deadline),
        Err(WaitFailure::TimedOut)
    ));
    native::native_readiness::kill_and_reap(&mut child.child, false).unwrap();
    assert!(matches!(
        check_progress(&mut child.child, deadline, now),
        Err(WaitFailure::PeerExited(_))
    ));
}

#[test]
fn partial_readiness_checks_the_guard_instead_of_blocking() {
    let (mut reader, mut writer) = UnixStream::pair().unwrap();
    reader.set_nonblocking(true).unwrap();
    writer.write_all(b"re").unwrap();
    let error = read_readiness(&mut reader, || Err(io::Error::other("guard reached"))).unwrap_err();
    assert_eq!(error.to_string(), "guard reached");

    let (mut reader, mut writer) = UnixStream::pair().unwrap();
    reader.set_nonblocking(true).unwrap();
    writer.write_all(b"ready").unwrap();
    read_readiness(&mut reader, || panic!("complete readiness must not wait")).unwrap();
}

fn assert_attached(case: &RealDaemon, run: &Run, id: &str) {
    let snapshot = case.snapshot(id);
    assert_eq!(snapshot.phase, Phase::Running);
    let child = snapshot
        .child
        .expect("running job must have an attached child");
    let marker_pid: u32 = std::fs::read_to_string(&run.marker)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert_eq!(child.pid, marker_pid);
    assert_eq!(probe_live(&child), IdentityStatus::VerifiedMatch);
}

fn finish(run: &mut Run, observation: &str) -> ExitStatus {
    let deadline = Instant::now() + WATCHDOG;
    loop {
        if let Some(status) = run.wrapper.child.try_wait().expect("check wrapper exit") {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "wrapper {} never exited after {observation}: {}",
            run.identity.pid,
            run.errors()
        );
        thread::yield_now();
    }
}

#[test]
fn production_queue_ignores_poisoned_inherited_settings() {
    // Run the real acceptance test in a separate process so poisoning does
    // not mutate the environment of concurrent tests in this test binary.
    let poison = native::Fixture::with_binary(super::daemon_bridge_support::binary("arc-daemon"));
    let rc = poison.root.join(".sqliterc");
    fs::write(&rc, b".print ARC_SQLITERC_POISON\n").unwrap();
    let mut sqlite = Command::new("/usr/bin/sqlite3");
    sqlite
        .env_clear()
        .env("HOME", &poison.root)
        .args(["-init"])
        .arg(&rc)
        .args(["-json", ":memory:", "SELECT 1 AS value"]);
    let contaminated = output(sqlite, "poisoned SQLite initialization probe");
    assert!(contaminated.status.success(), "{contaminated:?}");
    assert!(String::from_utf8_lossy(&contaminated.stdout).contains("ARC_SQLITERC_POISON"));

    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .env_clear()
        .env("HOME", &poison.root)
        .env("TMPDIR", &poison.root)
        .env("PATH", "/usr/bin:/bin")
        .env("ARC_TRAFFIC_CPU_BUDGET_MILLIS", "invalid")
        .args([
            "--exact",
            "queue::production_queue_accounting_reconciles_all_surfaces",
            "--nocapture",
        ])
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(profile) = std::env::var_os("LLVM_PROFILE_FILE") {
        command.env("LLVM_PROFILE_FILE", profile);
    }
    let mut child = native::OwnedChild::new(command.spawn().unwrap());
    let result = child
        .wait_with_output_for(Duration::from_secs(600), true)
        .expect("poisoned production queue test never completed");
    let stdout = String::from_utf8_lossy(&result.stdout);
    assert!(result.status.success(), "{result:?}");
    assert!(stdout.contains("running 1 test"), "{stdout}");
    assert!(stdout.contains("1 passed; 0 failed"), "{stdout}");
}

#[test]
fn production_queue_accounting_reconciles_all_surfaces() {
    let _serial = serialization::serial();
    // setup() and command() select the actual arc-daemon and arcctl binaries;
    // only the native workload is a fixture with a test-owned control socket.
    let mut case = setup_isolated("job-meter-production-queue");
    // Even an init file at the fixture HOME must not contaminate SQLite JSON.
    fs::write(
        case.fixture.root.join(".sqliterc"),
        b".print ARC_SQLITERC_POISON\n",
    )
    .unwrap();
    let first_listener = UnixListener::bind(case.fixture.root.join("first-control")).unwrap();
    let queued_listener = UnixListener::bind(case.fixture.root.join("queued-control")).unwrap();
    let mut first_command = command(&case, true, "channel-hold");
    assert_eq!(first_command.get_program(), case.fixture.binary("arcctl"));
    first_command.env(
        "ARC_METER_TEST_CONTROL",
        case.fixture.root.join("first-control"),
    );
    let mut first = Run::launch(&case, first_command);
    let mut first_control = await_control(&first_listener, &mut first);
    let first_entry = await_state(&case, &mut first, "RUNNING");
    let first_id = first_entry["job_id"].as_str().unwrap().to_owned();
    assert_attached(&case, &first, &first_id);
    assert_eq!(first.count(), 1);

    let mut queued_command = command(&case, true, "channel-hold");
    assert_eq!(queued_command.get_program(), case.fixture.binary("arcctl"));
    queued_command.env(
        "ARC_METER_TEST_CONTROL",
        case.fixture.root.join("queued-control"),
    );
    let mut queued = Run::launch(&case, queued_command);
    let queued_entry = await_state(&case, &mut queued, "QUEUED");
    let queued_id = queued_entry["job_id"].as_str().unwrap().to_owned();
    assert!(
        !queued.marker.exists(),
        "queued child started before release"
    );
    assert!(case.snapshot(&queued_id).child.is_none());
    assert_attached(&case, &first, &first_id);

    first_control.write_all(b"r").unwrap();
    assert_eq!(
        finish(&mut first, "first release").code(),
        Some(17),
        "{}",
        first.errors()
    );
    let mut queued_control = await_control(&queued_listener, &mut queued);
    let running = await_state(&case, &mut queued, "RUNNING");
    assert_eq!(running["job_id"], queued_id);
    assert_attached(&case, &queued, &queued_id);
    assert_eq!(queued.count(), 1);
    queued_control.write_all(b"r").unwrap();
    assert_eq!(
        finish(&mut queued, "queued release").code(),
        Some(17),
        "{}",
        queued.errors()
    );
    assert_eq!(first.count(), 1);
    assert_eq!(queued.count(), 1);

    let first_surfaces = reconcile(&case, &first_id);
    let queued_surfaces = reconcile(&case, &queued_id);
    for (surfaces, observer) in [
        (&first_surfaces, &first.identity),
        (&queued_surfaces, &queued.identity),
    ] {
        let summary = &surfaces["cli"];
        let recorded = report(summary);
        assert_eq!(recorded["observer"], json!(observer));
        assert_eq!(recorded["outcome"], json!({"kind":"EXITED","code":17}));
        assert_eq!(recorded["run_elapsed"]["clock"], "monotonic");
        assert!(recorded["run_elapsed"]["seconds"].is_number());
        assert_eq!(recorded["run_elapsed"]["includes_stopped"], true);
    }
    let summary = &queued_surfaces["cli"];
    assert_eq!(summary["queue_wait"]["basis"], "queued_to_reserved");
    assert_eq!(summary["queue_wait"]["clock"], "daemon_wall_clock");
    let entries = rows(
        &case,
        "SELECT job_id,state,queued_at_ms,granted_at_ms FROM traffic_entries",
    );
    let persisted = entries
        .iter()
        .find(|row| row["job_id"] == queued_id)
        .unwrap();
    let queue_seconds = (persisted["granted_at_ms"].as_f64().unwrap()
        - persisted["queued_at_ms"].as_f64().unwrap())
        / 1000.0;
    assert_eq!(summary["queue_wait"]["seconds"], json!(queue_seconds));
    assert_eq!(entries.len(), 2);
    for id in [&first_id, &queued_id] {
        let row = entries.iter().find(|row| row["job_id"] == *id).unwrap();
        assert_eq!(row["state"], "COMPLETED");
    }
    save(
        "production_queue_accounting_reconciles_all_surfaces",
        json!({"first":first_surfaces,"queued":queued_surfaces,"trafficEntries":entries,
            "queuedChildAbsentBeforeRelease":true,"singleExecutionCounts":[first.count(),queued.count()]}),
    );
    case.stop("production-queue-done");
}

//! A live write descriptor with no reader must not interrupt child supervision.
use super::{
    cli::{command, restart, wait},
    fail_open::receipt,
    fault::Fault,
    runtime::{Fixture, OwnedChild},
    support,
};
use std::{
    fs::{self, File},
    io::{BufRead, BufReader, Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::process::ExitStatusExt,
    },
    path::Path,
    process::{Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

const TARGET: &str = "printf '%s\\n' \"$$\" >> \"$ARC_TEST_MARKER\"; printf 'target-ready\\n'; IFS= read -r value; printf 'target-input:%s\\n' \"$value\"; exit 17";

fn broken_pipe() -> Stdio {
    let mut fds = [-1; 2];
    assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
    assert_eq!(unsafe { libc::close(fds[0]) }, 0);
    assert!(unsafe { libc::fcntl(fds[1], libc::F_GETFD) } >= 0);
    // The writer remains open and is inherited as stderr, with no reader.
    unsafe { Stdio::from(File::from_raw_fd(fds[1])) }
}

fn target(f: &Fixture, wrapped: bool, socket: &Path, marker: &Path) -> Command {
    let mut c = if wrapped {
        let mut c = command(f);
        c.args(["exec", "--", "/bin/sh", "-c", TARGET]);
        c
    } else {
        let mut c = Command::new("/bin/sh");
        c.args(["-c", TARGET]);
        c
    };
    c.current_dir(&f.root)
        .env(arc_core::ARC_SOCKET_PATH, socket)
        .env("ARC_TEST_MARKER", marker)
        .env("PRIVATE_ENV", "environment-secret-sentinel")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(broken_pipe());
    c
}

fn run_target(
    f: &Fixture,
    wrapped: bool,
    socket: &Path,
    marker: &Path,
    signal: bool,
    after_ready: impl FnOnce(),
) -> (ExitStatus, Vec<u8>) {
    let mut child = OwnedChild::new(target(f, wrapped, socket, marker).spawn().unwrap());
    let mut stdout = BufReader::new(child.child.stdout.take().unwrap());
    let mut poll = libc::pollfd {
        fd: stdout.get_ref().as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    assert!(
        unsafe { libc::poll(&mut poll, 1, 15_000) } > 0,
        "target handshake timed out"
    );
    let mut ready = String::new();
    stdout.read_line(&mut ready).unwrap();
    assert_eq!(ready, "target-ready\n");
    assert_eq!(fs::read_to_string(marker).unwrap().lines().count(), 1);
    after_ready();
    assert!(
        child.child.try_wait().unwrap().is_none(),
        "wrapper stopped supervising before target completion"
    );
    if signal {
        assert_eq!(
            unsafe { libc::kill(child.child.id() as i32, libc::SIGTERM) },
            0
        );
    } else {
        child
            .child
            .stdin
            .take()
            .unwrap()
            .write_all(b"input-bytes\n")
            .unwrap();
    }
    let status = wait(&mut child.child);
    let mut output = ready.into_bytes();
    stdout.read_to_end(&mut output).unwrap();
    (status, output)
}

fn no_reservation_or_cleanup(f: &Fixture, sentinel: &Path) {
    let db = rusqlite::Connection::open(&f.database).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM permits", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        db.query_row("SELECT count(*) FROM gc_operations", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(fs::read(sentinel).unwrap(), b"untouched");
}

fn outage(mode: &str, signal: bool) {
    let f = Fixture::new();
    let mut daemon = if mode == "absent" {
        None
    } else {
        Some(f.spawn("test"))
    };
    let fault = Fault::new(&f, mode);
    let marker = f.root.join("target-calls");
    let sentinel = f.root.join("cleanup-sentinel");
    fs::write(&sentinel, b"untouched").unwrap();
    let direct = run_target(&f, false, &fault.path, &marker, signal, || {});
    fs::remove_file(&marker).unwrap();
    let wrapped = run_target(&f, true, &fault.path, &marker, signal, || {});
    assert_eq!(direct.1, wrapped.1);
    if signal {
        assert_eq!(direct.0.signal(), Some(libc::SIGTERM));
        assert_eq!(wrapped.0.signal(), direct.0.signal());
    } else {
        assert_eq!(direct.0.code(), Some(17));
        assert_eq!(wrapped.0.code(), direct.0.code());
        assert_eq!(wrapped.1, b"target-ready\ntarget-input:input-bytes\n");
    }
    let observed = fault.finish();
    if mode == "committed-register" {
        assert!(
            observed
                .counts
                .get("REGISTER-committed")
                .copied()
                .unwrap_or(0)
                > 0,
            "no real commit/withheld-ack barrier"
        );
        let db = rusqlite::Connection::open(&f.database).unwrap();
        assert_eq!(
            db.query_row("SELECT count(*) FROM job_lifecycle", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
    let mut recovered = if let Some(mut d) = daemon.take() {
        d.stop(libc::SIGTERM);
        f.report();
        restart(&f)
    } else {
        f.spawn("test")
    };
    assert!(f.cli("status").status.success());
    no_reservation_or_cleanup(&f, &sentinel);
    assert_eq!(
        fs::read_to_string(&marker).unwrap().lines().count(),
        1,
        "replayed after recovery"
    );
    recovered.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn broken_stderr_unavailable_preserves_child_exit() {
    let _serial = support::serial();
    outage("absent", false);
    receipt(
        "broken-stderr-unavailable",
        serde_json::json!({"passed":true,"observedExit":17,"invocations":1,"stderr":"writer open, reader closed; diagnostic cannot be received"}),
    );
}

#[test]
fn broken_stderr_uncertain_preserves_child_exit() {
    let _serial = support::serial();
    outage("committed-register", false);
    receipt(
        "broken-stderr-uncertain",
        serde_json::json!({"passed":true,"observedExit":17,"invocations":1,"commit":"REGISTER ack withheld","stderr":"writer open, reader closed; diagnostic cannot be received"}),
    );
}

#[test]
fn broken_stderr_preserves_directed_signal_after_handshake() {
    let _serial = support::serial();
    for mode in ["absent", "committed-register"] {
        outage(mode, true);
    }
}

#[test]
fn broken_stderr_managed_completion_failure_preserves_child_exit() {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let marker = f.root.join("target-calls");
    let sentinel = f.root.join("cleanup-sentinel");
    fs::write(&sentinel, b"untouched").unwrap();
    let direct = run_target(&f, false, &f.socket, &marker, false, || {});
    fs::remove_file(&marker).unwrap();
    let wrapped = run_target(&f, true, &f.socket, &marker, false, || {
        let end = Instant::now() + Duration::from_secs(5);
        loop {
            let db = rusqlite::Connection::open(&f.database).unwrap();
            let attached: i64 = db
                .query_row(
                    "SELECT count(*) FROM job_lifecycle WHERE child_pid IS NOT NULL",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            if attached == 1 {
                break;
            }
            assert!(Instant::now() < end, "managed attachment was not committed");
            thread::sleep(Duration::from_millis(2));
        }
        daemon.stop(libc::SIGTERM);
        f.report();
    });
    assert_eq!(direct.0.code(), Some(17));
    assert_eq!(wrapped.0.code(), direct.0.code());
    assert_eq!(wrapped.1, direct.1);
    let db = rusqlite::Connection::open(&f.database).unwrap();
    let incomplete: i64 = db.query_row("SELECT count(*) FROM job_lifecycle WHERE child_pid IS NOT NULL AND outcome_kind IS NULL", [], |r| r.get(0)).unwrap();
    assert_eq!(incomplete, 1);
    drop(db);
    let mut recovered = restart(&f);
    assert!(f.cli("status").status.success());
    no_reservation_or_cleanup(&f, &sentinel);
    assert_eq!(fs::read_to_string(marker).unwrap().lines().count(), 1);
    recovered.stop(libc::SIGTERM);
    f.report();
}

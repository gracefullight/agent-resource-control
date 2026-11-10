//! A full, undrained stderr must not stop child supervision.
use super::{
    cli::{command, restart},
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
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const TARGET: &str = r#"import fcntl,json,os,sys,time
with open(sys.argv[1], 'a') as marker:
    marker.write(str(os.getpid())+'\n')
print('ready', flush=True)
data=sys.stdin.buffer.read()
flags=[]
for _ in range(100):
    flags.append(fcntl.fcntl(2,fcntl.F_GETFL))
    time.sleep(.001)
report={'argv':sys.argv[2:], 'cwd':os.getcwd(), 'env':os.environ['PRIVATE_ENV'],
        'stdin':list(data), 'flags':flags, 'fdflags':[fcntl.fcntl(i,fcntl.F_GETFD) for i in range(3)],
        'stderr_zero_write':os.write(2,b'')}
print(json.dumps(report,sort_keys=True), flush=True)
sys.exit(17)
"#;

fn full_pipe() -> (File, File, File, usize, i32) {
    let mut fds = [-1; 2];
    assert_eq!(unsafe { libc::pipe(fds.as_mut_ptr()) }, 0);
    let flags = unsafe { libc::fcntl(fds[1], libc::F_GETFL) };
    assert!(flags >= 0 && flags & libc::O_NONBLOCK == 0);
    assert_eq!(
        unsafe { libc::fcntl(fds[1], libc::F_SETFL, flags | libc::O_NONBLOCK) },
        0
    );
    let mut count = 0;
    loop {
        let bytes = [b'x'; 4096];
        let n = unsafe { libc::write(fds[1], bytes.as_ptr().cast(), bytes.len()) };
        if n < 0 {
            assert_eq!(
                std::io::Error::last_os_error().kind(),
                std::io::ErrorKind::WouldBlock
            );
            break;
        }
        count += n as usize;
    }
    assert!(count > 0);
    assert_eq!(unsafe { libc::fcntl(fds[1], libc::F_SETFL, flags) }, 0);
    let flags = unsafe { libc::fcntl(fds[1], libc::F_GETFL) };
    assert!(flags >= 0 && flags & libc::O_NONBLOCK == 0);
    let duplicate = unsafe { libc::dup(fds[1]) };
    assert!(duplicate >= 0);
    unsafe {
        (
            File::from_raw_fd(fds[0]),
            File::from_raw_fd(fds[1]),
            File::from_raw_fd(duplicate),
            count,
            flags,
        )
    }
}

fn target(
    f: &Fixture,
    wrapped: bool,
    socket: &std::path::Path,
    marker: &std::path::Path,
    after_ready: impl FnOnce(),
) -> (serde_json::Value, u32) {
    let (mut reader, writer, shared_writer, filled, flags) = full_pipe();
    let mut c = if wrapped {
        let mut c = command(f);
        c.args(["exec", "--", "python3"]);
        c
    } else {
        Command::new("python3")
    };
    c.args(["-c", TARGET])
        .arg(marker)
        .arg("argv-secret-sentinel")
        .current_dir(&f.root)
        .env(arc_core::ARC_SOCKET_PATH, socket)
        .env("PRIVATE_ENV", "environment-secret-sentinel")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::from(writer));
    let started = Instant::now();
    let mut child = OwnedChild::new(c.spawn().unwrap());
    drop(c);
    let pid = child.child.id();
    let mut stdout = BufReader::new(child.child.stdout.take().unwrap());
    let mut poll = libc::pollfd {
        fd: stdout.get_ref().as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    assert!(
        unsafe { libc::poll(&mut poll, 1, 10_000) } > 0,
        "child did not start"
    );
    let mut line = String::new();
    stdout.read_line(&mut line).unwrap();
    assert_eq!(line, "ready\n");
    let to_ready = started.elapsed();
    after_ready();
    let released = Instant::now();
    child
        .child
        .stdin
        .take()
        .unwrap()
        .write_all(b"stdin\0bytes\n")
        .unwrap();
    let until = Instant::now() + Duration::from_secs(3);
    let status = loop {
        assert_eq!(
            unsafe { libc::fcntl(shared_writer.as_raw_fd(), libc::F_GETFL) },
            flags,
            "wrapper changed the shared descriptor flags"
        );
        if let Some(status) = child.child.try_wait().unwrap() {
            break status;
        }
        assert!(
            Instant::now() < until,
            "wrapper blocked before the stderr reader was drained"
        );
        thread::sleep(Duration::from_millis(1));
    };
    assert_eq!(status.code(), Some(17));
    eprintln!(
        "full-stderr wrapped={wrapped} pid={pid} spawn-to-ready-ms={:.3} release-to-exit-ms={:.3}",
        to_ready.as_secs_f64() * 1000.0,
        released.elapsed().as_secs_f64() * 1000.0
    );
    let mut output = String::new();
    stdout.read_to_string(&mut output).unwrap();
    let report: serde_json::Value = serde_json::from_str(&output).unwrap();
    assert_eq!(report["flags"], serde_json::json!(vec![flags; 100]));
    assert_eq!(report["stderr_zero_write"], 0);
    assert_eq!(fs::read_to_string(marker).unwrap().lines().count(), 1);
    drop(shared_writer);
    // The reader stays open and is not drained until the wrapper has returned.
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, vec![b'x'; filled]);
    (report, pid)
}

fn compare(mode: &str) {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let fault = Fault::new(&f, mode);
    let sentinel = f.root.join("cleanup-sentinel");
    fs::write(&sentinel, b"untouched").unwrap();
    let direct_marker = f.root.join("direct-calls");
    let wrapped_marker = f.root.join("wrapped-calls");
    let (direct, direct_pid) = target(&f, false, &fault.path, &direct_marker, || {});
    let (wrapped, wrapper_pid) = target(&f, true, &fault.path, &wrapped_marker, || {});
    assert_eq!(direct, wrapped);
    let observed = fault.finish();
    assert!(observed
        .counts
        .keys()
        .all(|key| key == "REGISTER" || key == "REGISTER-committed"));
    if mode == "committed-register" {
        assert!(
            observed
                .counts
                .get("REGISTER-committed")
                .copied()
                .unwrap_or(0)
                > 0
        );
        let db = rusqlite::Connection::open(&f.database).unwrap();
        assert_eq!(
            db.query_row("SELECT count(*) FROM job_lifecycle", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }
    daemon.stop(libc::SIGTERM);
    f.report();
    let mut recovered = restart(&f);
    assert!(f.cli("status").status.success());
    assert_eq!(
        fs::read_to_string(&wrapped_marker).unwrap().lines().count(),
        1
    );
    let db = rusqlite::Connection::open(&f.database).unwrap();
    for table in ["permits", "gc_operations"] {
        let sql = format!("SELECT count(*) FROM {table}");
        assert_eq!(db.query_row(&sql, [], |r| r.get::<_, i64>(0)).unwrap(), 0);
    }
    assert_eq!(fs::read(sentinel).unwrap(), b"untouched");
    drop(db);
    recovered.stop(libc::SIGTERM);
    f.report();
    for entry in fs::read_dir(&f.root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_file() {
            let bytes = fs::read(path).unwrap();
            for secret in ["argv-secret-sentinel", "environment-secret-sentinel"] {
                assert!(
                    !bytes.windows(secret.len()).any(|w| w == secret.as_bytes()),
                    "secret persisted"
                );
            }
        }
    }
    eprintln!("full-stderr {mode}: direct pid {direct_pid}, wrapper pid {wrapper_pid}, one invocation, exit 17");
}

#[test]
fn full_stderr_unavailable_does_not_block_supervision() {
    compare("absent");
}

#[test]
fn full_stderr_uncertain_does_not_block_supervision() {
    compare("committed-register");
}

#[test]
fn full_stderr_managed_completion_failure_does_not_block_exit() {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let direct_marker = f.root.join("direct-calls");
    let wrapped_marker = f.root.join("wrapped-calls");
    let (direct, _) = target(&f, false, &f.socket, &direct_marker, || {});
    let (wrapped, _) = target(&f, true, &f.socket, &wrapped_marker, || {
        let until = Instant::now() + Duration::from_secs(5);
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
            assert!(
                Instant::now() < until,
                "managed attachment was not committed"
            );
            thread::sleep(Duration::from_millis(2));
        }
        daemon.stop(libc::SIGTERM);
        f.report();
    });
    assert_eq!(direct, wrapped);
    let db = rusqlite::Connection::open(&f.database).unwrap();
    let incomplete: i64 = db.query_row("SELECT count(*) FROM job_lifecycle WHERE child_pid IS NOT NULL AND outcome_kind IS NULL", [], |r| r.get(0)).unwrap();
    assert_eq!(incomplete, 1);
    drop(db);
    let mut recovered = restart(&f);
    assert!(f.cli("status").status.success());
    assert_eq!(
        fs::read_to_string(wrapped_marker).unwrap().lines().count(),
        1
    );
    recovered.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn full_stderr_directed_signal_preserves_native_status() {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut statuses = Vec::new();
    for wrapped in [false, true] {
        let (mut reader, writer, shared_writer, filled, flags) = full_pipe();
        let marker = f.root.join(format!("signal-calls-{wrapped}"));
        let mut c = if wrapped {
            let mut c = command(&f);
            c.args(["exec", "--", "/bin/sh"]);
            c
        } else {
            Command::new("/bin/sh")
        };
        c.args(["-c", "printf 'call\\n' >> \"$ARC_TEST_MARKER\"; printf 'ready\\n'; IFS= read -r value; exit 17"])
            .env("ARC_TEST_MARKER", &marker)
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::from(writer));
        let mut child = OwnedChild::new(c.spawn().unwrap());
        drop(c);
        let mut stdout = BufReader::new(child.child.stdout.take().unwrap());
        let mut poll = libc::pollfd {
            fd: stdout.get_ref().as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        assert!(unsafe { libc::poll(&mut poll, 1, 10_000) } > 0);
        let mut line = String::new();
        stdout.read_line(&mut line).unwrap();
        assert_eq!(line, "ready\n");
        let signaled = Instant::now();
        assert_eq!(
            unsafe { libc::kill(child.child.id() as i32, libc::SIGTERM) },
            0
        );
        let until = Instant::now() + Duration::from_secs(3);
        let status = loop {
            assert_eq!(
                unsafe { libc::fcntl(shared_writer.as_raw_fd(), libc::F_GETFL) },
                flags
            );
            if let Some(status) = child.child.try_wait().unwrap() {
                break status;
            }
            assert!(
                Instant::now() < until,
                "signal supervision blocked on stderr"
            );
            thread::sleep(Duration::from_millis(1));
        };
        eprintln!(
            "full-stderr signal wrapped={wrapped} pid={} signal-to-exit-ms={:.3}",
            child.child.id(),
            signaled.elapsed().as_secs_f64() * 1000.0
        );
        assert_eq!(status.signal(), Some(libc::SIGTERM));
        assert_eq!(fs::read_to_string(marker).unwrap(), "call\n");
        drop(shared_writer);
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, vec![b'x'; filled]);
        statuses.push(status);
    }
    assert_eq!(statuses[0], statuses[1]);
    let mut recovered = f.spawn("test");
    assert!(f.cli("status").status.success());
    assert_eq!(
        fs::read_to_string(f.root.join("signal-calls-true")).unwrap(),
        "call\n"
    );
    recovered.stop(libc::SIGTERM);
    f.report();
}

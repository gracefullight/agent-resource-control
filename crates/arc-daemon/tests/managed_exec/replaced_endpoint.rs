//! Real commit barriers followed by replacement of the proxy endpoint.
use super::{
    cli::{child_command, restart, wait},
    fail_open::{arc_stderr, receipt},
    runtime::Fixture,
    support,
};
use arc_ipc::Request;
use std::{
    ffi::OsString,
    fs,
    io::Write,
    os::unix::{ffi::OsStringExt, net::UnixListener, net::UnixStream},
    process::{Child, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Copy)]
enum Fault {
    RegisterAck,
    ClaimAck,
    AfterRegister,
}

fn replace(path: &std::path::Path, parent: bool) {
    fs::remove_file(path).unwrap();
    if parent {
        let dir = path.parent().unwrap();
        fs::remove_dir(dir).unwrap();
        fs::write(dir, b"not a directory").unwrap();
    } else {
        fs::write(path, b"not a socket").unwrap();
    }
}

fn proxy(
    f: &Fixture,
    fault: Fault,
    parent: bool,
) -> (std::path::PathBuf, thread::JoinHandle<Vec<String>>) {
    let dir = f.root.join("replace");
    fs::create_dir(&dir).unwrap();
    let path = dir.join("proxy.sock");
    let listener = UnixListener::bind(&path).unwrap();
    let socket = path.clone();
    let backend = f.socket.clone();
    let join = thread::spawn(move || {
        let mut actions = Vec::new();
        loop {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let request: Request = arc_ipc::read_message(&mut stream).unwrap();
            let action = request.payload["action"].as_str().unwrap().to_string();
            actions.push(action.clone());
            let mut daemon = UnixStream::connect(&backend).unwrap();
            daemon
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let response = arc_ipc::exchange(&mut daemon, &request).unwrap();
            assert_eq!(response.status, "ok", "real daemon commit barrier");
            let trigger = match fault {
                Fault::RegisterAck | Fault::AfterRegister => action == "REGISTER",
                Fault::ClaimAck => action == "CLAIM",
            };
            if trigger {
                // Make the next connection fail before releasing or dropping this acknowledgement.
                replace(&socket, parent);
                assert_eq!(
                    UnixStream::connect(&socket).unwrap_err().raw_os_error(),
                    Some(if parent {
                        libc::ENOTDIR
                    } else {
                        libc::ENOTSOCK
                    })
                );
                if matches!(fault, Fault::AfterRegister) {
                    arc_ipc::write_response(&mut stream, &response).unwrap();
                }
                break;
            }
            arc_ipc::write_response(&mut stream, &response).unwrap();
        }
        actions
    });
    (path, join)
}

fn handshake(listener: &UnixListener, child: &mut Child, start: Instant) -> (UnixStream, Duration) {
    listener.set_nonblocking(true).unwrap();
    loop {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let elapsed = start.elapsed();
                stream.set_nonblocking(false).unwrap();
                let _ = arc_ipc::read_frame(&mut stream).unwrap();
                return (stream, elapsed);
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => (),
            Err(e) => panic!("control socket: {e}"),
        }
        assert!(
            child.try_wait().unwrap().is_none(),
            "wrapper exited before child invocation"
        );
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "no child invocation"
        );
        thread::sleep(Duration::from_millis(1));
    }
}

fn observe(
    f: &Fixture,
    endpoint: &std::path::Path,
    marker: &std::path::Path,
    wrapped: bool,
) -> (Output, Duration) {
    let control = f.root.join(format!("control-{wrapped}.sock"));
    let listener = UnixListener::bind(control).unwrap();
    let args = [
        OsString::from(""),
        "two words".into(),
        "한글".into(),
        OsString::from_vec(vec![0xff, 0xfe]),
        "argv-secret-sentinel".into(),
    ];
    let mut command = child_command(f, wrapped);
    command
        .args(args)
        .env(arc_core::ARC_SOCKET_PATH, endpoint)
        .env(
            "ARC_FIXTURE_CONTROL",
            f.root.join(format!("control-{wrapped}.sock")),
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if wrapped {
        command.env("ARC_FIXTURE_MARKER", marker);
    }
    let start = Instant::now();
    let mut child = command.spawn().unwrap();
    let (mut barrier, elapsed) = handshake(&listener, &mut child, start);
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"stdin\0bytes\n")
        .unwrap();
    barrier.write_all(b"x").unwrap();
    wait(&mut child);
    (child.wait_with_output().unwrap(), elapsed)
}

fn scenario(name: &str, fault: Fault, parent: bool) {
    let f = Fixture::new();
    let sentinel = f.root.join("cleanup-sentinel");
    fs::write(&sentinel, b"untouched").unwrap();
    let mut daemon = f.spawn("test");
    let (endpoint, proxy) = proxy(&f, fault, parent);
    let marker = f.root.join("calls");
    let (direct, _) = observe(&f, &endpoint, &marker, false);
    let (wrapped, elapsed) = observe(&f, &endpoint, &marker, true);
    let actions = proxy.join().unwrap();
    assert!(
        elapsed <= Duration::from_millis(2000),
        "{name}: {elapsed:?}"
    );
    assert_eq!(direct.status.code(), Some(17));
    assert_eq!(wrapped.status.code(), Some(17));
    assert_eq!(direct.stdout, wrapped.stdout);
    assert_eq!(direct.stderr, arc_stderr(&wrapped.stderr));
    let diagnostic = String::from_utf8_lossy(&wrapped.stderr);
    assert!(diagnostic.contains("TRACKING_UNCERTAIN"), "{diagnostic}");
    assert!(!diagnostic.contains("secret"));
    assert_eq!(fs::read_to_string(&marker).unwrap().lines().count(), 1);
    assert_eq!(
        actions,
        if matches!(fault, Fault::ClaimAck) {
            vec!["REGISTER", "CLAIM"]
        } else {
            vec!["REGISTER"]
        }
    );
    let db = rusqlite::Connection::open(&f.database).unwrap();
    let row: (String, bool, bool) = db.query_row("SELECT j.state, l.launch_claimed_at_ms IS NOT NULL, l.child_pid IS NOT NULL FROM jobs j JOIN job_lifecycle l ON l.job_id=j.id", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap();
    assert_eq!(
        row.0,
        if matches!(fault, Fault::ClaimAck) {
            "LAUNCHING"
        } else {
            "REGISTERED"
        }
    );
    assert_eq!(row.1, matches!(fault, Fault::ClaimAck));
    assert!(!row.2);
    assert_eq!(
        db.query_row("SELECT count(*) FROM permits", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
    drop(db);
    daemon.stop(libc::SIGTERM);
    f.report();
    let mut recovered = restart(&f);
    assert!(f.cli("status").status.success());
    assert_eq!(fs::read_to_string(&marker).unwrap().lines().count(), 1);
    let db = rusqlite::Connection::open(&f.database).unwrap();
    for table in ["permits", "gc_operations"] {
        let sql = format!("SELECT count(*) FROM {table}");
        assert_eq!(db.query_row(&sql, [], |r| r.get::<_, i64>(0)).unwrap(), 0);
    }
    assert_eq!(fs::read(sentinel).unwrap(), b"untouched");
    drop(db);
    recovered.stop(libc::SIGTERM);
    f.report();
    receipt(
        name,
        serde_json::json!({"scenario":name,"parentSpawnToInvocationMs":elapsed.as_secs_f64()*1000.0,"boundary":"parent before wrapper spawn to child control connection; no subtraction","invocations":1,"observedExit":17,"recoveryInvocations":1,"realCommitPhase":row.0,"claimRecorded":row.1,"childRecorded":row.2,"actions":actions,"passed":true}),
    );
}

#[test]
fn committed_register_ack_loss_endpoint_replacement_runs_once() {
    let _serial = support::serial();
    for parent in [false, true] {
        scenario(
            if parent {
                "register-parent"
            } else {
                "register-file"
            },
            Fault::RegisterAck,
            parent,
        );
    }
}

#[test]
fn committed_claim_ack_loss_endpoint_replacement_runs_once() {
    let _serial = support::serial();
    for parent in [false, true] {
        scenario(
            if parent { "claim-parent" } else { "claim-file" },
            Fault::ClaimAck,
            parent,
        );
    }
}

#[test]
fn confirmed_register_endpoint_replacement_before_claim_runs_once() {
    let _serial = support::serial();
    for parent in [false, true] {
        scenario(
            if parent {
                "registered-parent"
            } else {
                "registered-file"
            },
            Fault::AfterRegister,
            parent,
        );
    }
}

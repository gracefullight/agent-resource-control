//! Native socket-access regression, with real daemon commit barriers.
use super::{
    cli::{child_command, command, restart, wait},
    fail_open::{arc_stderr, receipt},
    runtime::{Fixture, OwnedChild},
    support,
};
use arc_ipc::Request;
use std::{
    ffi::OsString,
    fs,
    io::{Read, Write},
    os::unix::{
        ffi::OsStringExt,
        fs::PermissionsExt,
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Case {
    BeforeRegister,
    RegisterAckLoss,
    BeforeClaim,
    ClaimAckLoss,
}
impl Case {
    fn label(self) -> &'static str {
        match self {
            Self::BeforeRegister => "connect-access-before-register",
            Self::RegisterAckLoss => "connect-access-register-ack-loss",
            Self::BeforeClaim => "connect-access-before-claim",
            Self::ClaimAckLoss => "connect-access-claim-ack-loss",
        }
    }
}

// The test owns this directory. Restoring it also runs during unwinding.
struct SocketDirectory(PathBuf);
impl SocketDirectory {
    fn new(root: &Path) -> Self {
        let path = root.join("denied");
        fs::create_dir(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        Self(path)
    }
    fn deny(path: &Path, socket: &Path) -> i32 {
        fs::set_permissions(path, fs::Permissions::from_mode(0o000)).unwrap();
        let error = match UnixStream::connect(socket) {
            Err(error) => error,
            Ok(_) => panic!("socket permission fixture did not deny access"),
        };
        eprintln!("native socket permission probe: {:?}", error.raw_os_error());
        assert_eq!(
            error.raw_os_error(),
            Some(libc::EACCES),
            "native permission prerequisite: {error}"
        );
        error.raw_os_error().unwrap()
    }
    fn restore(&self) {
        fs::set_permissions(&self.0, fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(
            fs::metadata(&self.0).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
}
impl Drop for SocketDirectory {
    fn drop(&mut self) {
        if let Err(error) = fs::set_permissions(&self.0, fs::Permissions::from_mode(0o700)) {
            eprintln!("failed to restore test socket directory permissions: {error}");
        }
    }
}

// Read the independently committed SQLite row before denying the next
// connection or dropping the real daemon's successful acknowledgement.
fn committed(database: &Path, request: &Request, action: &str) -> serde_json::Value {
    let id = request.payload["job_id"].as_str().unwrap();
    let db = rusqlite::Connection::open(database).unwrap();
    let (revision, claimed, child): (i64, Option<i64>, Option<i64>) = db
        .query_row(
            "SELECT revision, launch_claimed_at_ms, child_pid FROM job_lifecycle WHERE job_id=?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(revision, if action == "REGISTER" { 1 } else { 2 });
    assert_eq!(claimed.is_some(), action == "CLAIM");
    assert_eq!(child, None);
    serde_json::json!({"action":action,"revision":revision,"claimed":claimed.is_some(),"childAttached":false})
}

struct ProxyWorker {
    stop: Arc<AtomicBool>,
    join: Option<thread::JoinHandle<serde_json::Value>>,
}
impl ProxyWorker {
    fn finish(mut self) -> serde_json::Value {
        self.join.take().unwrap().join().unwrap()
    }
}
impl Drop for ProxyWorker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(worker) = self.join.take() {
            let _ = worker.join();
        }
    }
}

fn proxy(
    listener: UnixListener,
    backend: PathBuf,
    database: PathBuf,
    dir: PathBuf,
    socket: PathBuf,
    case: Case,
) -> ProxyWorker {
    let stop = Arc::new(AtomicBool::new(false));
    let stopped = stop.clone();
    let join = thread::spawn(move || {
        listener.set_nonblocking(true).unwrap();
        let mut actions = Vec::new();
        let mut commits = Vec::new();
        let end = Instant::now() + Duration::from_secs(8);
        loop {
            let (mut peer, _) = loop {
                if stopped.load(Ordering::SeqCst) {
                    return serde_json::json!({"stopped":true});
                }
                match listener.accept() {
                    Ok(peer) => break peer,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < end, "proxy request barrier timed out");
                        thread::sleep(Duration::from_millis(1));
                    }
                    Err(e) => panic!("proxy accept: {e}"),
                }
            };
            peer.set_nonblocking(false).unwrap();
            peer.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
            let request: Request = arc_ipc::read_message(&mut peer).unwrap();
            let action = request.payload["action"].as_str().unwrap().to_string();
            actions.push(action.clone());
            let mut target = UnixStream::connect(&backend).unwrap();
            target
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let response = arc_ipc::exchange(&mut target, &request).unwrap();
            assert_eq!(
                response.status,
                arc_ipc::STATUS_OK,
                "real daemon commit response"
            );
            commits.push(committed(&database, &request, &action));
            let deny_now = match case {
                Case::RegisterAckLoss | Case::BeforeClaim => action == "REGISTER",
                Case::ClaimAckLoss => action == "CLAIM",
                Case::BeforeRegister => unreachable!(),
            };
            if deny_now {
                let errno = SocketDirectory::deny(&dir, &socket);
                if case == Case::BeforeClaim {
                    // Only CLAIM is denied: the wrapper first receives REGISTER's ack.
                    arc_ipc::write_response(&mut peer, &response).unwrap();
                }
                // Otherwise drop the real committed acknowledgement.
                return serde_json::json!({"actions":actions,"commits":commits,
                    "nativeErrno":errno,"ackWithheld":case != Case::BeforeClaim});
            }
            arc_ipc::write_response(&mut peer, &response).unwrap();
        }
    });
    ProxyWorker {
        stop,
        join: Some(join),
    }
}

fn observe(case: Case) {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    let guard = SocketDirectory::new(&f.root);
    let socket = guard.0.join("peer.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    let marker = f.root.join("calls");
    let sentinel = f.root.join("untouched-resource");
    fs::write(&sentinel, b"preserve this file").unwrap();
    let args = [
        OsString::from(""),
        "two words".into(),
        OsString::from_vec(vec![0xff, 0xfe]),
        "argv-secret-sentinel".into(),
    ];
    let direct = child_command(&f, false)
        .args(&args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut direct = OwnedChild::new(direct);
    direct
        .child
        .stdin
        .take()
        .unwrap()
        .write_all(b"stdin\0bytes\n")
        .unwrap();
    assert_eq!(wait(&mut direct.child).code(), Some(17));
    let mut expected_stdout = Vec::new();
    let mut expected_stderr = Vec::new();
    direct
        .child
        .stdout
        .take()
        .unwrap()
        .read_to_end(&mut expected_stdout)
        .unwrap();
    direct
        .child
        .stderr
        .take()
        .unwrap()
        .read_to_end(&mut expected_stderr)
        .unwrap();

    let mut worker = None;
    let initial_errno = if case == Case::BeforeRegister {
        Some(SocketDirectory::deny(&guard.0, &socket))
    } else {
        worker = Some(proxy(
            listener,
            f.socket.clone(),
            f.database.clone(),
            guard.0.clone(),
            socket.clone(),
            case,
        ));
        None
    };
    let control = f.root.join("control.sock");
    let control_listener = UnixListener::bind(&control).unwrap();
    control_listener.set_nonblocking(true).unwrap();
    let mut c = child_command(&f, true);
    c.args(&args)
        .env(arc_core::ARC_SOCKET_PATH, &socket)
        .env("ARC_FIXTURE_CONTROL", &control)
        .env("ARC_FIXTURE_MARKER", &marker)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // This unadjusted boundary starts before spawning arcctl and ends at the
    // native child's handshake, including scheduling and observation overhead.
    let start = Instant::now();
    let mut wrapper = OwnedChild::new(c.spawn().unwrap());
    let wrapper_pid = wrapper.child.id();
    let end = start + Duration::from_secs(8);
    let (mut barrier, _) = loop {
        match control_listener.accept() {
            Ok(stream) => break stream,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if let Some(status) = wrapper.child.try_wait().unwrap() {
                    let mut stderr = String::new();
                    wrapper
                        .child
                        .stderr
                        .take()
                        .unwrap()
                        .read_to_string(&mut stderr)
                        .unwrap();
                    panic!("wrapper exited before child handshake: {status}; {stderr}");
                }
                assert!(Instant::now() < end, "child handshake timed out");
                thread::sleep(Duration::from_millis(1));
            }
            Err(e) => panic!("child control accept: {e}"),
        }
    };
    barrier.set_nonblocking(false).unwrap();
    barrier
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let info: serde_json::Value =
        serde_json::from_slice(&arc_ipc::read_frame(&mut barrier).unwrap()).unwrap();
    let elapsed = start.elapsed();
    assert_eq!(info["ppid"], wrapper_pid, "the CLI owns the native child");
    assert!(
        elapsed <= Duration::from_millis(2000),
        "{case:?} unadjusted: {elapsed:?}"
    );
    wrapper
        .child
        .stdin
        .take()
        .unwrap()
        .write_all(b"stdin\0bytes\n")
        .unwrap();
    barrier.write_all(b"x").unwrap();
    assert_eq!(wait(&mut wrapper.child).code(), Some(17));
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
    assert_eq!(stdout, expected_stdout);
    assert_eq!(arc_stderr(&stderr), expected_stderr);
    let diagnostic = String::from_utf8_lossy(&stderr);
    assert!(
        diagnostic.contains(if case == Case::BeforeRegister {
            "TRACKING_UNAVAILABLE\nCommand started. ARC tracking is unavailable."
        } else {
            "TRACKING_UNCERTAIN\nCommand started. Lifecycle recording is unconfirmed."
        }),
        "{diagnostic}"
    );
    for secret in ["argv-secret-sentinel", "environment-secret-sentinel"] {
        assert!(!diagnostic.contains(secret));
    }
    assert_eq!(fs::read_to_string(&marker).unwrap().lines().count(), 1);
    let proxy_report = worker.map(ProxyWorker::finish).unwrap_or_else(
        || serde_json::json!({"actions":[],"commits":[],"nativeErrno":initial_errno.unwrap()}),
    );
    let actions = match case {
        Case::BeforeRegister => vec![],
        Case::RegisterAckLoss | Case::BeforeClaim => vec!["REGISTER"],
        Case::ClaimAckLoss => vec!["REGISTER", "CLAIM"],
    };
    assert_eq!(proxy_report["actions"], serde_json::json!(actions));
    assert_eq!(proxy_report["nativeErrno"], libc::EACCES);
    if case != Case::BeforeRegister {
        assert_eq!(
            proxy_report["commits"].as_array().unwrap().len(),
            actions.len()
        );
        assert_eq!(proxy_report["ackWithheld"], case != Case::BeforeClaim);
    }
    guard.restore();
    let db = rusqlite::Connection::open(&f.database).unwrap();
    for table in ["permits", "gc_operations"] {
        let count: i64 = db
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0, "{table}");
    }
    let count: i64 = db
        .query_row("SELECT count(*) FROM job_lifecycle", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, i64::from(case != Case::BeforeRegister));
    drop(db);
    assert_eq!(fs::read(&sentinel).unwrap(), b"preserve this file");
    daemon.stop(libc::SIGTERM);
    f.report();
    let mut recovered = restart(&f);
    assert!(String::from_utf8(f.cli("status").stdout)
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
    assert_eq!(fs::read_to_string(&marker).unwrap().lines().count(), 1);
    assert_eq!(fs::read(&sentinel).unwrap(), b"preserve this file");
    let db = rusqlite::Connection::open(&f.database).unwrap();
    for table in ["permits", "gc_operations"] {
        let count: i64 = db
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0, "{table} after recovery");
    }
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
                    "persisted secret"
                );
            }
        }
    }
    receipt(
        case.label(),
        serde_json::json!({"scenario":case.label(),"passed":true,
        "parentSpawnToInvocationMs":elapsed.as_secs_f64()*1000.0,
        "boundary":"parent Instant before wrapper spawn to child control handshake; includes scheduling and observation overhead",
        "metadataElapsedMs":null,"handoffMs":null,"startupMs":null,
        "subdivisionReason":"Internal metadata, handoff and startup intervals were not measured.",
        "samples":{"startNs":0,"endNs":elapsed.as_nanos() as u64,"clock":"std::time::Instant"},
        "wrapperPid":wrapper_pid,"childPid":info["pid"],"childParentPid":info["ppid"],"invocations":1,"observedExit":17,
        "recoveryInvocations":1,"permits":0,"arcCleanup":false,"proxy":proxy_report}),
    );
}

#[test]
fn connect_access_denied_before_register_runs_once() {
    observe(Case::BeforeRegister);
}
#[test]
fn connect_access_denied_after_register_ack_loss_stays_uncertain() {
    observe(Case::RegisterAckLoss);
}
#[test]
fn connect_access_denied_before_claim_stays_uncertain() {
    observe(Case::BeforeClaim);
}
#[test]
fn connect_access_denied_after_claim_ack_loss_stays_uncertain() {
    observe(Case::ClaimAckLoss);
}

//! Current-user ownership through the built CLI and authenticated daemon.
//!
//! Every process here runs as this host's current user. A forged identity is a
//! live same-user process named by another one. Nothing in this file is a
//! second-account test; that case stays the external part of SAFE-007.
use super::{cli, protocol, runtime, support};
use arc_core::process_identity::ProcessIdentity;
use arc_ipc::{
    jobs::{
        CompleteRequest, JobId, JobSnapshot, Outcome, Phase, StartRequest, ERROR_JOB_CONFLICT,
        OPERATION_JOB_COMPLETE, OPERATION_JOB_START,
    },
    Request, Response,
};
use serde_json::{json, Value};
use std::{
    fs,
    io::Write,
    os::unix::{
        fs::{FileTypeExt, MetadataExt},
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant},
};

const WATCHDOG: Duration = Duration::from_secs(60);

/// The built CLI under a cleared environment and temporary HOME. Its fixture
/// child reports its own identity and then waits at `listener`.
struct Held {
    wrapper: runtime::child_watchdog::Owned,
    listener: UnixListener,
    marker: PathBuf,
}

fn held(f: &runtime::Fixture, socket: &Path, tag: &str) -> Held {
    let home = f.root.join(format!("{tag}-home"));
    fs::create_dir(&home).unwrap();
    let control = f.root.join(format!("{tag}.sock"));
    let listener = UnixListener::bind(&control).unwrap();
    let marker = f.root.join(format!("{tag}-started"));
    let mut command = Command::new(cli::binary("arcctl"));
    command
        .env_clear()
        .env("HOME", &home)
        .env("XDG_CONFIG_HOME", home.join("xdg"))
        .env("PATH", "/usr/bin:/bin")
        .env(arc_core::ARC_SOCKET_PATH, socket)
        .env(arc_core::ARC_DB_PATH, &f.database)
        .env("ARC_FIXTURE_MODE", "echo")
        .env("ARC_FIXTURE_CONTROL", &control)
        .env("ARC_FIXTURE_MARKER", &marker)
        .args(["exec", "--"])
        .arg(cli::binary("arc-exec-fixture"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    runtime::instrumentation::forward(&mut command);
    Held {
        wrapper: runtime::child_watchdog::Owned::spawn(&mut command).unwrap(),
        listener,
        marker,
    }
}

fn running(f: &runtime::Fixture, job_id: &JobId) -> JobSnapshot {
    let deadline = Instant::now() + WATCHDOG;
    loop {
        let job = protocol::status(f, job_id);
        if job.phase == Phase::Running {
            return job;
        }
        assert!(Instant::now() < deadline, "managed job never ran: {job:?}");
        thread::yield_now();
    }
}

fn rows(f: &runtime::Fixture, n: u32) -> i64 {
    let db = rusqlite::Connection::open(&f.database).unwrap();
    let sql = "SELECT count(*) FROM jobs WHERE id=?1";
    db.query_row(sql, [protocol::id(n).as_str()], |row| row.get(0))
        .unwrap()
}

/// Times the fixture child started; it appends one line per start.
fn starts(marker: &Path) -> usize {
    fs::read_to_string(marker).unwrap().lines().count()
}

fn invalid(response: Response) {
    assert_eq!(response.error.unwrap().code, arc_ipc::ERROR_INVALID_REQUEST);
}

/// Real and effective user the kernel reports for a live process.
fn users(pid: u32) -> (libc::uid_t, libc::uid_t) {
    #[cfg(target_os = "macos")]
    {
        let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
        let size = std::mem::size_of_val(&info) as libc::c_int;
        let buffer = (&mut info as *mut libc::proc_bsdinfo).cast();
        // SAFETY: the output buffer is writable and exactly `size` bytes.
        let written =
            unsafe { libc::proc_pidinfo(pid as i32, libc::PROC_PIDTBSDINFO, 0, buffer, size) };
        assert_eq!(written, size, "process {pid} is not readable by this user");
        (info.pbi_ruid, info.pbi_uid)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let status = fs::read_to_string(format!("/proc/{pid}/status")).unwrap();
        let line = status.lines().find_map(|line| line.strip_prefix("Uid:"));
        let mut ids = line.unwrap().split_whitespace();
        let mut ids = std::iter::from_fn(|| ids.next()?.parse().ok());
        (ids.next().unwrap(), ids.next().unwrap())
    }
}

/// A request as the relay forwarded it, and the daemon's error code if any.
type Forwarded = (Request, Option<String>);

/// REGISTER, CLAIM or ATTACH for a start request; otherwise the operation.
fn step(request: &Request) -> &str {
    let action = request.payload["action"].as_str();
    action.unwrap_or(&request.operation)
}

/// A local byte relay on the legacy lifecycle route. It passes its accepted
/// upstream socket to the daemon and may alter a request before forwarding.
/// Newer capability probes are unsupported there, as on a legacy peer.
struct Relay {
    path: PathBuf,
    seen: Arc<Mutex<Vec<Forwarded>>>,
    stop: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}

impl Relay {
    fn start(f: &runtime::Fixture, alter: fn(&mut Request)) -> Self {
        let path = f.root.join("relay.sock");
        let listener = UnixListener::bind(&path).unwrap();
        listener.set_nonblocking(true).unwrap();
        let backend = f.socket.clone();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let (log, stopped) = (Arc::clone(&seen), Arc::clone(&stop));
        let worker = Some(thread::spawn(move || {
            while !stopped.load(Ordering::Acquire) {
                let Ok((mut upstream, _)) = listener.accept() else {
                    thread::sleep(Duration::from_millis(1));
                    continue;
                };
                // Darwin lets an accepted socket inherit the listener's mode.
                upstream.set_nonblocking(false).unwrap();
                let _ = upstream.set_read_timeout(Some(WATCHDOG));
                let Ok(mut request) = arc_ipc::read_message::<_, Request>(&mut upstream) else {
                    continue;
                };
                if matches!(
                    request.operation.as_str(),
                    arc_ipc::policy::OPERATION_POLICY_CAPABILITIES
                        | arc_ipc::governor::OPERATION_CAPABILITIES
                        | arc_ipc::traffic::OPERATION_TRAFFIC_CAPABILITIES
                ) {
                    let code = arc_ipc::ERROR_UNSUPPORTED_CAPABILITY;
                    let response = Response::error(&request.request_id, code);
                    let _ = arc_ipc::write_response(&mut upstream, &response);
                    continue;
                }
                alter(&mut request);
                let mut target = runtime::connect(&backend);
                super::relay::send(&mut target, &[&upstream], &request);
                // A closed daemon connection closes the upstream one as well.
                let Ok(response) = arc_ipc::read_response(&mut target) else {
                    continue;
                };
                let _ = arc_ipc::write_response(&mut upstream, &response);
                let error = response.error.map(|error| error.code);
                log.lock().unwrap().push((request, error));
            }
        }));
        Self {
            path,
            seen,
            stop,
            worker,
        }
    }

    /// The job the daemon accepted from a forwarded registration.
    fn registered(&self) -> JobId {
        let seen = self.seen.lock().unwrap();
        let accepted =
            |(request, error): &&Forwarded| step(request) == "REGISTER" && error.is_none();
        let (request, _) = seen.iter().find(accepted).expect("relayed registration");
        serde_json::from_value(request.payload["job_id"].clone()).unwrap()
    }

    fn finish(mut self) -> Vec<Forwarded> {
        self.stop.store(true, Ordering::Release);
        self.worker.take().unwrap().join().unwrap();
        std::mem::take(&mut *self.seen.lock().unwrap())
    }
}

impl Drop for Relay {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}

/// Send a lifecycle request whose proof callback is served by this test
/// process and would confirm anything. Returns the daemon's answer and whether
/// the daemon disclosed a challenge to that callback.
fn with_forged_callback(f: &runtime::Fixture, operation: &str, payload: Value) -> (Response, bool) {
    let path = f.root.join("liar.sock");
    // Within the daemon's callback path bound: only the peer can be refused.
    assert!(path.to_str().unwrap().len() <= 100);
    let listener = UnixListener::bind(&path).unwrap();
    listener.set_nonblocking(true).unwrap();
    let [reached, asked, done] = [(); 3].map(|()| AtomicBool::new(false));
    let id = format!("arc-proof:{}#{}", path.display(), "b".repeat(32));
    let mut request = Request::new(id, operation);
    request.payload = payload;
    let response = thread::scope(|scope| {
        scope.spawn(|| {
            while !done.load(Ordering::Acquire) {
                let Ok((mut stream, _)) = listener.accept() else {
                    thread::sleep(Duration::from_millis(1));
                    continue;
                };
                let _ = stream.set_nonblocking(false);
                let _ = stream.set_read_timeout(Some(Duration::from_secs(1)));
                if let Ok(challenge) = arc_ipc::read_message::<_, Value>(&mut stream) {
                    asked.store(true, Ordering::Release);
                    let reply = json!({"kind":"arc.relay.response.v1","nonce":challenge["nonce"]});
                    let _ = arc_ipc::write_message(&mut stream, &reply);
                }
                reached.store(true, Ordering::Release);
            }
        });
        let response = arc_ipc::exchange(&mut f.connect(), &request);
        // Observe the daemon's connection before the callback stops.
        let deadline = Instant::now() + WATCHDOG;
        while !reached.load(Ordering::Acquire) && Instant::now() < deadline {
            thread::yield_now();
        }
        done.store(true, Ordering::Release);
        response
    });
    assert!(reached.into_inner(), "daemon never tried the callback");
    (response.unwrap(), asked.into_inner())
}

/// A registration naming the daemon, a live same-user process, as observer.
fn naming_daemon(n: u32, daemon: &runtime::OwnedChild) -> Value {
    let mut forged = protocol::register(n);
    let StartRequest::Register { observer, .. } = &mut forged else {
        unreachable!()
    };
    *observer = ProcessIdentity::capture(daemon.child.id()).unwrap();
    json!(forged)
}

#[test]
fn built_cli_current_user_socket_and_job_ownership() {
    let _serial = support::serial();
    let f = runtime::Fixture::new();
    let mut daemon = f.spawn_isolated("test");
    let me = unsafe { (libc::getuid(), libc::geteuid()) };
    // The endpoint is a socket this user owns, closed to group and others,
    // served by a daemon running as this user.
    let socket = fs::symlink_metadata(&f.socket).unwrap();
    assert!(socket.file_type().is_socket());
    assert_eq!(socket.uid(), me.1);
    assert_eq!(socket.mode() & 0o7777, 0o600);
    assert_eq!(users(daemon.child.id()), me);

    let mut owned = held(&f, &f.socket, "owned");
    let (mut barrier, report) = cli::accept(&owned.listener);
    let wrapper = owned.wrapper.child.id();
    let child: ProcessIdentity = serde_json::from_value(report["identity"].clone()).unwrap();
    assert_eq!(report["ppid"].as_u64(), Some(u64::from(wrapper)));
    let job_id = cli::recent(&f);
    // While both are alive: the job is bound to the authenticated peer that
    // registered it and to the child that peer itself launched.
    let job = running(&f, &job_id);
    assert_eq!(job.observer, ProcessIdentity::capture(wrapper).unwrap());
    assert_eq!(job.child.as_ref(), Some(&child));
    assert!(job.observer_verified_at_ms > 0);
    assert!(job.child_verified_at_ms.is_some());
    assert_eq!(users(wrapper), me);
    assert_eq!(users(child.pid), me);

    barrier.write_all(b"x").unwrap();
    let status = owned.wrapper.wait("isolated managed CLI").unwrap();
    assert_eq!(status.code(), Some(17));
    let done = protocol::status(&f, &job_id);
    assert_eq!(done.outcome, Some(Outcome::Exited { code: 17 }));
    assert_eq!((&done.observer, &done.child), (&job.observer, &job.child));
    assert_eq!(starts(&owned.marker), 1);
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn forged_or_mismatched_job_identity_is_rejected() {
    let _serial = support::serial();
    let f = runtime::Fixture::new();
    let mut daemon = f.spawn_isolated("test");
    let attach = |n: u32, child: &ProcessIdentity| {
        let request = StartRequest::Attach {
            job_id: protocol::id(n),
            observer: protocol::observer(),
            child: child.clone(),
        };
        protocol::call(&f, OPERATION_JOB_START, json!(request))
    };

    // A live same-user process (the daemon) named as observer by another peer.
    let forged = naming_daemon(801, &daemon);
    invalid(protocol::call(&f, OPERATION_JOB_START, forged));
    assert_eq!(rows(&f, 801), 0);

    // This peer's own jobs. A live same-user process outside its process tree
    // is not its child, and neither is the observer itself.
    for n in [802, 807] {
        protocol::start(&f, &protocol::register(n));
        let observer = protocol::observer();
        let job_id = protocol::id(n);
        protocol::start(&f, &StartRequest::Claim { job_id, observer });
    }
    let unrelated = ProcessIdentity::capture(unsafe { libc::getppid() } as u32).unwrap();
    invalid(attach(802, &unrelated));
    invalid(attach(802, &protocol::observer()));
    assert!(protocol::status(&f, &protocol::id(802)).child.is_none());
    // A process this user cannot verify (launchd, PID 1) is refused or kept
    // as an unknown observation; it never becomes a verified, running child.
    let launchd = ProcessIdentity::capture(1).unwrap_or_else(|_| ProcessIdentity {
        pid: 1,
        start_token: "1".into(),
        ..protocol::observer()
    });
    if let Some(error) = attach(807, &launchd).error {
        assert_eq!(error.code, arc_ipc::ERROR_INVALID_REQUEST);
    }
    let unverified = protocol::status(&f, &protocol::id(807));
    assert!(unverified.child_verified_at_ms.is_none());
    assert_ne!(unverified.phase, Phase::Running);

    // Supported relay: the built CLI reaches the daemon only through a relay
    // that forwards its requests unchanged. The daemon's direct peer is the
    // relay, yet the job is bound to the CLI and its own child.
    let relay = Relay::start(&f, |_| ());
    let mut victim = held(&f, &relay.path, "victim");
    let (mut barrier, report) = cli::accept(&victim.listener);
    let child: ProcessIdentity = serde_json::from_value(report["identity"].clone()).unwrap();
    let observer = ProcessIdentity::capture(victim.wrapper.child.id()).unwrap();
    let job_id = relay.registered();
    let job = running(&f, &job_id);
    assert_eq!(job.observer, observer);
    assert_eq!(job.child.as_ref(), Some(&child));
    assert!(job.child_verified_at_ms.is_some());

    // This test process is another live same-user peer. It cannot act on that
    // live job as the CLI, with or without a callback of its own, ...
    let complete = |observer: &ProcessIdentity| {
        json!(CompleteRequest {
            job_id: job_id.clone(),
            observer: observer.clone(),
            outcome: Outcome::Exited { code: 0 },
        })
    };
    let forged = complete(&observer);
    invalid(protocol::call(&f, OPERATION_JOB_COMPLETE, forged.clone()));
    let (response, disclosed) = with_forged_callback(&f, OPERATION_JOB_COMPLETE, forged);
    invalid(response);
    assert!(!disclosed);
    let replace = StartRequest::Attach {
        job_id: job_id.clone(),
        observer: observer.clone(),
        child: unrelated,
    };
    invalid(protocol::call(&f, OPERATION_JOB_START, json!(replace)));
    // ... and its own genuine identity does not match the job's observer.
    let own = protocol::call(&f, OPERATION_JOB_COMPLETE, complete(&protocol::observer()));
    assert_eq!(own.error.unwrap().code, ERROR_JOB_CONFLICT);
    let after = protocol::status(&f, &job_id);
    assert_eq!((after.phase, &after.outcome), (Phase::Running, &None));
    assert_eq!((&after.observer, &after.child), (&job.observer, &job.child));

    barrier.write_all(b"x").unwrap();
    let status = victim.wrapper.wait("relayed managed CLI").unwrap();
    assert_eq!(status.code(), Some(17));
    let seen = relay.finish();
    for expected in ["REGISTER", "CLAIM", "ATTACH", OPERATION_JOB_COMPLETE] {
        let accepted = seen.iter().any(|(request, error)| {
            step(request) == expected
                && error.is_none()
                && request.payload["job_id"] == job_id.as_str()
                && request.payload["observer"]["pid"] == observer.pid
        });
        assert!(accepted, "relayed {expected} was not accepted: {seen:?}");
    }
    let done = protocol::status(&f, &job_id);
    assert_eq!(done.outcome, Some(Outcome::Exited { code: 17 }));
    assert_eq!(starts(&victim.marker), 1);
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn relay_cannot_use_outgoing_socket_to_impersonate_daemon() {
    let _serial = support::serial();
    let f = runtime::Fixture::new();
    let mut daemon = f.spawn_isolated("test");
    let mut request = Request::new("relay-forgery", OPERATION_JOB_START);
    request.payload = naming_daemon(803, &daemon);
    let mut target = f.connect();
    let outgoing = target.try_clone().unwrap();
    invalid(super::relay::exchange(&mut target, &outgoing, &request));
    assert_eq!(rows(&f, 803), 0);
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn relay_challenge_to_daemon_socket_cannot_replay_a_lifecycle_mutation() {
    let _serial = support::serial();
    let f = runtime::Fixture::new();
    let mut daemon = f.spawn_isolated("test");
    // Point the forged callback at the daemon itself. A challenge must not be
    // a valid daemon request: otherwise this would recursively commit as the
    // daemon's own authenticated peer before the outer request is rejected.
    let mut request = Request::new(
        format!("arc-proof:{}#{}", f.socket.display(), "a".repeat(32)),
        OPERATION_JOB_START,
    );
    request.payload = naming_daemon(806, &daemon);
    invalid(arc_ipc::exchange(&mut f.connect(), &request).unwrap());
    assert_eq!(rows(&f, 806), 0);
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn multiple_relay_descriptors_close_the_request_without_committing() {
    let _serial = support::serial();
    let f = runtime::Fixture::new();
    let mut daemon = f.spawn_isolated("test");
    let listener = UnixListener::bind(f.root.join("relay.sock")).unwrap();
    let _client = UnixStream::connect(f.root.join("relay.sock")).unwrap();
    let (upstream, _) = listener.accept().unwrap();
    let mut request = Request::new("multiple-relay-fds", OPERATION_JOB_START);
    request.payload = serde_json::to_value(protocol::register(804)).unwrap();
    let mut target = f.connect();
    super::relay::send(&mut target, &[&upstream, &upstream], &request);
    assert!(arc_ipc::read_response(&mut target).is_err());
    assert_eq!(rows(&f, 804), 0);
    let retried = protocol::start(&f, &protocol::register(804));
    assert_eq!(retried.job.job_id, protocol::id(804));
    daemon.stop(libc::SIGTERM);
    f.report();
}

#[test]
fn relay_holding_upstream_cannot_change_the_observers_request() {
    let _serial = support::serial();
    let f = runtime::Fixture::new();
    let mut daemon = f.spawn_isolated("test");
    // The relay really holds the CLI's accepted socket but changes the job key
    // before forwarding the registration that the CLI's callback would prove.
    let relay = Relay::start(&f, |request| {
        if request.payload["action"] == "REGISTER" {
            request.payload["job_id"] = json!(protocol::id(805));
        }
    });
    let mut altered = held(&f, &relay.path, "altered");
    let pid = altered.wrapper.child.id();
    let status = altered.wrapper.wait("relay forgery CLI").unwrap();
    assert!(!status.success());
    let seen = relay.finish();
    let registration = seen.iter().find(|(request, _)| step(request) == "REGISTER");
    let (registration, error) = registration.expect("registration reached relay");
    assert_eq!(registration.payload["observer"]["pid"], pid);
    assert_eq!(error.as_deref(), Some(arc_ipc::ERROR_INVALID_REQUEST));
    // The fixture child records every start; none happened.
    assert!(!altered.marker.exists());
    assert_eq!(rows(&f, 805), 0);
    daemon.stop(libc::SIGTERM);
    f.report();
}

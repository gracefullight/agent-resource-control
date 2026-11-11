use super::{
    cli::{binary, child_command, command, recent, wait},
    protocol,
    runtime::Fixture,
    support,
};
use arc_ipc::{
    jobs::{Outcome, OPERATION_JOB_COMPLETE, OPERATION_JOB_START},
    Request,
};
use std::{
    collections::BTreeMap,
    fs,
    os::unix::{
        fs::symlink,
        net::{UnixListener, UnixStream},
    },
    process::Stdio,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};
struct Proxy {
    stop: Arc<AtomicBool>,
    join: Option<thread::JoinHandle<BTreeMap<String, usize>>>,
}
impl Proxy {
    fn start(f: &Fixture, mode: &'static str) -> (Self, std::path::PathBuf) {
        let path = f.root.join("proxy.sock");
        let listener = UnixListener::bind(&path).unwrap();
        listener.set_nonblocking(true).unwrap();
        let backend = f.socket.clone();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let join = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(15);
            let mut counts = BTreeMap::new();
            while !stopped.load(Ordering::SeqCst) {
                assert!(Instant::now() < deadline, "proxy fixture deadline");
                let (mut stream, _) = match listener.accept() {
                    Ok(s) => s,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(1));
                        continue;
                    }
                    Err(e) => panic!("{e}"),
                };
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let request: Request = arc_ipc::read_message(&mut stream).unwrap();
                let key = if request.operation == OPERATION_JOB_START {
                    request.payload["action"].as_str().unwrap().to_string()
                } else {
                    request.operation.clone()
                };
                let count = counts.entry(key.clone()).or_insert(0);
                *count += 1;
                if mode == "before-register"
                    || mode == "completion" && key == OPERATION_JOB_COMPLETE
                    || mode == "around-commit" && *count == 1
                {
                    continue;
                }
                let mut target = UnixStream::connect(&backend).unwrap();
                target
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let response = arc_ipc::exchange(&mut target, &request).unwrap();
                if mode == "around-commit" && *count == 2 {
                    continue;
                }
                let _ = arc_ipc::write_response(&mut stream, &response);
            }
            counts
        });
        (
            Self {
                stop,
                join: Some(join),
            },
            path,
        )
    }
    fn finish(mut self) -> BTreeMap<String, usize> {
        self.stop.store(true, Ordering::SeqCst);
        self.join.take().unwrap().join().unwrap()
    }
}
impl Drop for Proxy {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}
#[test]
fn cli_has_one_child_under_before_after_commit_reply_loss_and_retains_exit17() {
    let _serial = support::serial();
    for mode in ["around-commit", "completion", "before-register"] {
        let f = Fixture::new();
        let mut daemon = f.spawn("test");
        let (proxy, path) = Proxy::start(&f, mode);
        let marker = f.root.join("invocations");
        let mut c = super::historical::before_register(child_command(&f, true), mode);
        c.env(arc_core::ARC_SOCKET_PATH, path)
            .env("ARC_FIXTURE_MARKER", &marker)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = c.spawn().unwrap();
        wait(&mut child);
        let output = child.wait_with_output().unwrap();
        let counts = proxy.finish();
        if mode == "before-register" {
            assert_eq!(output.status.code(), Some(1));
            assert!(!marker.exists());
            assert!(output.stderr.starts_with(b"NOT_STARTED\n"));
        } else {
            assert_eq!(output.status.code(), Some(17));
            assert_eq!(fs::read_to_string(marker).unwrap().lines().count(), 1);
            if mode == "around-commit" {
                for key in ["REGISTER", "CLAIM", "ATTACH", OPERATION_JOB_COMPLETE] {
                    assert_eq!(counts[key], 3);
                }
                assert_eq!(output.stderr, b"fixture-stderr\n");
                let (state, code): (String, i64) = rusqlite::Connection::open(&f.database)
                    .unwrap()
                    .query_row("SELECT state,exit_code FROM jobs", [], |r| {
                        Ok((r.get(0)?, r.get(1)?))
                    })
                    .unwrap();
                assert_eq!((state.as_str(), code), ("FAILED", 17));
                assert_eq!(
                    protocol::status(&f, &recent(&f)).outcome,
                    Some(Outcome::Exited { code: 17 })
                );
            } else {
                assert!(String::from_utf8_lossy(&output.stderr).contains("COMPLETION_UNCONFIRMED"));
                assert!(protocol::status(&f, &recent(&f)).outcome.is_none());
            }
        }
        daemon.stop(libc::SIGTERM);
        f.report();
    }
}
#[test]
fn path_lookup_remote_and_unknown_commands_execute_without_resource_reservations() {
    let _serial = support::serial();
    let f = Fixture::new();
    let mut daemon = f.spawn("test");
    for (name, args, class) in [
        ("gh", vec!["workflow", "run"], arc_ipc::JobClass::RemoteCi),
        (
            "ssh",
            vec!["host", "command"],
            arc_ipc::JobClass::RemoteExec,
        ),
        ("unknown-tool", vec!["|", ""], arc_ipc::JobClass::Unknown),
    ] {
        symlink(binary("arc-exec-fixture"), f.root.join(name)).unwrap();
        let marker = f.root.join(format!("{name}-invocations"));
        let out = command(&f)
            .args(["exec", "--", name])
            .args(args)
            .env("PATH", &f.root)
            .env("ARC_FIXTURE_MODE", "echo")
            .env("ARC_FIXTURE_MARKER", &marker)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert_eq!(out.status.code(), Some(17), "{out:?}");
        assert_eq!(out.stderr, b"fixture-stderr\n");
        assert_eq!(fs::read_to_string(marker).unwrap().lines().count(), 1);
        assert_eq!(
            protocol::status(&f, &recent(&f))
                .classification
                .classification,
            class
        );
    }
    let db = rusqlite::Connection::open(&f.database).unwrap();
    assert_eq!(
        db.query_row("SELECT count(*) FROM permits", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
    drop(db);
    daemon.stop(libc::SIGTERM);
    f.report();
}

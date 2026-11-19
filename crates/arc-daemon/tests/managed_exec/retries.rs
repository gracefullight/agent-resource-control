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
            let deadline = Instant::now() + Duration::from_secs(60);
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
                    .set_read_timeout(Some(Duration::from_secs(60)))
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
                    .set_read_timeout(Some(Duration::from_secs(60)))
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
        let mut child = super::runtime::OwnedChild::new(c.spawn().unwrap());
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
        // GH_HOST alone does not establish the effective repository host.
        ("gh", vec!["workflow", "run"], arc_ipc::JobClass::Unknown),
        (
            "ssh",
            vec!["host", "command"],
            // A hostname without endpoint evidence is no longer a remote fact.
            arc_ipc::JobClass::Unknown,
        ),
        ("unknown-tool", vec!["|", ""], arc_ipc::JobClass::Unknown),
    ] {
        symlink(binary("arc-exec-fixture"), f.root.join(name)).unwrap();
        let marker = f.root.join(format!("{name}-invocations"));
        let mut invocation = command(&f);
        invocation
            .args(["exec", "--", name])
            .args(args)
            .env("PATH", &f.root)
            .env("GH_HOST", "github.com")
            .env_remove("GH_REPO")
            .env("ARC_FIXTURE_MODE", "echo")
            .env("ARC_FIXTURE_MARKER", &marker)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = super::runtime::OwnedChild::new(invocation.spawn().unwrap());
        wait(&mut child);
        let out = child.wait_with_output().unwrap();
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

// The wrapper owns a terminal-free session; ARC creates a separate process
// group for its child. Retain both groups for bounded cleanup on test failure.
pub(super) mod unknown_owned {
    use super::super::remote_unknown::kill_target_group;
    use arc_core::process_identity::ProcessIdentity;
    use std::{
        io,
        os::{fd::AsRawFd, unix::process::CommandExt},
        path::{Path, PathBuf},
        process::{Child, Command, ExitStatus, Output, Stdio},
        thread,
        time::{Duration, Instant},
    };

    const WATCHDOG: Duration = Duration::from_secs(60);
    pub(crate) struct SessionChild {
        pub child: Child,
        target_hint: Option<PathBuf>,
        target: Option<ProcessIdentity>,
        stdout: Vec<u8>,
        stderr: Vec<u8>,
        stdout_closed: bool,
        stderr_closed: bool,
        reaped: bool,
    }
    impl SessionChild {
        pub(crate) fn spawn(mut command: Command, target_hint: Option<&Path>) -> io::Result<Self> {
            command.stderr(Stdio::piped());
            // SAFETY: only async-signal-safe system calls run before exec.
            unsafe {
                command.pre_exec(move || {
                    if libc::setsid() < 0 {
                        return Err(io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
            let child = command.spawn()?;
            let owner = Self {
                child,
                target_hint: target_hint.map(Path::to_path_buf),
                target: None,
                stdout: Vec::new(),
                stderr: Vec::new(),
                stdout_closed: false,
                stderr_closed: false,
                reaped: false,
            };
            if let Some(stdout) = &owner.child.stdout {
                super::super::remote_unknown::set_nonblocking(stdout.as_raw_fd())?;
            }
            if let Some(stderr) = &owner.child.stderr {
                super::super::remote_unknown::set_nonblocking(stderr.as_raw_fd())?;
            }
            Ok(owner)
        }
        pub(crate) fn id(&self) -> u32 {
            self.child.id()
        }
        pub(crate) fn track_target(&mut self, target: ProcessIdentity) {
            self.target = Some(target);
        }
        // WNOWAIT pins the group leader's PID until after group cleanup, so
        // even a fast wrapper exit cannot cause a recycled group to be killed.
        pub(crate) fn exited(&self) -> io::Result<bool> {
            let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
            let result = unsafe {
                libc::waitid(
                    libc::P_PID,
                    self.id() as libc::id_t,
                    &mut info,
                    libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
                )
            };
            if result < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(unsafe { info.si_pid() } != 0)
        }

        fn signal_wrapper_group(&self, signal: i32) {
            // The unreaped session leader pins this exclusively owned group ID.
            if unsafe { libc::kill(-(self.id() as i32), signal) } != 0 {
                let error = io::Error::last_os_error();
                if error.raw_os_error() != Some(libc::ESRCH) {
                    eprintln!("owned session {} cleanup failed: {error}", self.id());
                }
            }
        }

        pub(crate) fn output(&mut self, status: ExitStatus) -> Output {
            Output {
                status,
                stdout: std::mem::take(&mut self.stdout),
                stderr: std::mem::take(&mut self.stderr),
            }
        }
        pub(crate) fn wait(&mut self) -> io::Result<ExitStatus> {
            let deadline = Instant::now() + WATCHDOG;
            loop {
                if !self.stderr_closed {
                    self.stderr_closed = match &mut self.child.stderr {
                        Some(stderr) => {
                            super::super::remote_unknown::drain_stream(stderr, &mut self.stderr)?
                        }
                        None => true,
                    };
                }
                if !self.stdout_closed {
                    self.stdout_closed = match &mut self.child.stdout {
                        Some(stdout) => {
                            super::super::remote_unknown::drain_stream(stdout, &mut self.stdout)?
                        }
                        None => true,
                    };
                }
                if self.exited()? && self.stderr_closed && self.stdout_closed {
                    let status = self.child.wait()?;
                    self.reaped = true;
                    return Ok(status);
                }
                if Instant::now() >= deadline {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        format!("owned session {} exceeded {WATCHDOG:?}", self.id()),
                    ));
                }
                thread::sleep(Duration::from_millis(2));
            }
        }
    }

    impl Drop for SessionChild {
        fn drop(&mut self) {
            if self.reaped {
                return;
            }
            if !kill_target_group(self.id(), &self.target, self.target_hint.as_deref()) {
                // Before PID publication, the gated fixture has no descendants;
                // ARC forwards this signal to any child already being spawned.
                self.signal_wrapper_group(libc::SIGTERM);
            }
            let deadline = Instant::now() + WATCHDOG;
            loop {
                match self.child.try_wait() {
                    Ok(Some(_)) => return,
                    Ok(None) if Instant::now() < deadline => {
                        kill_target_group(self.id(), &self.target, self.target_hint.as_deref());
                        thread::sleep(Duration::from_millis(2));
                    }
                    Err(error) => {
                        eprintln!("owned session {} wait failed: {error}", self.id());
                        return;
                    }
                    Ok(None) => {
                        eprintln!("owned session {} exceeded its watchdog", self.id());
                        kill_target_group(self.id(), &self.target, self.target_hint.as_deref());
                        self.signal_wrapper_group(libc::SIGKILL);
                        let hard_deadline = Instant::now() + WATCHDOG;
                        while Instant::now() < hard_deadline {
                            if !matches!(self.child.try_wait(), Ok(None)) {
                                return;
                            }
                            thread::sleep(Duration::from_millis(2));
                        }
                        eprintln!("owned session {} survived SIGKILL", self.id());
                        return;
                    }
                }
            }
        }
    }

    #[test]
    fn unwind_cleans_prepublication_and_stuck_subprocess_without_touching_other_processes() {
        use super::super::{cli::command, runtime::OwnedChild, support};
        use arc_core::process_identity::{probe, IdentityStatus, ProcessIdentity};
        use std::{fs, io::Write, os::unix::fs::PermissionsExt, process::Stdio};

        let _serial = support::serial();
        let f = super::super::runtime::Fixture::new();
        let mut daemon = f.spawn("test");
        let program = f.root.join("stalled-custom-program");
        fs::write(
            &program,
            r#"#!/bin/sh
if [ "$STALL" = pre ]; then
  printf '%s\n' "$$" > "$READY"
  IFS= read -r unused
else
  printf '%s\n' "$$" > "$PIDFILE"
  IFS= read -r unused || exit 126
  /bin/sleep 600 &
  printf '%s %s\n' "$$" "$!" > "$READY"
  wait "$!"
fi
"#,
        )
        .unwrap();
        fs::set_permissions(&program, fs::Permissions::from_mode(0o700)).unwrap();
        let mut unrelated = OwnedChild::new(
            Command::new("/bin/sleep")
                .arg("600")
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let unrelated_identity = ProcessIdentity::capture(unrelated.id()).unwrap();
        for mode in ["pre", "subprocess"] {
            let ready = f.root.join(format!("{mode}-ready"));
            let unpublished = f.root.join("custom-child-pid");
            let mut invocation = command(&f);
            invocation
                .args(["exec", "--"])
                .arg(&program)
                .env("STALL", mode)
                .env("READY", &ready)
                .env("PIDFILE", &unpublished)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            let mut owner = SessionChild::spawn(invocation, Some(&unpublished)).unwrap();
            let wrapper = ProcessIdentity::capture(owner.id()).unwrap();
            let deadline = Instant::now() + WATCHDOG;
            if mode == "subprocess" {
                let pid = loop {
                    if let Some(pid) = fs::read_to_string(&unpublished)
                        .ok()
                        .and_then(|text| text.trim().parse().ok())
                    {
                        break pid;
                    }
                    assert!(
                        !owner.exited().unwrap(),
                        "wrapper exited before PID publication"
                    );
                    assert!(Instant::now() < deadline, "PID was not published");
                    thread::sleep(Duration::from_millis(2));
                };
                owner.track_target(ProcessIdentity::capture(pid).unwrap());
                let input = owner.child.stdin.as_mut().unwrap();
                input.write_all(b"go\n").unwrap();
            }
            let identities = loop {
                let ids: Vec<_> = fs::read_to_string(&ready)
                    .unwrap_or_default()
                    .split_whitespace()
                    .filter_map(|value| value.parse::<u32>().ok())
                    .collect();
                if ids.len() == if mode == "pre" { 1 } else { 2 } {
                    let group = ids[0] as i32;
                    break ids
                        .into_iter()
                        .map(|pid| {
                            assert_eq!(unsafe { libc::getsid(pid as i32) }, owner.id() as i32);
                            assert_eq!(unsafe { libc::getpgid(pid as i32) }, group);
                            ProcessIdentity::capture(pid).unwrap()
                        })
                        .collect::<Vec<_>>();
                }
                assert!(
                    !owner.exited().unwrap(),
                    "{mode}: wrapper exited before stall"
                );
                assert!(Instant::now() < deadline, "{mode}: stall was not reached");
                thread::sleep(Duration::from_millis(2));
            };
            if mode == "pre" {
                assert!(!unpublished.exists(), "PID publication was not withheld");
            } else {
                assert_eq!(
                    fs::read_to_string(&unpublished).unwrap().trim(),
                    identities[0].pid.to_string()
                );
                fs::remove_file(&unpublished).unwrap();
            }
            let result = std::panic::catch_unwind(|| {
                let _owner = owner;
                panic!("injected {mode} assertion failure");
            });
            assert!(result.is_err());
            super::super::remote_unknown::assert_descendants_gone(identities, mode);
            super::super::remote_unknown::assert_descendants_gone(vec![wrapper], mode);
            assert_eq!(probe(&unrelated_identity), IdentityStatus::VerifiedMatch);
            assert!(unrelated.try_wait().unwrap().is_none());
        }
        unrelated.kill().unwrap();
        unrelated.wait().unwrap();
        daemon.stop(libc::SIGTERM);
        f.report();
    }
}

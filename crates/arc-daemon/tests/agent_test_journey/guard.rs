//! Drop and watchdog cleanup of a managed target that EOF and SIGTERM leave
//! alive in its own process group.
use super::*;
use arc_core::process_identity::{probe_live, IdentityStatus, ProcessIdentity};
use std::{
    os::unix::net::UnixListener,
    thread,
    time::{Duration, Instant},
};

/// The target's report over its control socket; it then blocks reading it.
fn report(listener: &UnixListener) -> (std::os::unix::net::UnixStream, Value) {
    listener.set_nonblocking(true).unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut stream = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                assert!(Instant::now() < deadline, "target report: 60 s watchdog");
                thread::sleep(Duration::from_millis(2));
            }
            Err(error) => panic!("target report: {error}"),
        }
    };
    stream.set_nonblocking(false).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(60)))
        .unwrap();
    let value = serde_json::from_slice(&arc_ipc::read_frame(&mut stream).unwrap()).unwrap();
    (stream, value)
}

// Retain a separately owned cleanup guard so a failed regression assertion
// cannot leave the deliberately uncooperative target running.
struct TargetGuard(ProcessIdentity);

impl Drop for TargetGuard {
    fn drop(&mut self) {
        if probe_live(&self.0) == IdentityStatus::VerifiedMatch {
            // SAFETY: the current process identity still matches our fixture.
            unsafe { libc::kill(self.0.pid as libc::pid_t, libc::SIGKILL) };
        }
    }
}

pub(super) fn run() {
    exercise(false);
}

pub(super) fn timeout() {
    exercise(true);
}

fn exercise(timeout: bool) {
    let f = Fixture::new();
    let s = Sandbox::provisioned(&f);
    let project = s.project("guard");
    s.stub("cargo");
    let mut daemon = f.spawn_isolated("");
    let installed = install_hook(&f, &s, &project);
    let event = pre_tool_use(&project, "cargo build");
    let command = returned_command(&f, &s, &project, &installed, &event);
    stage::assert_managed(&command, "cargo build");

    let control = f.root.join("guard.ctl");
    let listener = UnixListener::bind(&control).unwrap();
    let mut shell = s.shell(&f, &project, &command);
    shell
        .env("ARC_FIXTURE_MODE", "ignored-term")
        .env("ARC_FIXTURE_MARKER", f.root.join("guard-starts"))
        .env("ARC_FIXTURE_CONTROL", &control);
    let mut run = Run::start(&mut shell, &f.root, "guard");
    let (stream, reported) = report(&listener);
    let target: ProcessIdentity = serde_json::from_value(reported["identity"].clone()).unwrap();
    let _target_guard = TargetGuard(target.clone());
    let wrapper_group = run.pid() as i64;
    assert_eq!(reported["pgid"], json!(target.pid), "{reported}");
    assert_ne!(reported["pgid"], json!(wrapper_group), "{reported}");
    assert_eq!(probe_live(&target), IdentityStatus::VerifiedMatch);
    run.assert_stdin_cloexec();

    // EOF and SIGTERM leave it running: it ignores SIGTERM and never reads
    // standard input. An ignored signal is discarded when it is sent, so the
    // liveness check that follows is not a race.
    run.input(b"");
    assert_eq!(
        unsafe { libc::kill(target.pid as libc::pid_t, libc::SIGTERM) },
        0
    );
    assert!(run.alive(), "{}", String::from_utf8_lossy(&run.stderr()));
    assert_eq!(probe_live(&target), IdentityStatus::VerifiedMatch);

    let root = run.pid() as libc::pid_t;
    if timeout {
        // The target is known to be running; shorten only this injected
        // deadline. Normal journey commands retain the 60-second watchdog.
        let error = run
            .finish_for("controlled watchdog expiry", Duration::ZERO)
            .err()
            .expect("the live wrapper must exceed the controlled watchdog");
        assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
    } else {
        drop(run);
    }
    journey::eventually("managed target killed on cleanup", || {
        matches!(
            probe_live(&target),
            IdentityStatus::Missing | IdentityStatus::Mismatch
        )
    });
    // The owned wrapper was reaped, not left as a zombie or orphan.
    assert_eq!(unsafe { libc::kill(root, 0) }, -1);
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(libc::ESRCH)
    );
    drop(stream);
    daemon.stop(libc::SIGTERM);
    f.report();
}

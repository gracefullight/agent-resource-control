//! The shared prerequisite collector preserves output and owns its timeout path.
use super::*;
use std::{os::fd::OwnedFd, os::unix::net::UnixStream};

#[test]
fn prerequisite_probe_forced_expiry_terminates_and_reaps_child() {
    let (input, _peer) = UnixStream::pair().unwrap();
    let input: OwnedFd = input.into();
    let mut command = Command::new("/bin/cat");
    command.env_clear().stdin(Stdio::from(input));
    let mut child = spawn_probe(&mut command).unwrap();
    let pid = child.child.id();
    let identity = ProcessIdentity::capture(pid).unwrap();
    let error = collect_probe(&mut child, Duration::ZERO, "blocked fixture probe").unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    assert!(error.to_string().contains(&pid.to_string()));
    assert!(error.to_string().contains("blocked fixture probe"));
    assert!(!child.child.try_wait().unwrap().unwrap().success());
    assert!(matches!(
        probe_live(&identity),
        IdentityStatus::Missing | IdentityStatus::Mismatch
    ));
}

#[test]
fn prerequisite_probe_preserves_stdout_stderr_and_exit_status() {
    let mut command = Command::new("/bin/sh");
    command
        .env_clear()
        .args(["-c", "printf output; printf error >&2; exit 7"]);
    let output = fixture_output(&mut command).unwrap();
    assert_eq!(output.stdout, b"output");
    assert_eq!(output.stderr, b"error");
    assert_eq!(output.status.code(), Some(7));
}

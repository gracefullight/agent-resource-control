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

#[test]
fn cached_fixture_hash_tracks_same_length_rewrites() {
    let dir = std::env::temp_dir().join(format!(
        "arc-hash-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    fs::create_dir(&dir).unwrap();
    let path = dir.join("fixture");
    fs::write(&path, b"abc").unwrap();
    assert_eq!(
        sha(&path),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        sha(&path),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    fs::write(&path, b"xyz").unwrap();
    assert_eq!(
        sha(&path),
        "3608bca1e44ea6c4d268eb6db02260269892c0b42b86bbf1e77a6fa16c3c9282"
    );
    let reference = |path: &Path| {
        let mut command = Command::new("/usr/bin/shasum");
        command.args(["-a", "256"]).arg(path);
        let out = fixture_output(&mut command).unwrap();
        assert!(out.status.success());
        String::from_utf8(out.stdout)
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .to_owned()
    };
    fs::write(&path, vec![b'a'; 8192]).unwrap();
    let first = sha(&path);
    assert_eq!(first, reference(&path));
    assert_eq!(sha(&path), first);
    fs::write(&path, vec![b'b'; 8192]).unwrap();
    let second = sha(&path);
    assert_ne!(first, second);
    assert_eq!(second, reference(&path));
    fs::remove_dir_all(dir).unwrap();
}

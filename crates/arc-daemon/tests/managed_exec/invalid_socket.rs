//! Native invalid socket configuration must not launch a child.
use super::{cli::child_command, runtime::Fixture, support};
use std::{os::unix::net::UnixStream, path::Path, process::Stdio};

fn observe(path: &str, expected_errno: i32) {
    let _serial = support::serial();
    let error = UnixStream::connect(Path::new(path)).unwrap_err();
    assert_eq!(error.raw_os_error(), Some(expected_errno));

    let fixture = Fixture::new();
    let marker = fixture.root.join("invalid-socket-child");
    let output = child_command(&fixture, true)
        .arg("argv-secret-sentinel")
        .env(arc_core::ARC_SOCKET_PATH, path)
        .env("ARC_FIXTURE_MARKER", &marker)
        .stdin(Stdio::null())
        .output()
        .unwrap();
    let diagnostic = String::from_utf8_lossy(&output.stderr);
    eprintln!(
        "invalid socket probe: errno={expected_errno} exit={:?} marker={} stdout_bytes={} not_started={} tracking_started={}",
        output.status.code(),
        marker.exists(),
        output.stdout.len(),
        diagnostic.contains("NOT_STARTED"),
        diagnostic.contains("Command started.")
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(
        !marker.exists(),
        "invalid socket configuration launched a child"
    );
    assert!(output.stdout.is_empty(), "child produced stdout");
    assert_eq!(
        output.stderr,
        b"NOT_STARTED\nLocal execution preparation failed. No child was started.\n"
    );
    assert!(!diagnostic.contains("Command started."));
    for secret in ["argv-secret-sentinel", "environment-secret-sentinel"] {
        assert!(!diagnostic.contains(secret));
    }
}

#[test]
fn invalid_socket_parent_not_directory_does_not_launch() {
    observe("/dev/null/arc.sock", libc::ENOTDIR);
}

#[test]
fn invalid_socket_endpoint_not_socket_does_not_launch() {
    observe("/dev/null", libc::ENOTSOCK);
}

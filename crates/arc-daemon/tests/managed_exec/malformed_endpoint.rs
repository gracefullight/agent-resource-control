//! Address validation must reject malformed configuration before child launch.
use super::{cli::child_command, runtime::Fixture, support};
use std::process::Stdio;

#[test]
fn overlong_socket_configuration_does_not_launch() {
    let _serial = support::serial();
    let fixture = Fixture::new();
    let marker = fixture.root.join("malformed-socket-child");
    let output = child_command(&fixture, true)
        .arg("argv-secret-sentinel")
        .env(arc_core::ARC_SOCKET_PATH, "x".repeat(200))
        .env("ARC_FIXTURE_MARKER", &marker)
        .stdin(Stdio::null())
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(!marker.exists());
    assert!(output.stdout.is_empty());
    assert_eq!(
        output.stderr,
        b"NOT_STARTED\nLocal execution preparation failed. No child was started.\n"
    );
    assert!(!String::from_utf8_lossy(&output.stderr).contains("secret-sentinel"));
}

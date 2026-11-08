use super::*;

#[test]
fn resident_contract_holds_until_sigterm() {
    let _serial = support::serial();
    let fixture = Fixture::new();
    let mut daemon = fixture.spawn();
    wait_until_ready(&mut daemon, &fixture.socket);

    assert!(fs::symlink_metadata(&fixture.socket)
        .unwrap()
        .file_type()
        .is_socket());
    assert_eq!(mode(&fixture.socket), 0o600, "socket mode must be 0600");
    assert_eq!(
        mode(&fixture.directory),
        0o700,
        "socket directory mode must be 0700"
    );
    assert_no_tcp_listener(daemon.id());

    assert_hello(&fixture.socket);

    let mut incompatible = Request::new("resident-v99", OPERATION_HELLO);
    incompatible.protocol_version = 99;
    let mut stream = UnixStream::connect(&fixture.socket).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let response = arc_ipc::exchange(&mut stream, &incompatible).unwrap();
    assert_eq!(response.status, STATUS_ERROR);
    assert_eq!(
        response.error.as_ref().map(|error| error.code.as_str()),
        Some(ERROR_PROTOCOL_INCOMPATIBLE)
    );
    let response = arc_ipc::exchange(
        &mut stream,
        &Request::new("resident-after-v99", OPERATION_HELLO),
    )
    .unwrap();
    assert_eq!(response.status, STATUS_OK);
    assert_eq!(response.protocol_version, PROTOCOL_VERSION);
    assert_hello(&fixture.socket);

    // An oversized declaration must be rejected before waiting for any body.
    let mut oversized = UnixStream::connect(&fixture.socket).unwrap();
    oversized
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    std::io::Write::write_all(
        &mut oversized,
        &(arc_ipc::MAX_FRAME_BYTES + 1).to_be_bytes(),
    )
    .unwrap();
    let response = arc_ipc::read_response(&mut oversized).unwrap();
    assert_eq!(response.status, STATUS_ERROR);
    assert_eq!(response.error.unwrap().code, arc_ipc::ERROR_FRAME_TOO_LARGE);
    assert_hello(&fixture.socket);

    let status = arcctl(&fixture, "status");
    assert!(status.status.success(), "{status:?}");
    let stdout = String::from_utf8(status.stdout).unwrap();
    assert_eq!(
        stdout.trim_end(),
        format!(
            "running. Socket: {}. Protocol: 1. Sweeper off.",
            fixture.socket.display()
        )
    );
    assert!(!stdout.contains("eleven"), "{stdout}");

    let doctor = arcctl(&fixture, "doctor");
    assert!(doctor.status.success(), "{doctor:?}");
    let stdout = String::from_utf8(doctor.stdout).unwrap();
    assert_eq!(
        stdout.trim_end(),
        "ok. IPC owner-only. Tray not connected. Vendors not detected. \
         Command classification not checked. Sweeper off. Schema compatible. Version 2. Migration history verified."
    );
    assert!(!stdout.contains("eleven"), "{stdout}");

    let mut second = fixture.spawn();
    let second_status = wait_for_exit(&mut second);
    assert!(
        !second_status.success(),
        "a second daemon must exit non-zero: {second_status:?}"
    );
    let mut second_stderr = String::new();
    if let Some(pipe) = second.stderr.as_mut() {
        let _ = pipe.read_to_string(&mut second_stderr);
    }
    assert!(
        second_stderr.contains("socket already served"),
        "second daemon stderr: {second_stderr}"
    );
    assert_hello(&fixture.socket);

    let result = unsafe { libc::kill(daemon.id() as libc::pid_t, libc::SIGTERM) };
    assert_eq!(result, 0, "SIGTERM failed");
    let exit = wait_for_exit(&mut daemon);
    assert!(exit.success(), "daemon exit status after SIGTERM: {exit:?}");
    assert!(
        !fixture.socket.exists(),
        "socket {} must be removed on shutdown",
        fixture.socket.display()
    );

    let reopened = rusqlite::Connection::open(&fixture.database).unwrap();
    let integrity: String = reopened
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");
    assert_eq!(
        reopened
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        2
    );
    assert_eq!(
        reopened
            .query_row("SELECT COUNT(*) FROM gc_operations", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    reopened.close().unwrap();
}

#[test]
fn commands_report_daemon_unavailable_when_absent() {
    let _serial = support::serial();
    let fixture = Fixture::new();
    for command in ["status", "doctor"] {
        let output = arcctl(&fixture, command);
        assert!(!output.status.success(), "{command} must fail: {output:?}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            stderr.contains("DAEMON_UNAVAILABLE"),
            "{command} stderr: {stderr}"
        );
    }
}

use super::*;
use arc_ipc::{DoctorReport, SchemaStatus, ERROR_SCHEMA_INCOMPATIBLE, OPERATION_DOCTOR};
use rusqlite::Connection;

#[derive(Debug, PartialEq)]
struct Snapshot {
    records: Vec<(String, String, Option<String>, i64)>,
    history: Vec<(String, i64, String)>,
    objects: Vec<(String, String, Option<String>)>,
    witness: u32,
}
fn snapshot(connection: &Connection) -> Snapshot {
    Snapshot {
        records: connection
            .prepare(
                "SELECT id, display_name, adapter_version, discovered_at FROM vendors ORDER BY id",
            )
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap(),
        history: connection
            .prepare("SELECT id, applied_at, checksum FROM schema_migrations ORDER BY id")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap(),
        objects: connection
            .prepare("SELECT type, name, sql FROM sqlite_schema ORDER BY type, name")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap(),
        witness: connection
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap(),
    }
}

#[test]
fn live_schema_failures_use_bounded_worker_reach_cli_and_preserve_records_and_history() {
    let _serial = support::serial();
    for (sql, expected, reason, count) in [
        ("UPDATE schema_migrations SET checksum='altered'", SchemaStatus::ChecksumMismatch, "migration checksum mismatch", 1),
        ("DELETE FROM schema_migrations", SchemaStatus::MissingHistory, "missing migration history", 0),
        ("UPDATE schema_migrations SET id='0001_other'", SchemaStatus::InvalidHistory, "invalid migration history", 1),
        ("INSERT INTO schema_migrations VALUES ('0002_future',42,'unverified-future'); PRAGMA user_version=2", SchemaStatus::UnsupportedNewer, "unsupported newer schema", 2),
    ] {
        let fixture = Fixture::new();
        let connection = arc_storage::open_and_migrate(&fixture.database).unwrap();
        connection.execute("INSERT INTO vendors VALUES ('kept','Preserved record',NULL,17)", []).unwrap();
        connection.close().unwrap();
        let mut daemon = fixture.spawn();
        wait_until_ready(&mut daemon, &fixture.socket);
        // External mutations are test fixtures only. Production diagnostics remain read-only.
        let connection = Connection::open(&fixture.database).unwrap();
        connection.execute_batch(sql).unwrap();
        let before = snapshot(&connection);
        let mut stream = UnixStream::connect(&fixture.socket).unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        stream.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
        let response = arc_ipc::exchange(&mut stream, &Request::new("schema", OPERATION_DOCTOR)).unwrap();
        assert_eq!(response.protocol_version, 1);
        assert_eq!(response.status, STATUS_ERROR);
        assert_eq!(response.request_id, "schema");
        assert_eq!(response.error.unwrap().code, ERROR_SCHEMA_INCOMPATIBLE);
        let report: DoctorReport = serde_json::from_value(response.result.unwrap()).unwrap();
        assert_eq!(report.integrity_check, "ok");
        assert_eq!(report.schema_migrations, count);
        let schema = report.schema.unwrap();
        assert_eq!(schema.status, expected);
        assert!(!schema.history_verified);
        assert_eq!(schema.supported_version, 1);
        assert_eq!(schema.applied_version, if expected == SchemaStatus::UnsupportedNewer { Some(2) } else { None });
        let output = arcctl(&fixture, "doctor");
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        let text = String::from_utf8(output.stderr).unwrap();
        assert!(text.starts_with("SCHEMA_INCOMPATIBLE: "), "{text}");
        assert!(text.contains(reason), "{text}");
        assert_hello(&fixture.socket);
        assert!(arcctl(&fixture, "status").status.success());
        assert_eq!(snapshot(&connection), before);
        drop(stream);
        assert_eq!(unsafe { libc::kill(daemon.id() as libc::pid_t, libc::SIGTERM) }, 0);
        assert!(wait_for_exit(&mut daemon).success());
        assert!(!fixture.socket.exists());
        assert_eq!(snapshot(&connection), before);
        connection.close().unwrap();
    }
}

#[test]
fn baseline_client_adapter_accepts_actual_new_daemon_production_report() {
    let _serial = support::serial();
    let fixture = Fixture::new();
    let mut daemon = fixture.spawn();
    wait_until_ready(&mut daemon, &fixture.socket);
    let mut stream = UnixStream::connect(&fixture.socket).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    // Literal original request without payload, and baseline count-only validation.
    arc_ipc::write_frame(
        &mut stream,
        br#"{"protocol_version":1,"request_id":"baseline","operation":"diagnostics.doctor"}"#,
    )
    .unwrap();
    let response = arc_ipc::read_response(&mut stream).unwrap();
    assert_eq!(response.protocol_version, 1);
    assert_eq!(response.status, STATUS_OK);
    assert!(response.error.is_none());
    assert_eq!(response.request_id, "baseline");
    let result = response.result.unwrap();
    assert_eq!(result["integrity_check"].as_str(), Some("ok"));
    assert_eq!(result["schema_migrations"].as_i64(), Some(1));
    assert_eq!(result["schema"]["status"], "compatible");
    assert_eq!(result["schema"]["history_verified"], true);
    drop(stream);
    assert_eq!(
        unsafe { libc::kill(daemon.id() as libc::pid_t, libc::SIGTERM) },
        0
    );
    assert!(wait_for_exit(&mut daemon).success());
}

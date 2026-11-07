//! Actual arcctl against bounded, protocol-1 legacy and modern response fixtures.
use arc_ipc::{Request, Response};
use serde_json::{json, Value};
use std::{
    fs,
    os::unix::{fs::DirBuilderExt, net::UnixListener},
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

struct Endpoint(PathBuf);
impl Endpoint {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = arc_core::runtime_root().unwrap().join(format!(
            "c{:x}-{:x}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        Self(root)
    }
}
impl Drop for Endpoint {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn cli(result: Value, error: Option<&str>) -> Output {
    let endpoint = Endpoint::new();
    let socket = endpoint.0.join("arc.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    listener.set_nonblocking(true).unwrap();
    let code = error.map(str::to_owned);
    let server = thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(8);
        let mut stream = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "CLI did not connect");
                    thread::sleep(Duration::from_millis(1));
                }
                Err(error) => panic!("{error}"),
            }
        };
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let request: Request = arc_ipc::read_message(&mut stream).unwrap();
        assert_eq!(request.protocol_version, 1);
        assert_eq!(request.operation, arc_ipc::OPERATION_DOCTOR);
        assert!(request.payload.is_null());
        let mut response = match code {
            Some(code) => Response::error(request.request_id, code),
            None => Response::ok(request.request_id, Value::Null),
        };
        response.result = Some(result);
        arc_ipc::write_response(&mut stream, &response).unwrap();
    });
    let output = Command::new(env!("CARGO_BIN_EXE_arcctl"))
        .arg("doctor")
        .env(arc_core::ARC_SOCKET_PATH, socket)
        .output()
        .unwrap();
    server.join().unwrap();
    output
}

fn legacy(count: i64) -> Value {
    json!({"integrity_check":"ok","schema_migrations":count,
        "operation":"diagnostics.doctor","ipc_owner_only":true,"tray_connected":false,
        "vendors_detected":false,"command_classification":"not_checked","sweeper":"off"})
}
fn modern(status: &str, applied: Option<u32>, supported: u32, verified: bool, count: i64) -> Value {
    let mut result = legacy(count);
    result["schema"] = json!({"status":status,"applied_version":applied,
        "supported_version":supported,"history_verified":verified});
    result
}
const DISCLOSURE: &str = "ok. IPC owner-only. Tray not connected. Vendors not detected. Command classification not checked. Sweeper off.";

#[test]
fn actual_cli_reports_verified_production_and_two_version_fixture_success() {
    for version in [1, 2] {
        let output = cli(
            modern(
                "compatible",
                Some(version),
                version,
                true,
                i64::from(version),
            ),
            None,
        );
        assert!(output.status.success(), "{output:?}");
        assert!(output.stderr.is_empty());
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            format!(
                "{DISCLOSURE} Schema compatible. Version {version}. Migration history verified.\n"
            )
        );
    }
}

#[test]
fn new_client_old_daemon_labels_unavailable_details_without_claiming_verification() {
    let mut explicit_null = legacy(1);
    explicit_null["schema"] = Value::Null;
    for result in [legacy(1), explicit_null] {
        let output = cli(result, None);
        assert!(output.status.success(), "{output:?}");
        assert!(output.stderr.is_empty());
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            format!("{DISCLOSURE} Schema compatibility details unavailable (legacy daemon).\n")
        );
    }
}

#[test]
fn unavailable_and_malformed_present_details_fail_without_legacy_fallback() {
    let mut malformed = legacy(1);
    malformed["schema"] = json!({"status":"compatible","supported_version":1});
    for result in [legacy(2), legacy(0), malformed] {
        let output = cli(result, None);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert_eq!(output.stderr, b"SCHEMA_DETAILS_UNAVAILABLE\n");
    }
}

#[test]
fn every_schema_failure_survives_error_envelope_with_actionable_reason() {
    for (status, reason) in [
        ("missing_history", "missing migration history"),
        ("checksum_mismatch", "migration checksum mismatch"),
        ("invalid_history", "invalid migration history"),
        ("upgrade_required", "schema upgrade required"),
        ("unsupported_newer", "unsupported newer schema"),
        ("future_status", "unrecognized compatibility status"),
        ("compatible", "inconsistent compatibility details"),
    ] {
        let output = cli(
            modern(status, Some(2), 1, false, 2),
            Some("SCHEMA_INCOMPATIBLE"),
        );
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        let error = String::from_utf8(output.stderr).unwrap();
        assert!(error.starts_with("SCHEMA_INCOMPATIBLE: "), "{error}");
        assert!(error.contains(reason), "{error}");
        assert!(error.contains("Applied version: 2. Supported version: 1."));
        if status == "unsupported_newer" {
            assert!(error.contains("use a binary supporting that version"));
            assert!(error.contains("downgrade execution is not supported"));
        }
        if ["missing_history", "checksum_mismatch", "invalid_history"].contains(&status) {
            assert!(error.contains("preserve the database and investigate against a trusted copy"));
        }
        assert!(!error.contains('\u{1b}'));
    }
}

#[test]
fn inconsistent_or_unknown_success_envelopes_cannot_claim_schema_compatibility() {
    for result in [
        modern("future_status", Some(1), 1, true, 1),
        modern("compatible", Some(1), 1, false, 1),
        modern("compatible", Some(2), 2, true, 1),
        modern("compatible", None, 1, true, 1),
    ] {
        let output = cli(result, None);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8(output.stderr)
            .unwrap()
            .starts_with("SCHEMA_INCOMPATIBLE:"));
    }
}

#[test]
fn integrity_protocol_busy_and_absent_endpoint_keep_existing_error_categories() {
    let mut bad_integrity = modern("compatible", Some(1), 1, true, 1);
    bad_integrity["integrity_check"] = json!("failed");
    assert_eq!(cli(bad_integrity, None).stderr, b"INTEGRITY_FAILED\n");
    for (wire, expected) in [
        ("DB_BUSY", "IPC_ERROR\n"),
        ("PROTOCOL_INCOMPATIBLE", "PROTOCOL_INCOMPATIBLE\n"),
        ("INTEGRITY_FAILED", "INTEGRITY_FAILED\n"),
        ("SCHEMA_DETAILS_UNAVAILABLE", "SCHEMA_DETAILS_UNAVAILABLE\n"),
    ] {
        let output = cli(Value::Null, Some(wire));
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert_eq!(output.stderr, expected.as_bytes());
    }
    let endpoint = Endpoint::new();
    let output = Command::new(env!("CARGO_BIN_EXE_arcctl"))
        .arg("doctor")
        .env(arc_core::ARC_SOCKET_PATH, endpoint.0.join("absent.sock"))
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, b"DAEMON_UNAVAILABLE\n");
}

#[test]
fn doctor_help_explains_schema_scope_and_prestartup_stderr() {
    let output = Command::new(env!("CARGO_BIN_EXE_arcctl"))
        .args(["doctor", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("database integrity and schema compatibility"));
    assert!(text.contains("inspect its stderr"));
    assert!(!text.contains('\u{1b}'));
}

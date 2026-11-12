//! Deterministic protocol fixtures through the real authenticated read bridge.
//! These do not certify production native sampling or calibration.
use crate::{bridge::DesktopBridge, error::ErrorCode};
use arc_ipc::{meter::OPERATION_METER_SNAPSHOT, Request, Response};
use serde_json::{json, Value};
use std::{
    fs,
    os::unix::{fs::PermissionsExt, net::UnixListener},
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

struct SocketFixture {
    root: PathBuf,
    bridge: DesktopBridge,
    listener: UnixListener,
}
impl SocketFixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "arc-desktop-meter-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let socket = root.join("daemon.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        fs::set_permissions(&socket, fs::Permissions::from_mode(0o600)).unwrap();
        Self {
            root,
            bridge: DesktopBridge::with_socket_and_clock(socket, || 1700000000123),
            listener,
        }
    }
    fn serve(&self, responses: Vec<Value>) -> thread::JoinHandle<Vec<Request>> {
        let listener = self.listener.try_clone().unwrap();
        thread::spawn(move || {
            responses
                .into_iter()
                .map(|value| {
                    let (mut stream, _) = listener.accept().unwrap();
                    stream
                        .set_read_timeout(Some(Duration::from_secs(3)))
                        .unwrap();
                    let request: Request = arc_ipc::read_message(&mut stream).unwrap();
                    let response = if let Some(code) = value.as_str() {
                        Response::error(&request.request_id, code)
                    } else {
                        Response::ok(&request.request_id, value)
                    };
                    arc_ipc::write_response(&mut stream, &response).unwrap();
                    request
                })
                .collect()
        })
    }
}
impl Drop for SocketFixture {
    fn drop(&mut self) {
        // Only this exclusive test-created directory is owned by the fixture.
        let _ = fs::remove_file(self.root.join("daemon.sock"));
        let _ = fs::remove_dir(&self.root);
    }
}
fn golden() -> Value {
    serde_json::from_str(include_str!(
        "../../../../crates/arc-core/tests/fixtures/meter-snapshot.json"
    ))
    .unwrap()
}

#[test]
fn meter_core_schema_receipt_and_argument_free_operation_match() {
    let fixture = SocketFixture::new();
    let value = golden();
    let server = fixture.serve(vec![value.clone()]);
    let received = fixture.bridge.meter_snapshot().unwrap();
    assert_eq!(received.received_at_ms, 1700000000123);
    assert_eq!(serde_json::to_value(received.data).unwrap(), value);
    let requests = server.join().unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].operation, OPERATION_METER_SNAPSHOT);
    assert_eq!(requests[0].payload, Value::Null);
}

#[test]
fn meter_unsupported_unavailable_and_busy_do_not_disable_health() {
    let fixture = SocketFixture::new();
    let health = json!({"daemon":"running","protocol_version":1,"sweeper":"off"});
    let server = fixture.serve(vec![
        json!(arc_ipc::ERROR_UNSUPPORTED_CAPABILITY),
        health.clone(),
        json!(arc_ipc::meter::ERROR_METER_UNAVAILABLE),
        health.clone(),
        json!(arc_ipc::ERROR_DB_BUSY),
        health,
    ]);
    for code in [
        ErrorCode::Unsupported,
        ErrorCode::Unavailable,
        ErrorCode::Busy,
    ] {
        assert_eq!(fixture.bridge.meter_snapshot().unwrap_err().code, code);
        assert_eq!(
            fixture
                .bridge
                .daemon_health()
                .unwrap()
                .data
                .protocol_version,
            1
        );
    }
    // Busy must not renew the deadline or retry, consuming the health response.
    assert_eq!(server.join().unwrap().len(), 6);
}

#[test]
fn meter_rejects_bad_projection_without_losing_other_reads() {
    let fixture = SocketFixture::new();
    let mut wrong_unit = golden();
    wrong_unit["host"]["cpu_utilization"]["unit"] = json!("bytes");
    let mut extra = golden();
    extra["host"]["secret"] = json!("private");
    let mut invalid_identity = golden();
    invalid_identity["daemon_instance"]["pid"] = json!(0);
    let server = fixture.serve(vec![wrong_unit, extra, invalid_identity, golden()]);
    for _ in 0..3 {
        assert_eq!(
            fixture.bridge.meter_snapshot().unwrap_err().code,
            ErrorCode::InvalidReply
        );
    }
    assert!(fixture.bridge.meter_snapshot().unwrap().data.is_valid());
    server.join().unwrap();
}

#[test]
fn meter_shares_the_existing_single_permit_and_shutdown() {
    let fixture = SocketFixture::new();
    let listener = fixture.listener.try_clone().unwrap();
    let (accepted, ready) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let request: Request = arc_ipc::read_message(&mut stream).unwrap();
        accepted.send(()).unwrap();
        released.recv_timeout(Duration::from_secs(3)).unwrap();
        let _ = arc_ipc::write_response(&mut stream, &Response::ok(request.request_id, golden()));
    });
    let bridge = fixture.bridge.clone();
    let reader = thread::spawn(move || bridge.meter_snapshot());
    ready.recv_timeout(Duration::from_secs(3)).unwrap();
    assert_eq!(
        fixture.bridge.daemon_health().unwrap_err().code,
        ErrorCode::Busy
    );
    assert_eq!(
        fixture.bridge.meter_snapshot().unwrap_err().code,
        ErrorCode::Busy
    );
    fixture.bridge.shutdown();
    release.send(()).unwrap();
    assert_eq!(
        reader.join().unwrap().unwrap_err().code,
        ErrorCode::Cancelled
    );
    server.join().unwrap();
    assert_eq!(
        fixture.bridge.meter_snapshot().unwrap_err().code,
        ErrorCode::Cancelled
    );
}

#[test]
fn meter_async_uses_the_same_core_contract() {
    let fixture = SocketFixture::new();
    let server = fixture.serve(vec![golden()]);
    let received = tauri::async_runtime::block_on(fixture.bridge.meter_snapshot_async()).unwrap();
    assert!(received.data.is_valid());
    server.join().unwrap();
}

//! Deterministic same-user socket fixtures through the production registration
//! and bridge. These tests establish dispatch/decoding/deadlines, not SQLite or
//! native-workload calibration; the combined acceptance owner checks those.
use super::meter_router::Router;
use arc_desktop::{bridge::DesktopBridge, error::ErrorCode};
use arc_ipc::{meter::OPERATION_METER_SUMMARY, Request, Response};
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
    time::{Duration, Instant},
};

const ID: &str = "11111111111111111111111111111111";
struct Peer {
    root: PathBuf,
    listener: UnixListener,
    bridge: DesktopBridge,
}
impl Peer {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "arc-jm-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
        let path = root.join("d.sock");
        let listener = UnixListener::bind(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        Self {
            root,
            listener,
            bridge: DesktopBridge::from_socket_path(path),
        }
    }
    fn listener(&self) -> UnixListener {
        self.listener.try_clone().unwrap()
    }
}
impl Drop for Peer {
    fn drop(&mut self) {
        let _ = fs::remove_file(self.root.join("d.sock"));
        let _ = fs::remove_dir(&self.root);
    }
}
fn summary() -> Value {
    let report: Value = serde_json::from_str(include_str!(
        "../../../../../crates/arc-core/tests/fixtures/job-meter-report.json"
    ))
    .unwrap();
    json!({"schema_version":1,"job_id":ID,"accounting":{"state":"recorded","report":report},
        "queue_wait":{"seconds":0.0,"clock":"not_applicable","basis":"lifecycle_only"}})
}

#[test]
fn job_meter_registered_route_uses_authenticated_owner_socket_and_shared_report() {
    let peer = Peer::new();
    let listener = peer.listener();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let request: Request = arc_ipc::read_message(&mut stream).unwrap();
        assert_eq!(request.operation, OPERATION_METER_SUMMARY);
        assert_eq!(request.payload, json!({"job_id":ID}));
        assert_eq!(request.protocol_version, 1);
        arc_ipc::write_response(&mut stream, &Response::ok(request.request_id, summary())).unwrap();
    });
    let router = Router::new(peer.bridge.clone());
    let received = router.call("meter_summary", json!({"job_id":ID})).unwrap();
    assert_eq!(received["data"], summary());
    assert!(received["received_at_ms"].is_u64());
    assert!(arc_ipc::meter::decode_summary(&received["data"], ID)
        .unwrap()
        .is_valid());
    server.join().unwrap();
}

#[test]
fn job_meter_deadline_correlation_protocol_and_local_errors_are_bounded() {
    for fault in ["stall", "correlation", "protocol", "private", "unsupported"] {
        let peer = Peer::new();
        let listener = peer.listener();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(4)))
                .unwrap();
            let request: Request = arc_ipc::read_message(&mut stream).unwrap();
            assert_eq!(request.operation, OPERATION_METER_SUMMARY);
            if fault == "stall" {
                use std::io::Read;
                assert_eq!(
                    stream.read(&mut [0]).unwrap(),
                    0,
                    "client deadline closes socket"
                );
                return;
            }
            let mut reply = Response::ok(&request.request_id, summary());
            match fault {
                "correlation" => reply.request_id = "retired-epoch".into(),
                "protocol" => reply.protocol_version = 2,
                "private" => reply.result.as_mut().unwrap()["argv"] = json!("PRIVATE_SENTINEL"),
                "unsupported" => {
                    reply =
                        Response::error(request.request_id, arc_ipc::ERROR_UNSUPPORTED_CAPABILITY)
                }
                _ => unreachable!(),
            }
            arc_ipc::write_response(&mut stream, &reply).unwrap();
        });
        let router = Router::new(peer.bridge.clone());
        let started = Instant::now();
        let error = router
            .call("meter_summary", json!({"job_id":ID}))
            .unwrap_err();
        assert!(started.elapsed() < Duration::from_secs(3));
        assert_eq!(
            error["code"],
            match fault {
                "stall" => "uncertain",
                "protocol" => "protocol",
                "unsupported" => "unsupported",
                _ => "invalid_reply",
            }
        );
        assert!(!error.to_string().contains("PRIVATE_SENTINEL"));
        server.join().unwrap();
    }
}

#[test]
fn job_meter_pending_read_owns_one_permit_and_shutdown_retires_its_receipt() {
    let peer = Peer::new();
    let listener = peer.listener();
    let (accepted, ready) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let request: Request = arc_ipc::read_message(&mut stream).unwrap();
        assert_eq!(request.operation, OPERATION_METER_SUMMARY);
        accepted.send(()).unwrap();
        released.recv_timeout(Duration::from_secs(3)).unwrap();
        let _ = arc_ipc::write_response(&mut stream, &Response::ok(request.request_id, summary()));
    });
    let bridge = peer.bridge.clone();
    let reader = thread::spawn(move || bridge.meter_summary(ID));
    ready.recv_timeout(Duration::from_secs(3)).unwrap();
    assert_eq!(
        peer.bridge.daemon_health().unwrap_err().code,
        ErrorCode::Busy
    );
    assert_eq!(
        peer.bridge.meter_snapshot().unwrap_err().code,
        ErrorCode::Busy
    );
    assert_eq!(
        peer.bridge.meter_summary(ID).unwrap_err().code,
        ErrorCode::Busy
    );
    peer.bridge.shutdown();
    release.send(()).unwrap();
    assert_eq!(
        reader.join().unwrap().unwrap_err().code,
        ErrorCode::Cancelled
    );
    server.join().unwrap();
}

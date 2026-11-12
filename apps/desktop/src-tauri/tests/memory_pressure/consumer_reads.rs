//! Socket fixtures exercise production DesktopBridge, not a production daemon.
use arc_desktop::{bridge::DesktopBridge, error::ErrorCode};
use arc_ipc::{jobs::JobId, policy::*, read_frame, write_frame, Request, Response};
use serde_json::{json, Value};
use std::{
    fs,
    os::unix::{
        fs::DirBuilderExt,
        net::{UnixListener, UnixStream},
    },
    path::PathBuf,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

pub(super) struct Socket {
    root: PathBuf,
    pub(super) listener: UnixListener,
}
impl Socket {
    pub(super) fn new() -> Self {
        let root = PathBuf::from(format!(
            "/tmp/arc-policy-read-{}",
            arc_core::process_identity::random_job_id().unwrap()
        ));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        Self {
            listener: UnixListener::bind(root.join("s")).unwrap(),
            root,
        }
    }
    pub(super) fn bridge(&self) -> DesktopBridge {
        DesktopBridge::with_socket_and_clock(self.root.join("s"), || 99000)
    }
}
impl Drop for Socket {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
pub(super) fn read(stream: &mut UnixStream) -> Request {
    stream
        .set_read_timeout(Some(Duration::from_secs(4)))
        .unwrap();
    Request::decode(&read_frame(stream).unwrap()).unwrap()
}
pub(super) fn reply(stream: &mut UnixStream, request: &Request, value: Value) {
    write_frame(
        stream,
        &serde_json::to_vec(&Response::ok(&request.request_id, value)).unwrap(),
    )
    .unwrap();
}
fn capability(listener: &UnixListener) {
    let (mut stream, _) = listener.accept().unwrap();
    let request = read(&mut stream);
    assert_eq!(request.operation, OPERATION_POLICY_CAPABILITIES);
    assert!(request.payload.is_null());
    reply(&mut stream, &request, json!(Capabilities::supported()));
}
fn job() -> JobPolicyResult {
    JobPolicyResult {
        schema_version: 1,
        job_id: JobId::try_from("a".repeat(32)).unwrap(),
        effective: super::fixture::fallback(),
        decision_availability: DecisionAvailability::NotRecorded,
        last_decision: None,
    }
}

#[test]
fn production_bridge_reads_exact_policy_payload_without_refreshing_historical_age() {
    let socket = Socket::new();
    let bridge = socket.bridge();
    let server = thread::spawn(move || {
        capability(&socket.listener);
        let (mut stream, _) = socket.listener.accept().unwrap();
        let request = read(&mut stream);
        assert_eq!(request.operation, OPERATION_POLICY_CURRENT);
        assert!(request.payload.is_null());
        reply(&mut stream, &request, json!(super::fixture::fallback()));
        capability(&socket.listener);
        let (mut stream, _) = socket.listener.accept().unwrap();
        let request = read(&mut stream);
        assert_eq!(request.operation, OPERATION_POLICY_JOB);
        assert_eq!(request.payload, json!({ "job_id": "a".repeat(32) }));
        let mut result = job();
        result.last_decision = Some(RecordedDecision {
            policy_version: POLICY_VERSION.into(),
            kind: DecisionKind::Wait,
            reason: DecisionReason::ClassWindow,
            decided_at_ms: 1000,
            permit_generation: None,
            observation: super::fixture::fallback().observation,
        });
        result.decision_availability = DecisionAvailability::Available;
        reply(&mut stream, &request, json!(result));
    });
    let current = bridge.policy_current().unwrap();
    assert_eq!(current.received_at_ms, 99000);
    assert_eq!(current.data.reason, PolicyReason::ObservationMissing);
    let result = bridge.policy_job(&"a".repeat(32)).unwrap();
    let decision = result.data.last_decision.unwrap();
    assert_eq!(decision.decided_at_ms, 1000);
    assert_eq!(decision.observation.age_ms, None);
    assert_eq!(decision.reason, DecisionReason::ClassWindow);
    server.join().unwrap();
}

#[test]
fn explicit_unsupported_is_not_unavailable_invalid_request_or_malformed() {
    for (code, expected) in [
        (
            arc_ipc::ERROR_UNSUPPORTED_CAPABILITY,
            ErrorCode::Unsupported,
        ),
        (arc_ipc::ERROR_INVALID_REQUEST, ErrorCode::Rejected),
        (ERROR_POLICY_UNAVAILABLE, ErrorCode::Unavailable),
    ] {
        let socket = Socket::new();
        let bridge = socket.bridge();
        let server = thread::spawn(move || {
            let (mut stream, _) = socket.listener.accept().unwrap();
            let request = read(&mut stream);
            write_frame(
                &mut stream,
                &serde_json::to_vec(&Response::error(&request.request_id, code)).unwrap(),
            )
            .unwrap();
        });
        assert_eq!(bridge.policy_current().unwrap_err().code, expected);
        server.join().unwrap();
    }
    let socket = Socket::new();
    let bridge = socket.bridge();
    let server = thread::spawn(move || {
        let (mut stream, _) = socket.listener.accept().unwrap();
        let request = read(&mut stream);
        reply(
            &mut stream,
            &request,
            json!({ "capability": CAPABILITY, "launch_capability": "old" }),
        );
    });
    assert_eq!(
        bridge.policy_current().unwrap_err().code,
        ErrorCode::InvalidReply
    );
    server.join().unwrap();
}

#[test]
fn strict_job_validation_rejects_wrong_ids_arrays_unknown_fields_and_unsafe_numbers() {
    let mut wrong = json!(job());
    wrong["job_id"] = json!("b".repeat(32));
    let mut extra = json!(job());
    extra["path"] = json!("private");
    let mut unsafe_value = json!(job());
    unsafe_value["effective"]["observation"]["sequence"] = json!(9007199254740992u64);
    for value in [
        wrong,
        extra,
        unsafe_value,
        json!([1, 2]),
        json!({ "secret": "x".repeat(MAX_POLICY_BYTES + 1) }),
    ] {
        let socket = Socket::new();
        let bridge = socket.bridge();
        let server = thread::spawn(move || {
            capability(&socket.listener);
            let (mut stream, _) = socket.listener.accept().unwrap();
            let request = read(&mut stream);
            reply(&mut stream, &request, value);
        });
        assert_eq!(
            bridge.policy_job(&"a".repeat(32)).unwrap_err().code,
            ErrorCode::InvalidReply
        );
        server.join().unwrap();
    }
    let bridge = DesktopBridge::new();
    assert_eq!(
        bridge.policy_job("../private").unwrap_err().code,
        ErrorCode::InvalidRequest
    );
}

#[test]
fn policy_deadline_and_existing_single_flight_permit_bound_pending_reads() {
    let socket = Socket::new();
    let bridge = socket.bridge();
    let (seen_tx, seen_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let server = thread::spawn(move || {
        let (mut stream, _) = socket.listener.accept().unwrap();
        read(&mut stream);
        seen_tx.send(()).unwrap();
        done_rx.recv_timeout(Duration::from_secs(4)).unwrap();
    });
    let reading = bridge.clone();
    let start = Instant::now();
    let reader = thread::spawn(move || reading.policy_current());
    seen_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(
        bridge.policy_job(&"a".repeat(32)).unwrap_err().code,
        ErrorCode::Busy
    );
    assert_eq!(bridge.daemon_health().unwrap_err().code, ErrorCode::Busy);
    assert!(matches!(
        reader.join().unwrap().unwrap_err().code,
        ErrorCode::Uncertain | ErrorCode::Unavailable
    ));
    assert!(start.elapsed() < Duration::from_secs(3));
    done_tx.send(()).unwrap();
    server.join().unwrap();
    bridge.shutdown();
    assert_eq!(
        bridge.policy_current().unwrap_err().code,
        ErrorCode::Cancelled
    );
}

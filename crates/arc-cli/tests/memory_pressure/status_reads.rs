//! Production CLI against deterministic authenticated socket fixtures.
use arc_ipc::{jobs::JobId, policy::*, Request, Response};
use serde_json::json;
use std::{
    fs,
    os::unix::{fs::DirBuilderExt, net::UnixListener},
    path::PathBuf,
    process::Command,
    thread,
    time::Duration,
};

struct Socket {
    root: PathBuf,
    listener: UnixListener,
}
impl Socket {
    fn new() -> Self {
        let root = PathBuf::from(format!(
            "/tmp/arc-policy-cli-{}",
            arc_core::process_identity::random_job_id().unwrap()
        ));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        Self {
            listener: UnixListener::bind(root.join("s")).unwrap(),
            root,
        }
    }
}
impl Drop for Socket {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
fn critical() -> EffectivePolicy {
    let mut policy = super::fixture::fallback();
    policy.mode = PolicyMode::MemoryHold;
    policy.reason = PolicyReason::MemoryCritical;
    policy.observation = ObservationSummary {
        availability: ObservationAvailability::Available,
        pressure: Some(arc_core::meter::MemoryPressure::Critical),
        age_ms: Some(5),
        stale_after_ms: Some(3000),
        sampler: Some(SamplerIdentity {
            daemon_instance: arc_ipc::jobs::ProcessIdentity::capture(std::process::id()).unwrap(),
            generation: 1,
            monotonic_epoch: 1,
        }),
        sequence: Some(1),
        observed_at_ms: Some(1000),
        observed_monotonic_ms: Some(1000),
    };
    assert!(policy.is_valid());
    policy
}
fn delayed_status(
    policy: EffectivePolicy,
    unsupported: bool,
    malformed: bool,
    delay: Duration,
    reserved: bool,
) -> String {
    let socket = Socket::new();
    let path = socket.root.join("s");
    let server = thread::spawn(move || {
        for index in 0..if unsupported { 2 } else { 3 } {
            let (mut stream, _) = socket.listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let request = Request::decode(&arc_ipc::read_frame(&mut stream).unwrap()).unwrap();
            let response = match index {
                0 => {
                    assert_eq!(request.operation, arc_ipc::traffic::OPERATION_TRAFFIC_JOB);
                    Response::ok(
                        &request.request_id,
                        if reserved {
                            json!(arc_ipc::traffic::StatusResult::Unconfirmed {})
                        } else {
                            json!(arc_ipc::traffic::StatusResult::Queued {
                                sequence: 1,
                                queued_at_ms: 1000
                            })
                        },
                    )
                }
                1 => {
                    assert_eq!(request.operation, OPERATION_POLICY_CAPABILITIES);
                    if unsupported {
                        Response::error(&request.request_id, arc_ipc::ERROR_UNSUPPORTED_CAPABILITY)
                    } else {
                        Response::ok(&request.request_id, json!(Capabilities::supported()))
                    }
                }
                _ => {
                    assert_eq!(request.operation, OPERATION_POLICY_JOB);
                    let id = JobId::try_from("a".repeat(32)).unwrap();
                    assert_eq!(request.payload, json!({ "job_id": id }));
                    let old = critical();
                    let result = JobPolicyResult {
                        schema_version: 1,
                        job_id: id,
                        effective: policy.clone(),
                        decision_availability: DecisionAvailability::Available,
                        last_decision: Some(RecordedDecision {
                            policy_version: POLICY_VERSION.into(),
                            kind: DecisionKind::Wait,
                            reason: DecisionReason::MemoryPressure,
                            decided_at_ms: 1005,
                            permit_generation: None,
                            observation: old.observation,
                        }),
                    };
                    let mut value = json!(result);
                    if malformed {
                        value["effective"]["raw_command"] = json!("PRIVATE_SENTINEL");
                    }
                    Response::ok(&request.request_id, value)
                }
            };
            if index == 2 {
                thread::sleep(delay);
            }
            arc_ipc::write_response(&mut stream, &response).unwrap();
        }
    });
    let output = Command::new(env!("CARGO_BIN_EXE_arcctl"))
        .args(["job", "status", &"a".repeat(32)])
        .env(arc_core::ARC_SOCKET_PATH, path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    server.join().unwrap();
    assert!(output.stderr.is_empty());
    String::from_utf8(output.stdout).unwrap()
}

fn status(policy: EffectivePolicy, unsupported: bool, malformed: bool) -> String {
    delayed_status(policy, unsupported, malformed, Duration::ZERO, false)
}
#[test]
fn critical_expired_in_transit_is_last_received_and_cannot_claim_a_current_hold() {
    let mut policy = critical();
    policy.observation.age_ms = Some(2990);
    let text = delayed_status(policy, false, false, Duration::from_millis(100), false);
    assert!(text.contains("Last received policy (stale; current admission unconfirmed)"));
    assert!(!text.contains("Wait reason: Waiting for memory"));
    assert!(!text.contains("Initial policy active (L0)"));
    assert!(text.contains("Recorded observation age (ms): 5"));
}
#[test]
fn reserved_unconfirmed_lifecycle_preserves_authoritative_pressure_wait_without_claiming_running() {
    let text = delayed_status(critical(), false, false, Duration::ZERO, true);
    assert!(text.contains("State: outcome unknown\nReservation: retained"));
    assert!(text.contains("Wait reason: Waiting for memory"));
    assert!(!text.contains("State: running"));
    let text = delayed_status(
        super::fixture::fallback(),
        false,
        false,
        Duration::ZERO,
        true,
    );
    assert!(text.contains("State: outcome unknown\nReservation: retained"));
    assert!(!text.contains("Wait reason: Waiting for memory"));
}
#[test]
fn cli_status_distinguishes_current_pressure_from_last_recorded_decision_and_l0() {
    let text = status(critical(), false, false);
    assert!(text.contains("State: queued"));
    assert!(text.contains("Wait reason: Waiting for memory"));
    assert!(text.contains("Starts automatically when memory permits"));
    let mut stale = critical();
    stale.mode = PolicyMode::L0;
    stale.reason = PolicyReason::ObservationStale;
    stale.observation.availability = ObservationAvailability::Stale;
    stale.observation.age_ms = Some(3001);
    let text = status(stale, false, false);
    assert!(text.contains("Initial policy active (L0)"));
    assert!(text.contains("observation_stale"));
    assert!(!text.contains("Wait reason: Waiting for memory"));
    assert!(text.contains("Last recorded job decision: memory_pressure"));
    assert!(text.contains("Recorded observation age (ms): 5"));
}
#[test]
fn cli_old_daemon_and_invalid_policy_remain_distinct_without_leaking_peer_text() {
    assert!(status(super::fixture::fallback(), true, false).contains("unsupported by this daemon"));
    let text = status(super::fixture::fallback(), false, true);
    assert!(text.contains("current policy and job decision are unknown"));
    assert!(!text.contains("PRIVATE_SENTINEL"));
}

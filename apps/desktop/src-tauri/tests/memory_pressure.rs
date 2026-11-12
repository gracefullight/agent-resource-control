//! Task-1 desktop read foundation; task 3 adds production DesktopBridge cases.
#[path = "memory_pressure/consumer_reads.rs"]
mod consumer_reads;
#[path = "../../../../crates/arc-ipc/tests/fixtures/memory_pressure.rs"]
mod fixture;
#[path = "memory_pressure/registered_policy.rs"]
mod registered_policy;

#[test]
fn current_policy_and_last_job_decision_are_separate_bounded_read_fields() {
    use arc_ipc::{jobs::JobId, policy::*};
    let id = JobId::try_from("a".repeat(32)).unwrap();
    let mut effective = fixture::fallback();
    effective.reason = PolicyReason::ObservationFailed;
    effective.observation.availability = ObservationAvailability::Failed;
    let old = RecordedDecision {
        policy_version: POLICY_VERSION.into(),
        kind: DecisionKind::Wait,
        reason: DecisionReason::ClassWindow,
        decided_at_ms: 100,
        permit_generation: None,
        observation: fixture::fallback().observation,
    };
    let result = JobPolicyResult {
        schema_version: 1,
        job_id: id.clone(),
        effective,
        decision_availability: DecisionAvailability::Available,
        last_decision: Some(old),
    };
    let request = job_request("selected", &id);
    let decoded = interpret_job(&request, &job_response(&request, &result)).unwrap();
    assert_eq!(decoded.effective.reason, PolicyReason::ObservationFailed);
    assert_eq!(
        decoded.last_decision.unwrap().reason,
        DecisionReason::ClassWindow
    );
    assert!(serde_json::to_vec(&result).unwrap().len() < MAX_POLICY_BYTES);
}

#[allow(dead_code)]
#[path = "../../../../crates/arc-daemon/tests/support/native_exec.rs"]
mod execution;
#[allow(dead_code)]
#[path = "../../../../crates/arc-daemon/tests/support/native_fixture.rs"]
mod native;
#[cfg(target_os = "macos")]
#[path = "memory_pressure/native_launch_races.rs"]
mod native_launch_races;
#[cfg(target_os = "macos")]
#[path = "memory_pressure/native_reserved.rs"]
mod native_reserved;
#[cfg(target_os = "macos")]
#[path = "memory_pressure/native_sequence.rs"]
mod native_sequence;
#[cfg(target_os = "macos")]
#[path = "memory_pressure/native_support.rs"]
mod native_support;
#[cfg(target_os = "macos")]
#[path = "daemon_bridge_support/meter_router.rs"]
mod router;
#[allow(dead_code)]
#[path = "../../../../crates/arc-daemon/tests/support/mod.rs"]
mod serialization;

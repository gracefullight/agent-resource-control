//! Task-1 launch wire foundation. Task 3 adds CLI coordination/acceptance cases.
#[path = "../../arc-ipc/tests/fixtures/memory_pressure.rs"]
mod fixture;
#[path = "memory_pressure/status_reads.rs"]
mod status_reads;

#[test]
fn ready_is_generation_bound_and_acknowledgements_cannot_name_another_launch() {
    use arc_ipc::{jobs::*, policy::launch::*};
    let request = LaunchRequest {
        job_id: JobId::try_from("a".repeat(32)).unwrap(),
        observer: ProcessIdentity {
            pid: 123,
            start_token: "456".into(),
            boot_id: "00000000-0000-0000-0000-000000000001".into(),
        },
        permit_generation: 1,
    };
    let value = serde_json::json!(request);
    assert_eq!(decode_launch_request(&value), Ok(request.clone()));
    let ready = LaunchResult::Ready {
        permit_generation: 1,
        policy: fixture::fallback(),
    };
    assert!(ready.is_valid());
    let mut ack = LaunchAck::Unspawned {
        job_id: request.job_id.clone(),
        observer: request.observer.clone(),
        permit_generation: 1,
    };
    assert!(ack.matches(&request));
    if let LaunchAck::Unspawned {
        permit_generation, ..
    } = &mut ack
    {
        *permit_generation = 2;
    }
    assert!(!ack.matches(&request));
    assert!(decode_launch_ack(&serde_json::json!(ack), &request).is_err());
}

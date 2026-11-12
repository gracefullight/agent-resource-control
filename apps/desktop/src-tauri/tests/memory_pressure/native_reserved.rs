use super::{execution, native_support::*, serialization};
use arc_core::process_identity::ProcessIdentity;
use arc_ipc::{jobs::JobId, policy::launch::*, traffic::JobRequest, Request};
use serde_json::json;
use std::fs;

fn enqueue(case: &Case, id: &JobId, observer: &ProcessIdentity) -> u64 {
    let reply = case.rpc(
        "traffic.job",
        json!(JobRequest::Enqueue {
            job_id: id.clone(),
            observer: observer.clone(),
            lineage_context: None,
            classification: serde_json::from_value(
                json!({"classification":"TEST","locality":"HOST_LOCAL",
            "reason":"CARGO_TEST","uncertainty":"NONE"})
            )
            .unwrap()
        }),
    );
    assert_eq!(reply["state"], "GRANTED");
    reply["permit_generation"].as_u64().unwrap()
}
fn launch_request(input: &LaunchRequest) -> Request {
    let mut request = Request::new("reserved-launch", OPERATION_TRAFFIC_LAUNCH);
    request.payload = json!(input);
    request
}
#[test]
fn real_dispatcher_confirms_unspawned_cleanup_and_retains_uncertain_releases() {
    let _serial = serialization::serial();
    let mut case = Case::new(true);
    case.pressure("Normal", 0);
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let id = JobId::try_from(arc_core::process_identity::random_job_id().unwrap()).unwrap();
    let generation = enqueue(&case, &id, &observer);
    let input = LaunchRequest {
        job_id: id.clone(),
        observer: observer.clone(),
        permit_generation: generation,
    };
    let mut connection = case.fixture.connect();
    let ready = arc_ipc::exchange(&mut connection, &launch_request(&input)).unwrap();
    assert!(matches!(
        decode_launch_result(ready.result.as_ref().unwrap()).unwrap(),
        LaunchResult::Ready { .. }
    ));
    let mut ack = Request::new("unspawned", OPERATION_TRAFFIC_LAUNCH_ACK);
    ack.payload = json!(LaunchAck::Unspawned {
        job_id: id.clone(),
        observer: observer.clone(),
        permit_generation: generation
    });
    assert_eq!(
        arc_ipc::exchange(&mut connection, &ack).unwrap().result,
        Some(json!({}))
    );
    assert!(case
        .rows("SELECT id FROM audit_events WHERE event_type='TRAFFIC_LAUNCH_FENCE'")
        .is_empty());
    let mut uncertain = case.fixture.connect();
    let retried = arc_ipc::exchange(&mut uncertain, &launch_request(&input)).unwrap();
    assert!(matches!(
        decode_launch_result(retried.result.as_ref().unwrap()).unwrap(),
        LaunchResult::Ready { .. }
    ));
    // A lost READY without a negative ACK remains non-replayable.
    drop(uncertain);
    let retry = arc_ipc::exchange(&mut case.fixture.connect(), &launch_request(&input)).unwrap();
    assert_eq!(retry.status, "error");
    assert_eq!(
        case.rows("SELECT id FROM audit_events WHERE event_type='TRAFFIC_LAUNCH_FENCE'")
            .len(),
        1
    );
    case.rpc(
        "traffic.job",
        json!(JobRequest::Cancel {
            job_id: id,
            observer,
            permit_generation: Some(generation)
        }),
    );
    save(
        "unspawned-ack",
        &json!({"ready":ready,"ackObject":true,"knownUnspawnedReevaluated":true,"targetExecutions":0,"uncertainRelease":retried,"retry":retry}),
    );
    case.stop();
}

#[test]
fn bounded_daemon_outage_fail_open_executes_the_owned_target_once() {
    let _serial = serialization::serial();
    let mut case = Case::new(true);
    case.stop();
    let started = std::time::Instant::now();
    let mut run = Run::start(&case, "outage", "test");
    let child = run.started();
    assert!(started.elapsed() < std::time::Duration::from_secs(10));
    assert_eq!(run.finish().code(), Some(17));
    assert_eq!(run.count(), 1);
    let error = fs::read_to_string(&run.stderr).unwrap();
    assert!(
        error.contains("unavailable") || error.contains("unconfirmed"),
        "{error}"
    );
    save(
        "bounded-outage",
        &json!({"executions":run.count(),"child":child,"exitCode":17,
        "elapsedMs":started.elapsed().as_millis(),"diagnostic":error,"healthyCriticalHold":false}),
    );
}
#[test]
fn real_queued_cancellation_under_critical_never_starts_or_receives_a_permit() {
    let _serial = serialization::serial();
    let mut case = Case::new(true);
    let critical = case.pressure("Critical", 0);
    let mut run = Run::start(&case, "cancelled", "test");
    let id = case.job(run.wrapper.child.id());
    eventually(|| {
        case.rpc("policy.job", json!({"job_id":id}))["last_decision"]["reason"] == "memory_pressure"
    });
    assert_eq!(run.count(), 0);
    assert_eq!(
        unsafe { libc::kill(run.wrapper.child.id() as i32, libc::SIGINT) },
        0
    );
    let status = execution::wait(&mut run.wrapper.child);
    // Existing CLI preparation cancellation returns FAILURE, independently
    // of native-child signal outcomes. Keep that established contract.
    assert_eq!(status.code(), Some(1));
    let diagnostic = fs::read_to_string(&run.stderr).unwrap();
    assert!(
        diagnostic.contains("NOT_STARTED\nExecution cancelled. No child was started."),
        "{diagnostic}"
    );
    assert!(!diagnostic.contains("QUEUE_CLEANUP_UNCONFIRMED"));
    assert_eq!(run.count(), 0);
    let entries = case.rows("SELECT job_id,state FROM traffic_entries");
    assert_eq!(entries[0]["state"], "CANCELLED");
    assert!(case.rows("SELECT job_id FROM permits").is_empty());
    let result = case
        .router
        .call("policy_job", json!({"job_id":id}))
        .unwrap();
    save(
        "queued-cancellation",
        &json!({"critical":critical,"exitCode":1,"diagnostic":diagnostic,"executions":0,"newPermits":0,
        "entries":entries,"registeredBridge":result}),
    );
    case.stop();
}

#[test]
fn legacy_protocol_one_enqueue_poll_and_status_keep_their_original_shapes_under_critical() {
    let _serial = serialization::serial();
    let mut case = Case::new(true);
    case.pressure("Critical", 0);
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let id = JobId::try_from(arc_core::process_identity::random_job_id().unwrap()).unwrap();
    let enqueue = JobRequest::Enqueue {job_id:id.clone(),observer:observer.clone(),lineage_context:None,
        classification:serde_json::from_value(json!({"classification":"TEST","locality":"HOST_LOCAL","reason":"CARGO_TEST","uncertainty":"NONE"})).unwrap()};
    let first = case.rpc("traffic.job", json!(enqueue));
    let typed: arc_ipc::traffic::JobResult = serde_json::from_value(first.clone()).unwrap();
    assert!(matches!(
        typed,
        arc_ipc::traffic::JobResult::Queued {
            reason: arc_ipc::traffic::WaitReason::MemoryBudget,
            ..
        }
    ));
    assert_eq!(first.as_object().unwrap().len(), 4);
    let poll = case.rpc(
        "traffic.job",
        json!(JobRequest::Poll {
            job_id: id.clone(),
            observer: observer.clone()
        }),
    );
    assert!(matches!(
        serde_json::from_value::<arc_ipc::traffic::JobResult>(poll.clone()).unwrap(),
        arc_ipc::traffic::JobResult::Queued {
            reason: arc_ipc::traffic::WaitReason::MemoryBudget,
            ..
        }
    ));
    let status = case.rpc(
        "traffic.job",
        json!(JobRequest::Status { job_id: id.clone() }),
    );
    assert!(matches!(
        serde_json::from_value::<arc_ipc::traffic::StatusResult>(status.clone()).unwrap(),
        arc_ipc::traffic::StatusResult::Queued { .. }
    ));
    assert_eq!(status.as_object().unwrap().len(), 3);
    assert!(status.get("reason").is_none());
    assert!(case.rows("SELECT job_id FROM permits").is_empty());
    case.rpc(
        "traffic.job",
        json!(JobRequest::Cancel {
            job_id: id.clone(),
            observer,
            permit_generation: None
        }),
    );
    save(
        "legacy-protocol",
        &json!({"jobId":id,"enqueue":first,"poll":poll,"status":status,
        "newPermits":0,"protocolVersion":1,"pressureWaitMapsToLegacyMemoryBudget":true}),
    );
    case.stop();
}

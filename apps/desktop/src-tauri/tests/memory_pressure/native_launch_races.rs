//! Real arcctl barriers after reservation and after READY, before target spawn.
use super::{
    native_sequence::{persisted_child, reconcile},
    native_support::*,
    serialization,
};
use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    os::unix::net::{UnixListener, UnixStream},
    thread,
    time::{Duration, Instant},
};

fn arrive(listener: &UnixListener) -> (UnixStream, Value) {
    listener.set_nonblocking(true).unwrap();
    let mut accepted = None;
    eventually(|| match listener.accept() {
        Ok((stream, _)) => {
            accepted = Some(stream);
            true
        }
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => false,
        Err(error) => panic!("CLI checkpoint accept: {error}"),
    });
    let stream = accepted.unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut line = String::new();
    BufReader::new(&stream).read_line(&mut line).unwrap();
    (stream, serde_json::from_str(&line).unwrap())
}

fn reservation(case: &Case, id: &str, generation: u64) -> Value {
    let permits = case.rows("SELECT job_id,generation,state FROM permits");
    assert_eq!(
        permits,
        vec![json!({"job_id":id,"generation":generation,"state":"ACTIVE"})]
    );
    let entries = case.rows("SELECT job_id,generation,state FROM traffic_entries");
    assert_eq!(
        entries,
        vec![json!({"job_id":id,"generation":generation,"state":"RESERVED"})]
    );
    let jobs = case.rows("SELECT l.job_id,l.observer_pid,j.state AS phase,l.child_pid,l.outcome_kind FROM job_lifecycle l JOIN jobs j ON j.id=l.job_id");
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0]["job_id"], id);
    assert_eq!(jobs[0]["phase"], "LAUNCHING");
    assert!(jobs[0]["child_pid"].is_null());
    assert!(jobs[0]["outcome_kind"].is_null());
    json!({"permits":permits,"entries":entries,"jobs":jobs})
}

#[test]
fn reserved_before_critical_is_held_without_target_execution_then_runs_once_after_recovery() {
    let _serial = serialization::serial();
    let mut case = Case::new(true);
    let normal = case.pressure("Normal", 0);
    let socket = case.fixture.root.join("reserved-barrier.sock");
    let barrier = UnixListener::bind(&socket).unwrap();
    let mut run = Run::start_with_barrier(&case, "reserved", "test", Some(("reserved", &socket)));
    let (mut release, request) = arrive(&barrier);
    let id = request["job_id"].as_str().unwrap();
    let generation = request["permit_generation"].as_u64().unwrap();
    assert_eq!(request["observer"]["pid"], run.wrapper.child.id());
    assert_eq!(case.job(run.wrapper.child.id()), id);
    let before = reservation(&case, id, generation);
    assert_eq!(run.count(), 0);
    assert!(case
        .rows("SELECT id FROM audit_events WHERE event_type='TRAFFIC_LAUNCH_FENCE'")
        .is_empty());
    let critical = case.pressure(
        "Critical",
        normal["observation"]["sequence"].as_u64().unwrap(),
    );
    assert_eq!(
        critical["observation"]["sampler"],
        normal["observation"]["sampler"]
    );
    // The actual CLI now resumes its final launch handshake under fresh Critical.
    release.write_all(b"x").unwrap();
    eventually(|| {
        case.rpc("policy.job", json!({"job_id":id}))["last_decision"]["reason"] == "memory_pressure"
    });
    let held_surface = reconcile(&case, id, "memory_hold", "memory_pressure");
    let mut held = vec![];
    for _ in 0..20 {
        let policy = case.current();
        assert_eq!(policy["mode"], "memory_hold");
        assert_eq!(policy["observation"]["availability"], "available");
        assert_eq!(
            policy["observation"]["sampler"],
            critical["observation"]["sampler"]
        );
        assert!(
            policy["observation"]["age_ms"].as_u64().unwrap()
                <= policy["observation"]["stale_after_ms"].as_u64().unwrap()
        );
        assert!(run.wrapper.child.try_wait().unwrap().is_none());
        assert_eq!(run.count(), 0);
        let stored = reservation(&case, id, generation);
        assert_eq!(
            stored, before,
            "Critical must preserve the existing reservation"
        );
        assert!(case
            .rows("SELECT id FROM audit_events WHERE event_type='TRAFFIC_LAUNCH_FENCE'")
            .is_empty());
        held.push(json!({"policy":policy,"reservation":stored,"targetExecutions":run.count()}));
        thread::sleep(Duration::from_millis(100));
    }
    assert!(
        held.last().unwrap()["policy"]["observation"]["sequence"]
            .as_u64()
            .unwrap()
            > critical["observation"]["sequence"].as_u64().unwrap()
    );
    let recovered = case.pressure(
        "Normal",
        held.last().unwrap()["policy"]["observation"]["sequence"]
            .as_u64()
            .unwrap(),
    );
    let child = run.started();
    assert_eq!(run.finish().code(), Some(17));
    let recovery = reconcile(&case, id, "l0", "admitted");
    let persisted = persisted_child(&case, id, &child, &run);
    let permits = case.rows("SELECT job_id,generation,state FROM permits");
    assert_eq!(
        permits,
        vec![json!({"job_id":id,"generation":generation,"state":"RELEASED"})]
    );
    let result = case.router.call("job_by_id", json!({"job_id":id})).unwrap();
    assert_eq!(
        result["data"]["job"]["outcome"],
        json!({"kind":"EXITED","code":17})
    );
    save(
        "reserved-race",
        &json!({"jobId":id,"reservedGeneration":generation,
        "actualCli":"arcctl","barrier":"after reservation, before final launch request",
        "checkpoint":request,"normal":normal,"reservationBeforeCritical":before,"critical":critical,
        "held":held,"heldSurface":held_surface,"targetExecutionsDuringCritical":0,
        "permitsDuringCritical":1,"newPermitsDuringCritical":0,"reservationUnchanged":true,
        "recovered":recovered,"recoveryExecutions":run.count(),"handshake":run.handshake,
        "sqliteChild":persisted,"permitsAfterRecovery":permits,"recoverySurface":recovery,
        "outcome":result,"reconciledSqliteCliDesktop":true}),
    );
    case.stop();
}

#[test]
fn expired_ready_real_cli_reevaluates_with_real_dispatcher_and_executes_exactly_once() {
    let _serial = serialization::serial();
    let mut case = Case::new(true);
    let normal = case.pressure("Normal", 0);
    let socket = case.fixture.root.join("expiry-barrier.sock");
    let barrier = UnixListener::bind(&socket).unwrap();
    let mut run = Run::start_with_barrier(&case, "expiry", "test", Some(("ready", &socket)));
    let (mut release, ready) = arrive(&barrier);
    let request = &ready["request"];
    let id = request["job_id"].as_str().unwrap();
    let generation = request["permit_generation"].as_u64().unwrap();
    assert_eq!(request["observer"]["pid"], run.wrapper.child.id());
    assert_eq!(ready["policy"]["observation"]["availability"], "available");
    let reserved = reservation(&case, id, generation);
    let first = case.rows(
        "SELECT id,entity_id,created_at FROM audit_events WHERE event_type='TRAFFIC_LAUNCH_FENCE'",
    );
    assert_eq!(first.len(), 1);
    assert_eq!(first[0]["entity_id"], id);
    // READY is received before this barrier. Advancing beyond its maximum
    // lifetime deterministically expires it without ever installing a command.
    let end = Instant::now() + Duration::from_millis(2100);
    while Instant::now() < end {
        assert_eq!(run.count(), 0);
        assert!(run.wrapper.child.try_wait().unwrap().is_none());
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(reservation(&case, id, generation), reserved);
    release.write_all(b"x").unwrap();
    let child = run.started();
    assert_eq!(run.finish().code(), Some(17));
    let surface = reconcile(&case, id, "l0", "admitted");
    let persisted = persisted_child(&case, id, &child, &run);
    let final_marker = case.rows(
        "SELECT id,entity_id,created_at FROM audit_events WHERE event_type='TRAFFIC_LAUNCH_FENCE'",
    );
    assert_eq!(final_marker.len(), 1);
    assert_eq!(final_marker[0]["id"], first[0]["id"]);
    assert!(
        final_marker[0]["created_at"].as_u64().unwrap() > first[0]["created_at"].as_u64().unwrap()
    );
    let permits = case.rows("SELECT job_id,generation,state FROM permits");
    assert_eq!(
        permits,
        vec![json!({"job_id":id,"generation":generation,"state":"RELEASED"})]
    );
    let result = case.router.call("job_by_id", json!({"job_id":id})).unwrap();
    assert_eq!(
        result["data"]["job"]["outcome"],
        json!({"kind":"EXITED","code":17})
    );
    let diagnostic = fs::read_to_string(&run.stderr).unwrap();
    assert!(!diagnostic.contains("UNCONFIRMED"), "{diagnostic}");
    save(
        "expired-ready",
        &json!({"actualCli":"arcctl","normal":normal,
        "checkpoint":ready,"reservationBeforeExpiry":reserved,"firstMarker":first,
        "finalMarker":final_marker,"sameJobAndGeneration":true,"unspawnedExpiryReevaluated":true,
        "executionsBeforeRecovery":0,"targetExecutions":run.count(),"handshake":run.handshake,
        "sqliteChild":persisted,"permits":permits,"surface":surface,"result":result,
        "reconciledSqliteCliDesktop":true,"diagnostic":diagnostic}),
    );
    case.stop();
}

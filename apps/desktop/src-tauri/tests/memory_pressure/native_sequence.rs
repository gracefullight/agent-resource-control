use super::{native_support::*, serialization};
use serde_json::{json, Value};
use std::{fs, thread, time::Duration};

pub(super) fn reconcile(case: &Case, id: &str, mode: &str, reason: &str) -> Value {
    let desktop = case
        .router
        .call("policy_job", json!({"job_id":id}))
        .unwrap();
    let policy = &desktop["data"];
    assert_eq!(policy["job_id"], id);
    assert_eq!(policy["effective"]["mode"], mode);
    let sql = case.rows("SELECT job_id,policy_version,decision,reason_code,recorded_at_ms,observation_json,permit_generation FROM policy_decisions WHERE job_id IS NOT NULL ORDER BY rowid");
    let row = sql.iter().rev().find(|row| row["job_id"] == id).unwrap();
    let persisted: Value = serde_json::from_str(row["observation_json"].as_str().unwrap()).unwrap();
    let recorded = &policy["last_decision"];
    assert_eq!(recorded["reason"], reason);
    assert_eq!(row["reason_code"], reason);
    assert_eq!(row["decision"], recorded["kind"]);
    assert_eq!(row["recorded_at_ms"], recorded["decided_at_ms"]);
    assert_eq!(row["policy_version"], policy["effective"]["policy_version"]);
    assert_eq!(row["permit_generation"], recorded["permit_generation"]);
    assert_eq!(persisted, recorded["observation"]);
    assert_eq!(
        policy["effective"]["observation"]["sampler"],
        persisted["sampler"]
    );
    let cli = case.cli(&["job", "status", id]);
    assert!(cli.status.success(), "{cli:?}");
    let status = String::from_utf8(cli.stdout).unwrap();
    assert!(status.contains(policy["effective"]["policy_version"].as_str().unwrap()));
    assert!(status.contains(&format!("Job: {id}")));
    assert!(status.contains(&format!("Last recorded job decision: {reason}")));
    assert!(status.contains(&format!(
        "Observation sampler: {}",
        policy["effective"]["observation"]["sampler"]
    )));
    assert!(status.contains("Observation sequence: "));
    if mode == "memory_hold" {
        assert!(status.contains("Wait reason: Waiting for memory"));
    } else {
        assert!(!status.contains("Wait reason: Waiting for memory"));
    }
    let meter = case.cli(&["meter", "snapshot", "--json"]);
    assert!(meter.status.success(), "{meter:?}");
    let meter: Value = serde_json::from_slice(&meter.stdout).unwrap();
    let observation = &policy["effective"]["observation"];
    assert_eq!(
        meter["daemon_instance"],
        observation["sampler"]["daemon_instance"]
    );
    assert_eq!(
        meter["host"]["memory_pressure"]["reading"]["value"],
        observation["pressure"]
    );
    assert!(meter["sequence"].as_u64().unwrap() >= observation["sequence"].as_u64().unwrap());
    json!({"jobId":id,"sqlite":row,"desktop":desktop,"cliStatus":status,"cliMeterJson":meter})
}
pub(super) fn persisted_child(
    case: &Case,
    id: &str,
    identity: &arc_core::process_identity::ProcessIdentity,
    run: &Run,
) -> Value {
    let row = case
        .rows("SELECT job_id,child_pid,child_start_token,child_boot_id FROM job_lifecycle")
        .into_iter()
        .find(|row| row["job_id"] == id)
        .unwrap();
    assert_eq!(row["child_pid"], identity.pid);
    assert_eq!(row["child_start_token"], identity.start_token);
    assert_eq!(row["child_boot_id"], identity.boot_id);
    let starts: Vec<u32> = fs::read_to_string(&run.marker)
        .unwrap()
        .lines()
        .map(|line| line.parse().unwrap())
        .collect();
    assert_eq!(starts, vec![identity.pid]);
    row
}
fn held(case: &Case, run: &mut Run, id: &str, permits: usize) -> Value {
    assert!(run.wrapper.child.try_wait().unwrap().is_none());
    assert_eq!(run.count(), 0);
    let current = case.current();
    assert_eq!(current["mode"], "memory_hold");
    assert_eq!(current["observation"]["availability"], "available");
    assert!(
        current["observation"]["age_ms"].as_u64().unwrap()
            <= current["observation"]["stale_after_ms"].as_u64().unwrap()
    );
    let sql = case.rows("SELECT job_id,generation,state FROM permits");
    assert_eq!(sql.len(), permits);
    assert!(!sql.iter().any(|row| row["job_id"] == id));
    let entry = case
        .rows("SELECT job_id,state FROM traffic_entries")
        .into_iter()
        .find(|row| row["job_id"] == id)
        .unwrap();
    assert_eq!(entry["state"], "QUEUED");
    json!({"observation":current["observation"],"permits":sql,"entry":entry,"targetExecutions":run.count()})
}
#[test]
fn production_normal_critical_normal_reconciles_real_execution_sqlite_cli_and_registered_bridge() {
    let _serial = serialization::serial();
    let mut case = Case::new(true);
    let initial = case.pressure("Normal", 0);
    let mut running = Run::start(&case, "running", "test");
    let running_child = running.started();
    let running_id = case.job(running.wrapper.child.id());
    eventually(|| {
        case.rows("SELECT state FROM permits")
            .iter()
            .any(|row| row["state"] == "ACTIVE")
    });
    let permits_before = case.rows("SELECT job_id FROM permits").len();
    assert_eq!(permits_before, 1);
    let normal = reconcile(&case, &running_id, "l0", "admitted");
    let running_sqlite_child = persisted_child(&case, &running_id, &running_child, &running);
    let critical = case.pressure(
        "Critical",
        initial["observation"]["sequence"].as_u64().unwrap(),
    );
    assert_eq!(
        initial["observation"]["sampler"],
        critical["observation"]["sampler"]
    );
    let mut waiting = Run::start(&case, "held", "fmt");
    let held_id = case.job(waiting.wrapper.child.id());
    eventually(|| {
        case.rpc("policy.job", json!({"job_id":held_id}))["last_decision"]["reason"]
            == "memory_pressure"
    });
    // Test and Lint have different free windows and together fit L0's CPU and memory budget.
    let held_surface = reconcile(&case, &held_id, "memory_hold", "memory_pressure");
    let first = held(&case, &mut waiting, &held_id, permits_before);
    assert_eq!(running.finish().code(), Some(17));
    let running_surface = case
        .router
        .call("job_by_id", json!({"job_id":running_id}))
        .unwrap();
    assert_eq!(
        running_surface["data"]["job"]["outcome"],
        json!({"kind":"EXITED","code":17})
    );
    assert_eq!(running.count(), 1);
    // After the existing child exits, even its released slot cannot admit the held root.
    let mut interval = vec![first];
    let before = case.rows("SELECT id FROM policy_decisions").len();
    for _ in 0..20 {
        interval.push(held(&case, &mut waiting, &held_id, permits_before));
        thread::sleep(Duration::from_millis(100));
    }
    let after = case.rows("SELECT id FROM policy_decisions").len();
    assert_eq!(
        before, after,
        "unchanged fresh polls must not append audit decisions"
    );
    assert!(
        interval.last().unwrap()["observation"]["sequence"]
            .as_u64()
            .unwrap()
            > critical["observation"]["sequence"].as_u64().unwrap(),
        "Critical must be republished during the held interval"
    );
    let last = case.current();
    assert_eq!(last["mode"], "memory_hold");
    case.publish_pressure("Normal");
    let held_child = waiting.started();
    // The child handshake alone can precede its launch ACK. Wait for durable
    // attachment before a policy read, which correctly rejects an active lease.
    eventually(|| {
        case.rows("SELECT child_pid FROM job_lifecycle")
            .iter()
            .any(|row| row["child_pid"] == held_child.pid)
    });
    let recovered = case.pressure("Normal", last["observation"]["sequence"].as_u64().unwrap());
    assert_eq!(recovered["observation"]["pressure"], "normal");
    assert!(
        recovered["observation"]["sequence"].as_u64().unwrap()
            > last["observation"]["sequence"].as_u64().unwrap()
    );
    assert_ne!(running_child, held_child);
    assert_eq!(waiting.finish().code(), Some(17));
    let recovery_surface = reconcile(&case, &held_id, "l0", "admitted");
    let held_sqlite_child = persisted_child(&case, &held_id, &held_child, &waiting);
    let permits = case.rows("SELECT job_id,generation,state FROM permits");
    assert_eq!(permits.len(), 2);
    assert_eq!(
        permits
            .iter()
            .filter(|row| row["job_id"] == held_id)
            .count(),
        1
    );
    let recovered_job = case
        .router
        .call("job_by_id", json!({"job_id":held_id}))
        .unwrap();
    assert_eq!(
        recovered_job["data"]["job"]["outcome"],
        json!({"kind":"EXITED","code":17})
    );
    save(
        "injected",
        &json!({"sequence":["Normal","Critical","Normal"],"normal":normal,"criticalPolicy":critical,
        "critical":{"newRootPermits":0,"newRootChildStarts":0,"childFinishedWhileHeld":true,"samples":interval},
        "runningChildOutcomeUnchanged":true,"runningChild":running_child,"runningResult":running_surface,
        "held":held_surface,"recovery":{"heldJobExecutions":waiting.count(),"child":held_child,
        "policy":recovered,"surface":recovery_surface,"result":recovered_job},
        "reconciledSqliteCliDesktop":true,"audit":{"before":before,"after":after},
        "sqliteChildren":[running_sqlite_child,held_sqlite_child],"childHandshakes":[running.handshake,waiting.handshake],"permits":permits,
        "cliOutput":[fs::read_to_string(&running.stdout).unwrap(),fs::read_to_string(&waiting.stdout).unwrap()]}),
    );
    case.stop();
}
#[test]
fn system_source_smoke_records_actual_observations_separately_from_injection() {
    let _serial = serialization::serial();
    let mut case = Case::new(false);
    let mut samples = vec![];
    let mut previous = 0;
    for _ in 0..2 {
        let mut sample = None;
        eventually(|| {
            let current = case.current();
            if current["observation"]["sequence"]
                .as_u64()
                .is_some_and(|n| n > previous)
            {
                sample = Some(current);
                true
            } else {
                false
            }
        });
        let sample = sample.unwrap();
        previous = sample["observation"]["sequence"].as_u64().unwrap();
        let routed = case.router.call("policy_current", json!({})).unwrap();
        assert_eq!(
            routed["data"]["observation"]["sampler"],
            sample["observation"]["sampler"]
        );
        let cli = case.cli(&["meter", "snapshot", "--json"]);
        assert!(cli.status.success());
        let native: Value = serde_json::from_slice(&cli.stdout).unwrap();
        assert_eq!(
            native["daemon_instance"],
            sample["observation"]["sampler"]["daemon_instance"]
        );
        assert_eq!(
            native["host"]["memory_pressure"]["reading"]["value"],
            sample["observation"]["pressure"]
        );
        samples.push(json!({"policy":sample,"cliMeter":native,"registeredBridge":routed}));
    }
    save(
        "native-smoke",
        &json!({"executed":true,"source":"SystemSource","separateFromInjection":true,
        "actualObservations":samples,"nativeCriticalForced":false}),
    );
    case.stop();
}

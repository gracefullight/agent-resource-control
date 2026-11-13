use super::{native_support::*, serialization, support::*};
use serde_json::json;
use std::{io::Write, os::unix::net::UnixListener};

#[test]
fn gv_n02_one_slot_pause_bypass_restart_resume_reconciles_sqlite_cli_registered_bridge() {
    let _serial = serialization::serial();
    let mut case = Case::new(true);
    let normal = case.pressure("Normal", 0);
    let initial = switches(&case, true, 0, None);
    let mut a = Run::start(&case, "A", "build");
    let a_child = a.started();
    let a_id = case.job(a.wrapper.child.id());
    eventually(|| {
        case.rows("SELECT state FROM traffic_entries")
            .iter()
            .any(|p| p["state"] == "RUNNING")
    });
    let mut b = Run::start(&case, "B", "build");
    let b_id = queued(&case, &mut b);
    let before = permits(&case, 1, 1);
    let critical_before_pause = case.pressure(
        "Critical",
        normal["observation"]["sequence"].as_u64().unwrap(),
    );
    let pause = intent(0, false, 1);
    let committed = cli_set(&case, &pause);
    assert_eq!(committed["status"], "COMMITTED");
    let paused = switches(
        &case,
        false,
        1,
        Some(pause["mutation_id"].as_str().unwrap()),
    );
    let b_child = b.started();
    alive(&a_child);
    let after = permits(&case, 1, 1);
    assert_eq!(
        before, after,
        "B must not receive a new permit or alter A's reservation"
    );
    let b_running = job(&case, &b_id, &b, &b_child, false);
    assert_eq!(b_running["sqlite"]["mode"], "UNRESERVED");
    assert!(b_running["sqlite"]["generation"].is_null());
    let mut c = Run::start(&case, "C-paused-critical", "build");
    let c_child = c.started();
    let c_id = case.job(c.wrapper.child.id());
    assert_eq!(permits(&case, 1, 1), before);
    assert_eq!(c.finish().code(), Some(17));
    assert_eq!(b.finish().code(), Some(17));
    alive(&a_child);
    assert_eq!(a.finish().code(), Some(17));
    let jobs = vec![
        job(&case, &a_id, &a, &a_child, true),
        job(&case, &b_id, &b, &b_child, true),
        job(&case, &c_id, &c, &c_child, true),
    ];
    let released = permits(&case, 1, 0);
    let restarted = restart(&mut case);
    let persisted = switches(
        &case,
        false,
        1,
        Some(pause["mutation_id"].as_str().unwrap()),
    );
    assert_eq!(
        case.rows("SELECT child_pid FROM job_lifecycle WHERE child_pid IS NOT NULL")
            .len(),
        3
    );
    assert_eq!(
        (a.count(), b.count(), c.count()),
        (1, 1, 1),
        "restart must never replay children"
    );
    let mut d = Run::start(&case, "D-after-restart", "build");
    let d_child = d.started();
    let d_id = case.job(d.wrapper.child.id());
    permits(&case, 1, 0);
    assert_eq!(d.finish().code(), Some(17));
    let d_done = job(&case, &d_id, &d, &d_child, true);
    let resume = intent(1, true, 2);
    let resumed = bridge_set(&case, &resume);
    assert_eq!(resumed["status"], "COMMITTED");
    let active = switches(&case, true, 2, None);
    // Critical is freshly republished by the production Meter worker after restart.
    let critical = case.pressure("Critical", 0);
    let recovery_path = case.fixture.root.join("recovery.sock");
    let recovery_listener = UnixListener::bind(&recovery_path).unwrap();
    let mut e = Run::start_with_barrier(
        &case,
        "E-resumed-critical",
        "build",
        Some(("queued", &recovery_path)),
    );
    let (mut recovery_release, queued_checkpoint) = checkpoint(&recovery_listener);
    let e_id = queued(&case, &mut e);
    assert_eq!(queued_checkpoint["request"]["job_id"], e_id);
    eventually(|| {
        case.rpc("policy.job", json!({"job_id":e_id}))["last_decision"]["reason"]
            == "memory_pressure"
    });
    permits(&case, 1, 0);
    assert_eq!(e.count(), 0);
    let held = case
        .router
        .call("policy_job", json!({"job_id":e_id}))
        .unwrap();
    let recovered = case.pressure(
        "Normal",
        critical["observation"]["sequence"].as_u64().unwrap(),
    );
    recovery_release.write_all(b"x").unwrap();
    let e_child = e.started();
    let mut f = Run::start(&case, "F-resumed-window", "build");
    let f_id = queued(&case, &mut f);
    permits(&case, 2, 1);
    assert_eq!(f.count(), 0);
    alive(&e_child);
    assert_eq!(e.finish().code(), Some(17));
    let f_child = f.started();
    assert_eq!(f.finish().code(), Some(17));
    let final_jobs = vec![
        job(&case, &e_id, &e, &e_child, true),
        job(&case, &f_id, &f, &f_child, true),
    ];
    let final_permits = permits(&case, 3, 0);
    let final_switch = switches(&case, true, 2, None);
    let meter = case.router.call("meter_snapshot", json!({})).unwrap();
    let cli_meter = case.cli(&["meter", "snapshot", "--json"]);
    assert!(cli_meter.status.success());
    let cli_meter: serde_json::Value = serde_json::from_slice(&cli_meter.stdout).unwrap();
    assert_eq!(
        meter["data"]["daemon_instance"],
        cli_meter["daemon_instance"]
    );
    save_case(
        "GV-N02-06",
        json!({"normal":normal,"initial":initial,"A":a_child,"B":b_child,"BqueuedStarts":0,
        "beforePause":before,"criticalBeforePause":critical_before_pause,"pause":committed,"paused":paused,"BRunning":b_running,"afterPause":after,
        "pausedOutcomes":jobs,"released":released,"restart":restarted,"persistedOff":persisted,"postRestart":d_done,
        "resume":resumed,"active":active,"freshCritical":critical,"heldAfterResume":held,"queuedCheckpoint":queued_checkpoint,"recovered":recovered,
        "resumedOutcomes":final_jobs,"finalPermits":final_permits,"finalSwitch":final_switch,
        "actualChildCounts":[a.count(),b.count(),c.count(),d.count(),e.count(),f.count()],"meter":meter,"cliMeter":cli_meter}),
    );
    case.stop();
}

use super::{execution, native_support::*, serialization, support::*};
use serde_json::json;
use std::{fs, io::Write, os::unix::net::UnixListener};

#[test]
fn gv_n09_reserved_before_ready_pause_releases_reservation_and_launches_once() {
    let _serial = serialization::serial();
    let mut case = Case::new(true);
    let normal = case.pressure("Normal", 0);
    let path = case.fixture.root.join("reserved.sock");
    let listener = UnixListener::bind(&path).unwrap();
    let mut run = Run::start_with_barrier(
        &case,
        "reserved-convert",
        "build",
        Some(("reserved", &path)),
    );
    let (mut barrier, request) = checkpoint(&listener);
    assert_eq!(request["mode"], "RESERVED");
    let id = case.job(run.wrapper.child.id());
    assert_eq!(request["request"]["job_id"], id);
    let old = request["request"]["permit_generation"].as_u64().unwrap();
    let before = permits(&case, 1, 1);
    let critical = case.pressure(
        "Critical",
        normal["observation"]["sequence"].as_u64().unwrap(),
    );
    let paused = bridge_set(&case, &intent(0, false, 9));
    let retired = permits(&case, 1, 0);
    assert_eq!(run.count(), 0);
    barrier.write_all(b"x").unwrap();
    let child = run.started();
    assert_eq!(run.finish().code(), Some(17));
    let done = job(&case, &id, &run, &child, true);
    assert_eq!(done["sqlite"]["mode"], "UNRESERVED");
    assert_eq!(done["sqlite"]["previous_generation"], old);
    assert!(done["sqlite"]["generation"].is_null());
    let reads = switches(&case, false, 1, None);
    let final_permits = permits(&case, 1, 0);
    save_case(
        "GV-N09-reserved",
        json!({"checkpoint":request,"critical":critical,"before":before,"pause":paused,
        "retiredReservation":retired,"outcome":done,"finalPermits":final_permits,"reads":reads,"starts":run.count()}),
    );
    case.stop();
}

#[test]
fn gv_n10_pause_resume_before_reserved_launch_restores_critical_and_uses_fresh_generation() {
    let _serial = serialization::serial();
    let mut case = Case::new(true);
    let normal = case.pressure("Normal", 0);
    let path = case.fixture.root.join("resume.sock");
    let listener = UnixListener::bind(&path).unwrap();
    let mut run = Run::start_with_barrier(
        &case,
        "resume-before-ready",
        "build",
        Some(("reserved,queued", &path)),
    );
    let (mut barrier, request) = checkpoint(&listener);
    let id = case.job(run.wrapper.child.id());
    let old = request["request"]["permit_generation"].as_u64().unwrap();
    cli_set(&case, &intent(0, false, 10));
    permits(&case, 1, 0);
    bridge_set(&case, &intent(1, true, 11));
    let critical = case.pressure(
        "Critical",
        normal["observation"]["sequence"].as_u64().unwrap(),
    );
    barrier.write_all(b"x").unwrap();
    let (mut queue_release, queued_checkpoint) = checkpoint(&listener);
    assert_eq!(queued_checkpoint["request"]["action"], "POLL");
    assert_eq!(queued_checkpoint["request"]["job_id"], id);
    eventually(|| {
        case.rpc("policy.job", json!({"job_id":id}))["last_decision"]["reason"] == "memory_pressure"
    });
    assert_eq!(run.count(), 0);
    let held = permits(&case, 1, 0);
    let recovered = case.pressure(
        "Normal",
        critical["observation"]["sequence"].as_u64().unwrap(),
    );
    // Pause/resume retired the old reservation. Keep its queued poll behind an
    // explicit barrier until Normal is observed, before it can acquire READY.
    queue_release.write_all(b"x").unwrap();
    let child = run.started();
    assert_eq!(run.finish().code(), Some(17));
    let done = job(&case, &id, &run, &child, true);
    assert!(done["sqlite"]["generation"].as_u64().unwrap() > old);
    let final_permits = permits(&case, 1, 0);
    let reads = switches(&case, true, 2, None);
    save_case(
        "GV-N10-reserved-resume",
        json!({"oldRequest":request,"queuedCheckpoint":queued_checkpoint,"held":held,"critical":critical,
        "recovered":recovered,"outcome":done,"finalPermits":final_permits,"reads":reads,"starts":run.count()}),
    );
    case.stop();
}

#[test]
fn gv_n10_ready_before_pause_serializes_and_expired_unspawned_release_runs_once() {
    let _serial = serialization::serial();
    for fallback in [false, true] {
        let mut case = Case::new(true);
        if fallback {
            fs::write(case.fixture.root.join("pressure"), b"Denied").unwrap();
            eventually(|| case.current()["observation"]["availability"] != "available");
        } else {
            case.pressure("Normal", 0);
        }
        let path = case.fixture.root.join("ready.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let mut run =
            Run::start_with_barrier(&case, "ready-before-pause", "build", Some(("ready", &path)));
        let (mut barrier, ready) = checkpoint(&listener);
        assert_eq!(ready["release"]["state"], "RESERVED");
        let id = case.job(run.wrapper.child.id());
        let switch_before = switches(&case, true, 0, None);
        let blocked = case.cli(&[
            "daemon",
            "pause",
            "--json",
            "--expected-revision",
            "0",
            "--mutation-id",
            "0-00000000000000000000000000000012",
        ]);
        assert!(
            !blocked.status.success(),
            "an outstanding launch lease excludes the toggle"
        );
        let blocked_diagnostic = String::from_utf8(blocked.stderr).unwrap();
        assert!(
            blocked_diagnostic.contains("Change not confirmed"),
            "{blocked_diagnostic}"
        );
        let readback = switches(&case, true, 0, Some("0-00000000000000000000000000000012"));
        assert!(readback["registeredBridge"]["data"]["mutation_result"].is_null());
        // The production launch lease expires while the actual CLI is held
        // before exec. A switch may commit only after that lease is released.
        // Wait beyond the absolute production lease, while retaining concrete
        // no-spawn evidence at our barrier. The first server call has returned
        // DB_BUSY: there is no detached waiter. Replay the immutable identity,
        // never invent a new CAS base or invert the desired state.
        std::thread::sleep(std::time::Duration::from_millis(2100));
        let pause = cli_set(&case, &intent(0, false, 0x12));
        assert_eq!(pause["switch"]["revision"], 1);
        assert_eq!(run.count(), 0);
        barrier.write_all(b"x").unwrap();
        let child = run.started();
        assert_eq!(run.finish().code(), Some(17));
        let done = job(&case, &id, &run, &child, true);
        let final_permits = permits(&case, 1, 0);
        let reads = switches(&case, false, 1, None);
        save_case(
            if fallback {
                "GV-N10-ready-fallback"
            } else {
                "GV-N10-ready-healthy"
            },
            json!({"ready":ready,
            "before":switch_before,"blockedDiagnostic":blocked_diagnostic,"readbackWhileLeaseHeld":readback,
            "pause":pause,"outcome":done,"permits":final_permits,"reads":reads,"starts":run.count()}),
        );
        case.stop();
    }
}

#[test]
fn gv_n11_cancel_converted_unspawned_work_cannot_be_revived_by_resume() {
    let _serial = serialization::serial();
    let mut case = Case::new(true);
    case.pressure("Normal", 0);
    let path = case.fixture.root.join("cancel.sock");
    let listener = UnixListener::bind(&path).unwrap();
    let mut run = Run::start_with_barrier(
        &case,
        "cancel-converted",
        "build",
        Some(("reserved", &path)),
    );
    let (_barrier, request) = checkpoint(&listener);
    let id = case.job(run.wrapper.child.id());
    bridge_set(&case, &intent(0, false, 21));
    assert_eq!(
        unsafe { libc::kill(run.wrapper.child.id() as i32, libc::SIGINT) },
        0
    );
    let exit = execution::wait(&mut run.wrapper.child);
    assert_eq!(exit.code(), Some(1));
    assert_eq!(run.count(), 0);
    let before = case.rows("SELECT job_id,state FROM traffic_entries");
    assert_eq!(before[0]["state"], "CANCELLED");
    cli_set(&case, &intent(1, true, 22));
    assert_eq!(
        case.rows("SELECT job_id,state FROM traffic_entries"),
        before
    );
    // This wrapper has exited. Another socket opener cannot replay its owner
    // identity; status is an independent read, never an impersonated mutation.
    let mut replay = arc_ipc::Request::new("dead-owner-replay", "governor.job");
    replay.payload = json!({"action":"TRAFFIC","request":{"action":"POLL","job_id":id,"observer":request["request"]["observer"]}});
    let rejected = arc_ipc::exchange(&mut case.fixture.connect(), &replay).unwrap();
    assert_eq!(rejected.error.unwrap().code, arc_ipc::ERROR_INVALID_REQUEST);
    let cli = case.cli(&["job", "status", &id]);
    assert!(cli.status.success());
    assert!(String::from_utf8_lossy(&cli.stdout).contains("State: cancelled"));
    let final_permits = permits(&case, 1, 0);
    let bridge = case.router.call("job_by_id", json!({"job_id":id})).unwrap();
    assert!(case.rows("SELECT child_pid FROM job_lifecycle")[0]["child_pid"].is_null());
    save_case(
        "GV-N11-converted-cancel",
        json!({"checkpoint":request,"exit":exit.code(),"starts":run.count(),"terminal":before,
        "registeredBridge":bridge,"permits":final_permits,"diagnostic":fs::read_to_string(&run.stderr).unwrap()}),
    );
    case.stop();
}

//! Native daemon scenarios use ACK/readiness/release barriers, never deadline edits.
use super::*;
fn pressure(case: &Case, level: &str, previous: u64) -> Value {
    assert!(case.injected);
    write(&case.fixture.root.join("pressure-next"), level);
    fs::rename(
        case.fixture.root.join("pressure-next"),
        case.fixture.root.join("pressure"),
    )
    .unwrap();
    let mut got = Value::Null;
    eventually(|| {
        let value = case.rpc("policy.current", Value::Null);
        if value["observation"]["pressure"] == level.to_lowercase()
            && value["observation"]["availability"] == "available"
            && value["observation"]["sequence"]
                .as_u64()
                .is_some_and(|v| v > previous)
        {
            got = value;
            true
        } else {
            false
        }
    });
    got
}
fn control(case: &Case, enabled: bool) -> Value {
    let output = case.cli(&["daemon", if enabled { "resume" } else { "pause" }, "--json"]);
    assert!(output.status.success(), "{output:?}");
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["status"], "COMMITTED");
    assert_eq!(result["switch"]["enabled"], enabled);
    result
}
fn restart(case: &mut Case, crash: bool) {
    let identity = ProcessIdentity::capture(case.daemon.child.id()).unwrap();
    if crash {
        case.daemon.child.kill().unwrap();
        case.daemon.child.wait().unwrap();
        fs::remove_file(&case.fixture.socket).unwrap();
    } else {
        case.daemon.stop(libc::SIGTERM);
    }
    assert_eq!(probe_live(&identity), IdentityStatus::Missing);
    fs::rename(
        case.fixture.root.join("runtime.json"),
        case.fixture.root.join("runtime-before-restart.json"),
    )
    .unwrap();
    let mut command = case.fixture.command();
    if case.injected {
        command.env("ARC_PRESSURE_CONTROL", case.fixture.root.join("pressure"));
    }
    case.daemon = native::OwnedChild::new(command.spawn().unwrap());
    case.fixture.wait_for(&mut case.daemon, "arc-daemon ready.");
    assert_ne!(case.daemon.child.id(), identity.pid);
}
pub fn governor_pressure(read: Read) {
    let mut case = case("governor-critical", read, true);
    let critical = pressure(&case, "Critical", 0);
    let p = projects::project(&case, "held-python", Kind::Pytest, true);
    let mut run = start(&case, &p, &Kind::Pytest.argv());
    let queue = queued(&case, &mut run, Kind::Pytest, "memory_pressure");
    assert_eq!(case.active()["permits"], 0);
    let pause = control(&case, false);
    let entry = entered(&case, &mut run, Kind::Pytest);
    assert_eq!(case.active()["permits"], 0);
    let off = case.cli(&["daemon", "governor", "--json"]);
    assert!(off.status.success());
    restart(&mut case, false);
    assert_eq!(
        case.cli(&["daemon", "governor", "--json"]).stdout,
        off.stdout
    );
    assert!(run.wrapper.child.try_wait().unwrap().is_none());
    let resume = control(&case, true);
    assert_eq!(case.active()["permits"], 0);
    run.finish(0);
    let unreserved = terminal(&case, &run, Kind::Pytest, 0);
    assert!(unreserved["job"]["diagnostic"]["admission"]["reservation"].is_null());
    let normal = pressure(&case, "Normal", 0);
    let q = projects::project(&case, "resumed-python", Kind::Unittest, true);
    let mut resumed = start(&case, &q, &Kind::Unittest.argv());
    let resumed_entry = entered(&case, &mut resumed, Kind::Unittest);
    assert_eq!(case.active()["permits"], 1);
    resumed.finish(0);
    let done = terminal(&case, &resumed, Kind::Unittest, 0);
    let history = permit_history(&case, 1, 1);
    case.save("governor-critical", json!({"scope":"explicit pressure source into the production Meter worker; native remaining fields","critical":critical,"queued":queue,"pause":pause,"unreservedEntry":entry,"pausePersistedAcrossRestart":true,"resume":resume,"unreservedOutcome":unreserved,"normal":normal,"resumedEntry":resumed_entry,"reservedOutcome":done,"permits":history}));
    case.done();
}
pub fn cancel_restart(read: Read) {
    let mut case = case("cancel-restart", read, false);
    let p = projects::project(&case, "running", Kind::Pytest, true);
    let q = projects::project(&case, "cancelled", Kind::Unittest, true);
    let r = projects::project(&case, "surviving-queue", Kind::Pytest, true);
    let mut live = start(&case, &p, &Kind::Pytest.argv());
    let entry = entered(&case, &mut live, Kind::Pytest);
    let live_id = entry["jobId"].as_str().unwrap();
    let mut cancelled = start(&case, &q, &Kind::Unittest.argv());
    let before = queued(&case, &mut cancelled, Kind::Unittest, "class_window");
    let id = before["sqlite"]["id"].as_str().unwrap();
    assert_eq!(
        unsafe { libc::kill(cancelled.wrapper.child.id() as i32, libc::SIGTERM) },
        0
    );
    assert!(!cancelled.wait().success());
    eventually(|| case.row(id)["state"] == "CANCELLED");
    assert_eq!(log(&q.join("launches")).lines().count(), 0);
    assert!(!q.join("bodies").exists());
    let cancellation = case.reconcile(id, "TEST", "PYTHON_UNITTEST");
    let cancelled_meter = meter(&case, id);
    // The fresh CLI status observed the reaped wrapper. Cancellation proves no
    // child launch; Meter still preserves its interrupted lifecycle evidence.
    assert_eq!(
        cancelled_meter["accounting"],
        json!({"state":"unknown","reason":"interrupted"})
    );
    let barrier =
        std::os::unix::net::UnixListener::bind(case.fixture.root.join("queued.sock")).unwrap();
    let mut waiting = start_with(&case, &r, &Kind::Pytest.argv(), |cmd| {
        cmd.env("ARC_PRESSURE_ACCEPTANCE_STAGE", "queued").env(
            "ARC_PRESSURE_ACCEPTANCE_BARRIER",
            case.fixture.root.join("queued.sock"),
        );
    });
    let (mut release, checkpoint) = shared::protocol::checkpoint(&barrier);
    let waiting_before = queued(&case, &mut waiting, Kind::Pytest, "class_window");
    let waiting_id = waiting_before["sqlite"]["id"].as_str().unwrap();
    // Delay the queued client's next poll while the owned daemon is replaced.
    // This is a readiness barrier, not an enlarged production outage timeout.
    restart(&mut case, true);
    assert!(live.wrapper.child.try_wait().unwrap().is_none());
    assert_eq!(case.active()["permits"], 1);
    let recovered = case.reconcile(live_id, "TEST", "PYTHON_PYTEST");
    release.write_all(b"x").unwrap();
    assert_eq!(case.row(waiting_id)["state"], "QUEUED");
    assert_eq!(log(&r.join("launches")).lines().count(), 0);
    live.finish(0);
    let finished = terminal(&case, &live, Kind::Pytest, 0);
    let next = entered(&case, &mut waiting, Kind::Pytest);
    assert_eq!(next["jobId"], waiting_id);
    waiting.finish(0);
    eventually(|| case.row(waiting_id)["state"] == "COMPLETED");
    let next_done = case.reconcile(waiting_id, "TEST", "PYTHON_PYTEST");
    assert_eq!(next_done["sqlite"]["exit_code"], 0);
    assert_eq!(next_done["diagnostic"]["admission"]["permit_active"], false);
    assert_eq!(log(&r.join("launches")).lines().count(), 1);
    assert_eq!(log(&r.join("bodies")).lines().count(), 1);
    // This queued client met two daemon identities before attachment. Preserve
    // the existing uncertainty rule: a native exit is not invented accounting.
    assert!(log(&r.join("stderr")).contains("TRACKING_UNCERTAIN"));
    let meter_cli = case.cli(&["meter", "summary", "--job", waiting_id, "--json"]);
    assert!(meter_cli.status.success());
    let meter: Value = serde_json::from_slice(&meter_cli.stdout).unwrap();
    assert_eq!(
        meter,
        (case.read)(
            &case.fixture.socket,
            "meter_summary",
            json!({"job_id":waiting_id})
        )
    );
    assert_eq!(
        meter["accounting"],
        json!({"state":"unknown","reason":"unavailable"})
    );
    let history = permit_history(&case, 2, 1);
    case.save("cancel-restart", json!({"running":entry,"queuedCancellation":before,"cancelled":cancellation,"cancelledMeter":cancelled_meter,"cancelledStarts":0,"queuedBeforeRestart":waiting_before,"checkpoint":checkpoint,"runningAfterRestart":recovered,"firstOutcome":finished,"sameQueuedIdAfterRestart":next,"secondOutcome":next_done,"queuedMeterAfterPeerChange":meter,"starts":[1,0,1],"permits":history}));
    case.done();
}

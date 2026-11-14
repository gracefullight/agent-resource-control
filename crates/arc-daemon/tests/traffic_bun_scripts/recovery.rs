//! Production restart, queued cancellation and bounded unavailability.
use super::{projects::*, *};
pub fn run(read: Read) {
    let mut case = Case::new("recovery", read, false);
    let p = project(&case, "live", Kind::Test, true);
    let q = project(&case, "queued", Kind::Test, true);
    let mut live = Run::start(&case, &p, &Kind::Test.argv());
    let entry = live.entered(&case, false);
    let live_id = entry["jobId"].as_str().unwrap();
    let mut queued = Run::start(&case, &q, &Kind::Test.argv());
    let queued_id = case.job(queued.wrapper.child.id());
    eventually(|| case.row(&queued_id)["state"] == "QUEUED");
    assert_eq!(queued.count("bun"), 0);
    let before = case.reconcile(&queued_id, "TEST", "BUN_SCRIPT_TEST");
    assert_eq!(
        unsafe { libc::kill(queued.wrapper.child.id() as i32, libc::SIGTERM) },
        0
    );
    assert!(!queued.wait().success());
    eventually(|| case.row(&queued_id)["state"] == "CANCELLED");
    assert_eq!(queued.count("bun"), 0);
    let cancelled = case.reconcile(&queued_id, "TEST", "BUN_SCRIPT_TEST");
    // Crash only this owned isolated daemon; wrappers and real Bun survive.
    let first = case.daemon.child.id();
    case.daemon.child.kill().unwrap();
    case.daemon.child.wait().unwrap();
    // The crashed owned process is reaped before removing its stale socket.
    fs::remove_file(&case.fixture.socket).unwrap();
    assert!(live.wrapper.child.try_wait().unwrap().is_none());
    fs::rename(
        case.fixture.root.join("runtime.json"),
        case.fixture.root.join("runtime-before-restart.json"),
    )
    .unwrap();
    case.daemon = native::OwnedChild::new(case.fixture.command().spawn().unwrap());
    case.fixture.wait_for(&mut case.daemon, "arc-daemon ready.");
    assert_ne!(first, case.daemon.child.id());
    assert_eq!(case.active()["permits"], 1);
    let recovered = case.reconcile(live_id, "TEST", "BUN_SCRIPT_TEST");
    live.finish(0);
    let done = case.terminal(&live, "TEST", "BUN_SCRIPT_TEST", 0);
    assert_eq!(live.count("bun"), 1);
    assert_eq!(log(&p.join("bodies")).lines().count(), 1);
    // An absent endpoint exercises the CLI's existing absolute preparation bound.
    let outage = project(&case, "outage", Kind::Test, false);
    let start = Instant::now();
    let mut fail_open = Run::start_with(&case, &outage, &Kind::Test.argv(), |cmd| {
        cmd.env(
            arc_core::ARC_SOCKET_PATH,
            case.fixture.root.join("missing.sock"),
        );
    });
    fail_open.finish(0);
    assert!(
        start.elapsed() < Duration::from_secs(3),
        "absolute outage bound regressed: {:?}",
        start.elapsed()
    );
    assert_eq!(fail_open.count("bun"), 1);
    assert_eq!(log(&outage.join("bodies")).lines().count(), 1);
    validate_artifact(&case, &outage, Kind::Test);
    assert!(log(&outage.join("stderr")).contains("TRACKING_UNAVAILABLE"));
    assert_eq!(
        case.rows("SELECT count(*) AS n FROM job_lifecycle")[0]["n"],
        2
    );
    case.save("recovery",json!({"queued":before,"cancelled":cancelled,"daemonPids":[first,case.daemon.child.id()],"liveAfterCrash":recovered,"completed":done,"actualStarts":[1,0,1],"outageElapsedMs":start.elapsed().as_millis(),"outageNotice":log(&outage.join("stderr")),"fixtureCrashOnly":true}));
    case.done();
}

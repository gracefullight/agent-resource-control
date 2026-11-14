//! Explicit pressure injection is separate from default-policy real-tool proof.
use super::{projects::*, *};
fn pressure(case: &Case, level: &str, previous: u64) -> Value {
    assert!(case.injected);
    let next = case.fixture.root.join("pressure-next");
    write(&next, level);
    fs::rename(next, case.fixture.root.join("pressure")).unwrap();
    let mut got = Value::Null;
    eventually(|| {
        let p = case.rpc("policy.current", Value::Null);
        if p["observation"]["pressure"] == level.to_lowercase()
            && p["observation"]["availability"] == "available"
            && p["observation"]["sequence"]
                .as_u64()
                .is_some_and(|n| n > previous)
        {
            got = p;
            true
        } else {
            false
        }
    });
    got
}
fn set(case: &Case, enabled: bool, base: u64) -> Value {
    let rev = base.to_string();
    let mutation = format!("{base}-{:032x}", base + 1);
    let out = case.cli(&[
        "daemon",
        if enabled { "resume" } else { "pause" },
        "--json",
        "--expected-revision",
        &rev,
        "--mutation-id",
        &mutation,
    ]);
    assert!(out.status.success(), "{out:?}");
    let v: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(v["status"], "COMMITTED");
    v
}
pub fn run(read: Read) {
    let mut case = Case::new("governor-pressure", read, true);
    let critical = pressure(&case, "Critical", 0);
    let p = project(&case, "held-typecheck", Kind::Typecheck, true);
    let mut a = Run::start(&case, &p, &Kind::Typecheck.argv());
    let id = case.job(a.wrapper.child.id());
    eventually(|| case.row(&id)["state"] == "QUEUED");
    let held = case.reconcile(&id, "TYPECHECK", "BUN_SCRIPT_TYPECHECK");
    assert_eq!(
        held["diagnostic"]["admission"]["wait_reason"],
        "memory_pressure"
    );
    assert_eq!(a.count("bun"), 0);
    assert_eq!(case.active()["permits"], 0);
    let pause = set(&case, false, 0);
    let started = a.entered(&case, true);
    assert_eq!(case.active()["permits"], 0);
    let off = case.cli(&["daemon", "governor", "--json"]);
    assert!(off.status.success());
    let off: Value = serde_json::from_slice(&off.stdout).unwrap();
    assert_eq!(off["switch"]["enabled"], false);
    // Durable OFF survives a controlled fixture restart while the native child lives.
    case.daemon.stop(libc::SIGTERM);
    assert!(a.wrapper.child.try_wait().unwrap().is_none());
    let mut command = case.fixture.command();
    command.env("ARC_PRESSURE_CONTROL", case.fixture.root.join("pressure"));
    fs::rename(
        case.fixture.root.join("runtime.json"),
        case.fixture.root.join("runtime-before-restart.json"),
    )
    .unwrap();
    case.daemon = native::OwnedChild::new(command.spawn().unwrap());
    case.fixture.wait_for(&mut case.daemon, "arc-daemon ready.");
    let persisted = case.cli(&["daemon", "governor", "--json"]);
    assert!(persisted.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&persisted.stdout).unwrap(),
        off
    );
    let resume = set(&case, true, 1);
    assert!(a.wrapper.child.try_wait().unwrap().is_none());
    assert_eq!(case.active()["permits"], 0);
    a.finish(0);
    let unreserved = case.terminal(&a, "TYPECHECK", "BUN_SCRIPT_TYPECHECK", 0);
    assert!(unreserved["job"]["diagnostic"]["admission"]["reservation"].is_null());
    let normal = pressure(&case, "Normal", 0);
    let q = project(&case, "resumed-typecheck", Kind::Typecheck, true);
    let mut b = Run::start(&case, &q, &Kind::Typecheck.argv());
    let reserved = b.entered(&case, true);
    assert_eq!(case.active()["permits"], 1);
    b.finish(0);
    let done = case.terminal(&b, "TYPECHECK", "BUN_SCRIPT_TYPECHECK", 0);
    validate_artifact(&case, &q, Kind::Typecheck);
    case.save("governor-pressure",json!({"pressureSource":"explicit injected field in production Meter worker; other host fields native","critical":critical,"held":held,"pause":pause,"unreservedEntry":started,"persistedOff":off,"resume":resume,"unreservedOutcome":unreserved,"normal":normal,"reservedEntry":reserved,"reservedOutcome":done,"starts":[1,1]}));
    case.done();
}

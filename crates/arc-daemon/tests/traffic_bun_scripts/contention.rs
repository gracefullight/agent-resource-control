//! Synchronization uses real Cargo/Vite entry or an open tsc source reader.
use super::{projects::*, *};
fn queued(case: &Case, run: &mut Run, kind: Kind, reason: &str) -> Value {
    let id = case.job(run.wrapper.child.id());
    eventually(|| {
        case.row(&id)["state"] == "QUEUED"
            && log(&run.project.join("stderr")).contains("QUEUED\nWaiting to start.")
    });
    assert_eq!(run.count("bun") + run.count("cargo"), 0);
    assert!(!run.project.join("bodies").exists());
    assert!(cleanup::children(run.wrapper.child.id())
        .unwrap()
        .is_empty());
    let snapshot = case.reconcile(&id, kind.class(), kind.reason());
    assert_eq!(snapshot["diagnostic"]["admission"]["wait_reason"], reason);
    assert_eq!(snapshot["diagnostic"]["admission"]["permit_active"], false);
    snapshot
}
fn pair(read: Read, name: &str, kinds: [Kind; 2], wait: &str, injected_memory: bool) {
    let case = Case::new(name, read, false);
    let paths = [
        project(&case, "first", kinds[0], true),
        project(&case, "second", kinds[1], true),
    ];
    assert_ne!(paths[0], paths[1]);
    let mut a = Run::start(&case, &paths[0], &kinds[0].argv());
    let a_entry = a.entered(&case, kinds[0] == Kind::Typecheck);
    let a_id = a_entry["jobId"].as_str().unwrap();
    if injected_memory {
        // Default reservations hit CPU before memory. This separately labeled
        // fixture changes only A's estimate to exercise the production memory
        // branch with two real, different-class invocations and spare CPU.
        assert_eq!(kinds[0], Kind::Typecheck);
        let output = Command::new("/usr/bin/sqlite3")
            .arg(&case.fixture.database)
            .args([
                "-cmd",
                ".timeout 2000",
                "UPDATE permits SET memory_bytes_est=3758096384 WHERE state='ACTIVE';",
            ])
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        assert_eq!(case.active()["permits"], 1);
        assert_eq!(case.active()["cpu"], 1000);
        assert_eq!(case.active()["memory"], 3758096384_u64);
    }
    let before = case.reconcile(a_id, kinds[0].class(), kinds[0].reason());
    let mut b = Run::start_with(&case, &paths[1], &kinds[1].argv(), |cmd| {
        cmd.env("TBS_PREDECESSOR", a_id)
            .env("TBS_PREDECESSOR_PID", a_entry["child"]["pid"].to_string());
    });
    let queue = queued(&case, &mut b, kinds[1], wait);
    assert!(a.wrapper.child.try_wait().unwrap().is_none());
    assert_eq!(case.active()["permits"], 1);
    assert!(case.active()["cpu"].as_u64().unwrap() <= 2000);
    assert!(case.active()["memory"].as_u64().unwrap() <= 4294967296);
    let healthy_wait = Instant::now();
    if kinds == [Kind::Typecheck, Kind::Typecheck] {
        // A healthy queue outlives the 1800 ms outage-preparation deadline.
        // Both gates remain held and each read observes durable queuing.
        while healthy_wait.elapsed() < Duration::from_millis(2100) {
            assert!(a.wrapper.child.try_wait().unwrap().is_none());
            assert_eq!(b.count("bun"), 0);
            assert!(cleanup::children(b.wrapper.child.id()).unwrap().is_empty());
            let snapshot = case.reconcile(
                queue["sqlite"]["id"].as_str().unwrap(),
                kinds[1].class(),
                kinds[1].reason(),
            );
            assert_eq!(snapshot["diagnostic"]["admission"]["state"], "QUEUED");
            assert_eq!(case.active()["permits"], 1);
            thread::sleep(Duration::from_millis(40));
        }
    }
    // Both own independent source/output roots; no tool lock is shared.
    a.finish(0);
    let a_done = case.terminal(&a, kinds[0].class(), kinds[0].reason(), 0);
    let b_entry = b.entered(&case, kinds[1] == Kind::Typecheck);
    let b_id = b_entry["jobId"].as_str().unwrap();
    assert_ne!(a_id, b_id);
    let ar = case.row(a_id);
    let br = case.row(b_id);
    assert!(br["granted_at_ms"].as_u64().unwrap() >= ar["released_at"].as_u64().unwrap());
    assert!(log(&paths[1].join("release-witness")).contains("RELEASED"));
    assert_eq!(
        probe_live(&serde_json::from_value(a_entry["child"].clone()).unwrap()),
        IdentityStatus::Missing
    );
    assert_eq!(case.active()["permits"], 1);
    b.finish(0);
    let b_done = case.terminal(&b, kinds[1].class(), kinds[1].reason(), 0);
    let artifacts = [
        validate_artifact(&case, &paths[0], kinds[0]),
        validate_artifact(&case, &paths[1], kinds[1]),
    ];
    for (r, k) in [(&a, kinds[0]), (&b, kinds[1])] {
        assert_eq!(r.count(if k == Kind::Cargo { "cargo" } else { "bun" }), 1);
        if k != Kind::Typecheck {
            assert_eq!(log(&r.project.join("bodies")).lines().count(), 1);
        }
    }
    assert_eq!(case.active()["permits"], 0);
    case.save(name,json!({"realTools":true,"memoryEstimateInjection":injected_memory,"independentRoots":paths,"entry":[a_entry,b_entry],"firstBefore":before,"queued":queue,"terminal":[a_done,b_done],"originalStarts":[1,1],"bodyStarts":[1,1],"secondExecReleaseWitness":log(&paths[1].join("release-witness")),"artifacts":artifacts,"activePermitsAtEnd":0}));
    case.done();
}
pub fn builds(read: Read, cargo_first: bool) {
    pair(
        read,
        if cargo_first {
            "cargo-first"
        } else {
            "bun-script-first"
        },
        if cargo_first {
            [Kind::Cargo, Kind::Vite]
        } else {
            [Kind::Vite, Kind::Cargo]
        },
        "class_window",
        false,
    );
}
pub fn typechecks(read: Read) {
    pair(
        read,
        "typecheck-window",
        [Kind::Typecheck, Kind::Typecheck],
        "class_window",
        false,
    );
}
pub fn cpu(read: Read) {
    pair(
        read,
        "mixed-cpu",
        [Kind::Vite, Kind::Typecheck],
        "cpu_budget",
        false,
    );
}
pub fn memory(read: Read) {
    pair(
        read,
        "mixed-memory",
        [Kind::Typecheck, Kind::Test],
        "memory_budget",
        true,
    );
}

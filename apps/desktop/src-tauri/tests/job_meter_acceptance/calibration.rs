use super::{serialization, support::*};
use serde_json::{json, Value};
use std::{
    fs,
    process::{Command, Stdio},
    time::Instant,
};

fn digest(path: &std::path::Path) -> String {
    let result = Command::new("/usr/bin/shasum")
        .args(["-a", "256"])
        .arg(path)
        .output()
        .unwrap();
    assert!(result.status.success());
    String::from_utf8(result.stdout)
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_owned()
}
fn seconds(raw: &Value) -> f64 {
    raw["seconds"].as_f64().unwrap() + raw["microseconds"].as_f64().unwrap() / 1e6
}

#[test]
fn at_mtr_01_two_workers_ten_seconds_reaped_descendants_persist_on_all_surfaces() {
    let _serial = serialization::serial();
    let mut case = setup("at-mtr-01");
    let source = workspace().join("scripts/job-meter/native-workload.c");
    let workload = case.fixture.root.join("native-workload");
    let compiler = Command::new("clang")
        .args(["-std=c11", "-O2", "-Wall", "-Wextra", "-Werror"])
        .arg(&source)
        .arg("-o")
        .arg(&workload)
        .output()
        .unwrap();
    assert!(compiler.status.success(), "{compiler:?}");
    let mut command = Command::new(case.fixture.binary("arcctl"));
    command
        .args(["exec", "--"])
        .arg(&workload)
        .arg("10")
        .env(arc_core::ARC_SOCKET_PATH, &case.fixture.socket)
        .env_remove("ARC_TRAFFIC_LINEAGE")
        .stdin(Stdio::null());
    let start = Instant::now();
    let mut run = Run::launch(&case, command);
    assert!(run.finish().success(), "{}", run.errors());
    let actual_elapsed = start.elapsed().as_secs_f64();
    let reference: Value = serde_json::from_str(&run.output()).unwrap();
    let id = job_id(&case, run.identity.pid);
    let surfaces = reconcile(&case, &id);
    let recorded = &surfaces["sqliteReport"];
    assert_eq!(recorded["child"]["pid"], reference["root"]["pid"]);
    assert_eq!(
        recorded["child"]["start_token"],
        reference["root"]["start_token"]
    );
    assert_eq!(
        recorded["child"]["boot_id"].as_str().unwrap(),
        reference["boot_id"].as_str().unwrap().to_lowercase()
    );
    assert_eq!(recorded["scope"], reference["scope"]);
    assert_eq!(reference["worker_count"], 2);
    let workers = reference["workers"].as_array().unwrap();
    assert_eq!(workers.len(), 2);
    let mut identities = std::collections::HashSet::new();
    for worker in workers {
        assert!(identities.insert(worker["identity"]["pid"].as_u64().unwrap()));
        assert_eq!(worker["short_lived_reaped"], 4);
        assert_eq!(worker["short_lived"].as_array().unwrap().len(), 4);
        for child in worker["short_lived"].as_array().unwrap() {
            assert!(identities.insert(child["pid"].as_u64().unwrap()));
            assert!(
                child["start_token"]
                    .as_str()
                    .unwrap()
                    .parse::<u64>()
                    .unwrap()
                    > 0
            );
        }
    }
    let raw = &reference["raw"];
    let user = seconds(&raw["self_user"]) + seconds(&raw["children_user"]);
    let system = seconds(&raw["self_system"]) + seconds(&raw["children_system"]);
    let independent = user + system;
    assert!((independent - reference["total_core_seconds"].as_f64().unwrap()).abs() < 1e-9);
    let measured = recorded["cpu_core_seconds"]["total"].as_f64().unwrap();
    let error = (measured - independent).abs();
    let allowed = 0.1f64.max(0.05 * independent);
    let elapsed = recorded["run_elapsed"]["seconds"].as_f64().unwrap();
    assert!((9.5..15.0).contains(&elapsed), "{elapsed}");
    assert!((9.5..15.0).contains(&reference["elapsed_seconds"].as_f64().unwrap()));
    assert!(independent > 1.0, "workers must consume actual CPU");
    assert!(
        error <= allowed,
        "recorded {measured}; reference {independent}; error {error} > {allowed}"
    );
    assert_eq!(
        recorded["completeness"], "partial",
        "opaque production commands remain conservatively Partial"
    );
    assert_eq!(
        recorded["missing_coverage"],
        json!(["unobserved_descendants"])
    );
    assert_eq!(surfaces["cli"]["queue_wait"]["seconds"], 0.0);
    let snapshot = ok(&case, "meter.snapshot", Value::Null);
    assert!(arc_ipc::meter::decode_snapshot(&snapshot)
        .unwrap()
        .is_valid());
    assert!(snapshot.get("job_id").is_none());
    let before = case.identity();
    case.stop("calibration-before-restart");
    let offline = case.bridge.daemon_health().unwrap_err();
    assert!(!case.fixture.socket.exists());
    case.start();
    let after = case.identity();
    assert_ne!(before, after);
    let health = case.bridge.daemon_health().unwrap();
    assert_eq!(health.data.protocol_version, 1);
    let restarted = reconcile(&case, &id);
    assert_eq!(surfaces["cli"], restarted["cli"]);
    let recent_errors = fs::read_to_string(case.fixture.root.join("stderr")).unwrap();
    assert!(recent_errors.is_empty(), "{recent_errors}");
    save(
        "at_mtr_01_two_workers_ten_seconds_reaped_descendants_persist_on_all_surfaces",
        json!({
            "workerCount":2,"shortLivedDescendantsReaped":true,"scopeMatched":true,
            "scope":"native_waited_descendants","elapsedSeconds":elapsed,"outerElapsedSeconds":actual_elapsed,
            "reference":reference,"referenceUserCoreSeconds":user,"referenceSystemCoreSeconds":system,
            "recordedUserCoreSeconds":recorded["cpu_core_seconds"]["user"],
            "recordedSystemCoreSeconds":recorded["cpu_core_seconds"]["system"],
            "recordedTotalCoreSeconds":measured,"absoluteErrorCoreSeconds":error,"allowedErrorCoreSeconds":allowed,
            "withinTolerance":true,"surfaces":surfaces,"afterRestart":restarted,
            "restart":{"before":before,"after":after,"offlineHealthError":offline,"health":health,
                "recentStderr":recent_errors,"socketOwnerMode":"0600","privateRuntimeMode":"0700"},
            "sources":[{"path":"scripts/job-meter/native-workload.c","sha256":digest(&source)},
                {"path":"apps/desktop/src-tauri/tests/job_meter_acceptance/calibration.rs",
                 "sha256":digest(&workspace().join("apps/desktop/src-tauri/tests/job_meter_acceptance/calibration.rs"))}],
            "measurementTail":"C reference is sampled after all waits and before final formatting/exit; terminal wait4 includes that tail.",
            "basis":"Independent C getrusage SELF + waited CHILDREN integer microseconds; production PID-specific wait4. No production conversion helper used by reference.",
            "snapshot":snapshot,"launches":1,"fixture":{"runtime":case.fixture.root,"database":case.fixture.database,"socket":case.fixture.socket}
        }),
    );
    case.stop("calibration-done");
}

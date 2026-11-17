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
    let calibration_source = case.fixture.root.join("native-workload-calibration.c");
    let workload = case.fixture.root.join("native-workload");
    // The shared C reference starts its elapsed clock after identity and boot
    // queries, whereas ARC starts immediately before spawn. Anchor the test's
    // independent reference to Darwin's native process birth timestamp, then
    // measure the remaining workload with CLOCK_MONOTONIC. The 09:57 failure
    // output was not retained; this is a boundary correction, not a claim
    // that its cause was reproduced. CPU getrusage remains unchanged.
    let original = fs::read_to_string(&source).unwrap();
    let before_start = "double started = now(), deadline = started + duration;";
    let elapsed_arg = "now() - started);";
    let elapsed_field = "\\\"elapsed_seconds\\\":%.9f,\\\"workers\\\":[";
    let worker_end =
        "    burn_until(deadline);\n    write_all(receipt_fd, &receipt, sizeof(receipt));";
    assert_eq!(original.matches(before_start).count(), 1);
    assert_eq!(original.matches(elapsed_arg).count(), 1);
    assert_eq!(original.matches(elapsed_field).count(), 1);
    assert_eq!(original.matches(worker_end).count(), 1);
    let reference = original
        .replace(
            "#include <time.h>",
            "#include <time.h>\n#include <sys/time.h>",
        )
        .replace(
            before_start,
            r#"struct timeval reference_wall;
    if (gettimeofday(&reference_wall, NULL)) fail("gettimeofday");
    double reference_startup = (double)reference_wall.tv_sec + (double)reference_wall.tv_usec / 1e6
                             - (double)own.start / 1e6;
    if (!isfinite(reference_startup) || reference_startup < 0) return 2;
    double started = now(), deadline = started + duration;"#,
        )
        .replace(
            elapsed_arg,
            "now() - started + reference_startup, reference_startup);",
        )
        .replace(
            elapsed_field,
            "\\\"elapsed_seconds\\\":%.9f,\\\"birth_to_work_seconds\\\":%.9f,\\\"workers\\\":[",
        )
        .replace(
            worker_end,
            r#"    burn_until(deadline);
    /* A busy host may schedule the workers for less than one CPU second
       during the fixed ten-second interval. Preserve the CPU-work assertion
       by completing a minimum amount of actual work in each worker. */
    struct rusage worker_usage = {0};
    volatile uint64_t work = 1;
    for (;;) {
        if (getrusage(RUSAGE_SELF, &worker_usage)) fail("worker getrusage");
        int64_t cpu_us = ((int64_t)worker_usage.ru_utime.tv_sec + worker_usage.ru_stime.tv_sec) * 1000000
                       + worker_usage.ru_utime.tv_usec + worker_usage.ru_stime.tv_usec;
        if (cpu_us >= 510000) break;
        for (unsigned n = 0; n < 32768; ++n) work = work * 1664525u + 1013904223u;
    }
    write_all(receipt_fd, &receipt, sizeof(receipt));"#,
        );
    fs::write(&calibration_source, reference).unwrap();
    let compiler = Command::new("clang")
        .args(["-std=c11", "-O2", "-Wall", "-Wextra", "-Werror"])
        .arg(&calibration_source)
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
    let reference_elapsed = reference["elapsed_seconds"].as_f64().unwrap();
    let startup = reference["birth_to_work_seconds"].as_f64().unwrap();
    assert!(startup >= 0.0 && startup < reference_elapsed);
    // Preserve the original tolerance calculated from the workload-only
    // interval; adding the birth boundary must not enlarge the tolerance.
    let elapsed_tolerance = 0.1f64.max(0.05 * (reference_elapsed - startup));
    println!(
        "native calibration measured={elapsed:.6}s reference={reference_elapsed:.6}s tolerance={elapsed_tolerance:.6}s error={:.6}s birth_to_work={:.6}s",
        (elapsed - reference_elapsed).abs(),
        startup
    );
    // The workload's configured duration is part of the case; accuracy is
    // checked only against the independent reference from this execution.
    assert_eq!(reference["requested_seconds"], 10.0);
    assert!((elapsed - reference_elapsed).abs() <= elapsed_tolerance);
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
                {"path":"generated native-workload-calibration.c","sha256":digest(&calibration_source)},
                {"path":"apps/desktop/src-tauri/tests/job_meter_acceptance/calibration.rs",
                 "sha256":digest(&workspace().join("apps/desktop/src-tauri/tests/job_meter_acceptance/calibration.rs"))}],
            "elapsedReference":"Darwin process birth wall timestamp to a paired wall/monotonic sample, then monotonic to the final reference sample",
            "measurementTail":"C reference is sampled after all waits and before final formatting/exit; terminal wait4 includes that tail.",
            "basis":"Independent C getrusage SELF + waited CHILDREN integer microseconds; production PID-specific wait4. No production conversion helper used by reference.",
            "snapshot":snapshot,"launches":1,"fixture":{"runtime":case.fixture.root,"database":case.fixture.database,"socket":case.fixture.socket}
        }),
    );
    case.stop("calibration-done");
}

//! Current and accepted legacy production daemons, authenticated CLI + actual
//! registered router. Every child has a private runtime and owned Child handle.
use super::{
    daemon_bridge_support::{binary, detail, HeldJob, RealDaemon},
    meter_router::Router,
    serialization,
};
use arc_core::meter::{MeterSnapshot, Reading, SamplingMode};
use serde_json::{json, Value};
use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
    thread,
    time::{Duration, Instant},
};

fn cli(case: &RealDaemon) -> Output {
    let mut command = Command::new(binary("arcctl"));
    command
        .args(["meter", "snapshot", "--json"])
        .env(arc_core::ARC_SOCKET_PATH, &case.fixture.socket);
    case.profile_command(&mut command);
    command.output().unwrap()
}
fn next(case: &RealDaemon, previous: u64) -> MeterSnapshot {
    let deadline = Instant::now() + Duration::from_secs(35);
    loop {
        let output = cli(case);
        if output.status.success() {
            let value = serde_json::from_slice(&output.stdout).unwrap();
            let sample = arc_ipc::meter::decode_snapshot(&value).unwrap();
            if sample.sequence > previous {
                return sample;
            }
        } else {
            assert!(
                String::from_utf8_lossy(&output.stderr).starts_with("METER_UNAVAILABLE"),
                "{output:?}"
            );
        }
        assert!(
            Instant::now() < deadline,
            "sampler did not advance beyond {previous}"
        );
        thread::sleep(Duration::from_millis(100));
    }
}
fn routed(router: &Router) -> MeterSnapshot {
    let value = router.call("meter_snapshot", json!({})).unwrap();
    arc_ipc::meter::decode_snapshot(&value["data"]).unwrap()
}
fn available(sample: &MeterSnapshot) {
    assert!(sample.is_valid());
    assert!(sample.host.cpu_utilization.reading.value().is_some());
    assert!(sample.host.memory_available_bytes.reading.value().is_some());
    assert!(sample.host.memory_pressure.reading.value().is_some());
    assert!(sample.host.swap_used_bytes.reading.value().is_some());
    assert!(sample.host.filesystem.identity.as_ref().unwrap().is_local);
    assert!(sample
        .host
        .filesystem
        .capacity_bytes
        .reading
        .value()
        .is_some());
    assert!(sample.host.filesystem.free_bytes.reading.value().is_some());
    assert!(sample.daemon.cpu_core_seconds.reading.value().is_some());
    assert!(sample.daemon.rss_bytes.reading.value().is_some());
}

fn legacy_peer(root: &std::path::Path) -> Value {
    let receipt_path = root.join("target/host-meter/legacy/latest.json");
    let cached = fs::read(&receipt_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .filter(|receipt| {
            receipt["acceptedHead"] == "efedf0f4eb9b97eed641c82f3c4766bb16f2e3de"
                && receipt["exitCode"] == 0
                && receipt["binary"]["path"]
                    .as_str()
                    .is_some_and(|path| root.join(path).is_file())
        });
    if let Some(receipt) = cached {
        return receipt;
    }
    // The maintained preparer uses a separate source/target directory, locked
    // offline Cargo and a two-hour child deadline. It can run while this test's
    // Cargo invocation owns the current workspace lock, including under llvm-cov.
    let status = Command::new("bun")
        .arg("scripts/host-meter/prepare-legacy.ts")
        .current_dir(root)
        .env("CARGO_NET_OFFLINE", "true")
        .status()
        .expect("start bounded offline legacy compatibility preparation");
    assert!(
        status.success(),
        "legacy compatibility preparation: {status}"
    );
    serde_json::from_slice(&fs::read(receipt_path).expect("prepared legacy receipt")).unwrap()
}

#[test]
fn successive_native_samples_headless_cli_router_and_restart() {
    let _serial = serialization::serial();
    let mut case =
        RealDaemon::host_meter("successive_native_samples_headless_cli_router_and_restart");
    let first = next(&case, 0);
    assert_eq!(first.sequence, 1);
    assert!(matches!(
        first.host.cpu_utilization.reading,
        Reading::FirstInterval {}
    ));
    // No router/window is constructed until native passes have advanced without it.
    let headless = next(&case, first.sequence);
    available(&headless);
    assert_eq!(headless.sequence, first.sequence + 1);
    assert!(headless.sampled_monotonic_ms > first.sampled_monotonic_ms);
    assert!(matches!(
        headless.cadence.mode,
        SamplingMode::Idle | SamplingMode::Pressure
    ));
    let mut job = HeldJob::launch(&case, 17);
    let running = case.wait_running(&job.identity);
    let id = detail(&running).job_id.as_str().to_owned();
    let router = Router::new(case.bridge.clone());
    let mut previous = headless.clone();
    let mut samples = vec![];
    for _ in 0..10 {
        let current = next(&case, previous.sequence);
        available(&current);
        assert_eq!(current.sequence, previous.sequence + 1);
        assert_eq!(current.daemon_instance, case.identity());
        assert!(current.sampled_monotonic_ms > previous.sampled_monotonic_ms);
        let interval = current.host.cpu_interval.as_ref().unwrap();
        assert_eq!(interval.started_monotonic_ms, previous.sampled_monotonic_ms);
        assert_eq!(interval.ended_monotonic_ms, current.sampled_monotonic_ms);
        assert_eq!(
            interval.elapsed_ms,
            current.sampled_monotonic_ms - previous.sampled_monotonic_ms
        );
        assert!(matches!(
            current.cadence.mode,
            SamplingMode::Active | SamplingMode::Pressure
        ));
        let through_router = routed(&router);
        assert_eq!(through_router.daemon_instance, current.daemon_instance);
        assert!(through_router.sequence >= current.sequence);
        assert_eq!(
            router.call("daemon_health", json!({})).unwrap()["data"]["protocol_version"],
            1
        );
        assert_eq!(
            router.call("job_by_id", json!({"job_id":id})).unwrap()["data"]["job"]["job_id"],
            id
        );
        samples.push(json!({"cli":current,"registeredRouter":through_router}));
        previous = current;
    }
    assert_eq!(job.release_and_wait().code(), Some(17));
    let completed = case.bridge.job_by_id(&id).unwrap();
    let idle = next(&case, previous.sequence);
    assert!(matches!(
        idle.cadence.mode,
        SamplingMode::Idle | SamplingMode::Pressure
    ));
    let before_restart = case.identity();
    case.stop("before-meter-restart");
    assert_eq!(
        router.call("meter_snapshot", json!({})).unwrap_err()["code"],
        "unavailable"
    );
    assert_eq!(
        router.call("daemon_health", json!({})).unwrap_err()["code"],
        "unavailable"
    );
    assert!(!cli(&case).status.success());
    case.start();
    let restarted = next(&case, 0);
    assert_ne!(case.identity(), before_restart);
    assert_eq!(restarted.daemon_instance, case.identity());
    assert_eq!(restarted.sequence, 1);
    assert!(matches!(
        restarted.host.cpu_utilization.reading,
        Reading::FirstInterval {}
    ));
    assert_eq!(
        restarted.daemon.peak_observation_started_monotonic_ms,
        restarted.sampled_monotonic_ms
    );
    assert_eq!(
        restarted.daemon.observed_peak_rss_bytes.reading,
        restarted.daemon.rss_bytes.reading
    );
    assert_eq!(routed(&router).daemon_instance, restarted.daemon_instance);
    assert_eq!(
        router.call("daemon_health", json!({})).unwrap()["data"]["protocol_version"],
        1
    );
    assert_eq!(
        router.call("job_by_id", json!({"job_id":id})).unwrap()["data"]["job"]["outcome"],
        json!(detail(&completed).outcome)
    );
    case.save("native-meter", &json!({"first":first,"headless":headless,"successiveSamples":samples,
        "afterCompletion":idle,"beforeRestart":before_restart,"restart":restarted,"completed":completed,
        "headlessObservedBeforeRouter":true,"defaultProductionLimits":true,"persistedResourceSamples":0}));
    case.stop("after-meter-restart");
}

#[test]
fn accepted_old_daemon_is_unsupported_without_disabling_health_or_jobs() {
    let _serial = serialization::serial();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap()
        .to_owned();
    let receipt = legacy_peer(&root);
    assert_eq!(
        receipt["acceptedHead"],
        "efedf0f4eb9b97eed641c82f3c4766bb16f2e3de"
    );
    let legacy = root.join(receipt["binary"]["path"].as_str().unwrap());
    assert!(legacy.is_file(), "missing accepted old-daemon prerequisite");
    let mut case = RealDaemon::meter_process(
        "accepted_old_daemon_is_unsupported_without_disabling_health_or_jobs",
        legacy,
    );
    let router = Router::new(case.bridge.clone());
    let mut job = HeldJob::launch(&case, 17);
    let running = case.wait_running(&job.identity);
    let id = detail(&running).job_id.as_str().to_owned();
    let output = cli(&case);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).starts_with("UNSUPPORTED_CAPABILITY"));
    assert_eq!(
        router.call("meter_snapshot", json!({})).unwrap_err()["code"],
        "unsupported"
    );
    assert_eq!(
        router.call("daemon_health", json!({})).unwrap()["data"]["protocol_version"],
        1
    );
    assert_eq!(
        router.call("recent_jobs", json!({})).unwrap()["data"]["jobs"][0]["job_id"],
        id
    );
    assert_eq!(job.release_and_wait().code(), Some(17));
    let selected = router.call("job_by_id", json!({"job_id":id})).unwrap();
    assert_eq!(
        selected["data"]["job"]["outcome"],
        json!({"kind":"EXITED","code":17})
    );
    case.save(
        "old-daemon",
        &json!({"legacyBuild":receipt,"daemon":case.identity(),
        "cliExit":output.status.code(),"cliStderr":String::from_utf8_lossy(&output.stderr),
        "meterError":"unsupported","selected":selected}),
    );
    case.stop("legacy-stopped");
}

//! Forced per-field failures in a test entrypoint using the actual daemon
//! supervisor/collector/sampler and real authenticated CLI. Not calibration.
use super::{runtime, support};
use arc_core::{
    meter::{MeterSnapshot, Reading},
    process_identity::ProcessIdentity,
};
use serde_json::json;
use std::{
    fs,
    path::PathBuf,
    process::Command,
    thread,
    time::{Duration, Instant},
};

#[test]
fn partial_native_failures_preserve_available_fields_cli_health_and_jobs() {
    let _serial = support::serial();
    let fixture = runtime::Fixture::new();
    // spawn() keeps the production socket/instance preparation and supervisor.
    // Only the child environment carries the test-only source fault.
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args([
            "--exact",
            "controlled_daemon_child",
            "--nocapture",
            "--test-threads=1",
        ])
        .env("ARC_BOUNDED_CHILD", "meter-partial")
        .env("ARC_TEST_METER_PARTIAL", "1");
    fixture.configure(&mut command);
    let mut child = runtime::OwnedChild::new(command.spawn().unwrap());
    fixture.wait_for(&mut child, "arc-daemon ready.");
    let identity = ProcessIdentity::capture(child.child.id()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    let sample: MeterSnapshot = loop {
        let output = Command::new(fixture.binary("arcctl"))
            .args(["meter", "snapshot", "--json"])
            .env(arc_core::ARC_SOCKET_PATH, &fixture.socket)
            .output()
            .unwrap();
        if output.status.success() {
            assert!(output.stderr.is_empty());
            break arc_ipc::meter::decode_snapshot(
                &serde_json::from_slice(&output.stdout).unwrap(),
            )
            .unwrap();
        }
        assert!(String::from_utf8_lossy(&output.stderr).starts_with("METER_UNAVAILABLE"));
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(5));
    };
    assert_eq!(sample.daemon_instance, identity);
    assert!(matches!(
        sample.host.memory_available_bytes.reading,
        Reading::Denied {}
    ));
    assert!(matches!(
        sample.host.swap_used_bytes.reading,
        Reading::Failed { .. }
    ));
    assert!(sample.host.memory_pressure.reading.value().is_some());
    assert!(sample.host.filesystem.free_bytes.reading.value().is_some());
    assert!(sample.daemon.rss_bytes.reading.value().is_some());
    fixture.assert_cli();
    let query = arc_ipc::Request::new("partial-health", arc_ipc::OPERATION_HEALTH);
    assert_eq!(
        arc_ipc::exchange(&mut fixture.connect(), &query)
            .unwrap()
            .status,
        "ok"
    );
    let mut query = arc_ipc::Request::new("partial-jobs", arc_ipc::jobs::OPERATION_JOB_STATUS);
    query.payload = json!({"query":"RECENT"});
    assert_eq!(
        arc_ipc::exchange(&mut fixture.connect(), &query)
            .unwrap()
            .result
            .unwrap()["jobs"],
        json!([])
    );
    child.stop(libc::SIGTERM);
    let report = fixture.report();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("target/host-meter/integration")
        .join(std::process::id().to_string());
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("partial-native.json"), serde_json::to_vec_pretty(&json!({
        "snapshot":sample,"identity":identity,"injectedSourceFault":true,"nativeCalibration":false,
        "healthAndJobsPreserved":true,"shutdown":report,
    })).unwrap()).unwrap();
}

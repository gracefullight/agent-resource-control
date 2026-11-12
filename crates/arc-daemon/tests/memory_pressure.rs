//! Contract and production writer/transport memory-admission checks.
#![allow(dead_code)]
#[path = "../src/admission.rs"]
mod admission;
#[path = "../src/connection.rs"]
mod connection;
#[path = "../src/db_worker.rs"]
mod db_worker;
#[path = "../../arc-ipc/tests/fixtures/memory_pressure.rs"]
mod fixture;
#[path = "../src/job_meter_dispatch.rs"]
mod job_meter_dispatch;
#[path = "../src/limits.rs"]
mod limits;
#[path = "../src/meter.rs"]
mod meter;
#[path = "../src/platform.rs"]
mod platform;
#[path = "../src/runtime_metrics.rs"]
mod runtime_metrics;
#[path = "../src/transport.rs"]
mod transport;
#[path = "../src/transport_test_support.rs"]
mod transport_test_support;

#[test]
fn policy_probe_is_additive_and_missing_reading_is_an_explicit_l0_result() {
    use arc_ipc::policy::*;
    let request = capabilities_request("pressure-probe");
    let capabilities = interpret_capabilities(&request, &capabilities_response(&request)).unwrap();
    assert_eq!(capabilities.capability, CAPABILITY);
    assert_eq!(
        arc_ipc::traffic::Capabilities::supported().capability,
        "traffic.v1"
    );
    let request = current_request("missing");
    let effective =
        interpret_current(&request, &current_response(&request, &fixture::fallback())).unwrap();
    assert_eq!(effective.profile, "macos-memory-l0-v1");
    assert_eq!(effective.mode, PolicyMode::L0);
    assert_eq!(effective.reason, PolicyReason::ObservationMissing);
    assert_eq!(effective.observation.pressure, None);
}

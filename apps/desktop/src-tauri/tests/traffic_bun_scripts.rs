//! Real tools + production daemon + registered production Tauri handlers.
//! MockRuntime supplies only the shell; these tests do not open or inspect UI.
#[cfg(not(target_os = "macos"))]
#[test]
fn required_native_platform() {
    panic!("Bun-script native acceptance requires macOS");
}
#[cfg(target_os = "macos")]
#[allow(dead_code)]
#[path = "../../../../crates/arc-daemon/tests/traffic_cli/process_cleanup.rs"]
mod cleanup;
#[cfg(target_os = "macos")]
#[allow(dead_code)]
#[path = "../../../../crates/arc-daemon/tests/support/native_fixture.rs"]
mod native;
#[cfg(target_os = "macos")]
#[path = "daemon_bridge_support/meter_router.rs"]
mod router;
#[cfg(target_os = "macos")]
#[path = "../../../../crates/arc-daemon/tests/support/mod.rs"]
mod serialization;
#[cfg(target_os = "macos")]
#[allow(dead_code)]
#[path = "../../../../crates/arc-daemon/tests/traffic_bun_scripts/mod.rs"]
mod suite;
#[cfg(target_os = "macos")]
fn registered(
    socket: &std::path::Path,
    command: &str,
    args: serde_json::Value,
) -> serde_json::Value {
    router::Router::new(arc_desktop::bridge::DesktopBridge::from_socket_path(
        socket.to_owned(),
    ))
    .call(command, args)
    .unwrap()["data"]
        .clone()
}
#[cfg(target_os = "macos")]
#[test]
fn registered_bridge_real_tools_outputs() {
    let _s = serialization::serial();
    suite::outputs::run(registered);
}
#[cfg(target_os = "macos")]
#[test]
fn registered_bridge_cargo_first_one_build_slot() {
    let _s = serialization::serial();
    suite::contention::builds(registered, true);
}
#[cfg(target_os = "macos")]
#[test]
fn registered_bridge_bun_script_first_one_build_slot() {
    let _s = serialization::serial();
    suite::contention::builds(registered, false);
}
#[cfg(target_os = "macos")]
#[test]
fn registered_bridge_two_typechecks_one_typecheck_slot() {
    let _s = serialization::serial();
    suite::contention::typechecks(registered);
}
#[cfg(target_os = "macos")]
#[test]
fn registered_bridge_mixed_cpu_budget() {
    let _s = serialization::serial();
    suite::contention::cpu(registered);
}
#[cfg(target_os = "macos")]
#[test]
fn registered_bridge_mixed_memory_budget_injected_estimate() {
    let _s = serialization::serial();
    suite::contention::memory(registered);
}
#[cfg(target_os = "macos")]
#[test]
fn registered_bridge_production_recovery() {
    let _s = serialization::serial();
    suite::recovery::run(registered);
}

#[cfg(target_os = "macos")]
#[test]
fn registered_bridge_invocation_lifecycle_stdio_exit17() {
    let _s = serialization::serial();
    suite::semantics::invocation(registered);
}
#[cfg(target_os = "macos")]
#[test]
fn registered_bridge_native_signal_and_running_cancellation() {
    let _s = serialization::serial();
    suite::semantics::signals(registered);
}
#[cfg(target_os = "macos")]
#[test]
fn registered_bridge_nested_script_meter_lineage() {
    let _s = serialization::serial();
    suite::semantics::nested(registered);
}
#[cfg(target_os = "macos")]
#[test]
fn registered_bridge_injected_pressure_and_governor_preservation() {
    let _s = serialization::serial();
    suite::preservation::run(registered);
}

#[cfg(target_os = "macos")]
#[test]
fn registered_bridge_lost_enqueue_start_finish_acknowledgments() {
    let _s = serialization::serial();
    suite::protocol::acknowledgments(registered);
}
#[cfg(target_os = "macos")]
#[test]
fn native_cancellation_before_enqueue() {
    let _s = serialization::serial();
    suite::protocol::before_enqueue_cancel(registered);
}

#[cfg(target_os = "macos")]
#[test]
fn registered_bridge_repository_desktop_build_typecheck_lint() {
    let _s = serialization::serial();
    suite::repository::run(registered);
}
#[cfg(target_os = "macos")]
#[test]
fn native_old_peer_and_absolute_hung_outage_bound() {
    let _s = serialization::serial();
    suite::compatibility::old_peer_and_outage(registered);
}
#[cfg(target_os = "macos")]
#[test]
fn registered_bridge_peer_replacement_retains_job_identity() {
    let _s = serialization::serial();
    suite::compatibility::replacement(registered);
}

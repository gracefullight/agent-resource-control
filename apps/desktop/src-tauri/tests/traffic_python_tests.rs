//! Real offline Python/Cargo/Bun, production daemon and registered Tauri reads.
//! MockRuntime supplies the handler shell only; no GUI or browser is exercised.
#[cfg(not(target_os = "macos"))]
#[test]
fn required_native_platform() {
    panic!("Python TEST acceptance requires macOS and installed Python/pytest/Cargo/Bun");
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
#[path = "../../../../crates/arc-daemon/tests/traffic_python_tests/mod.rs"]
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
fn registered_python_execution_semantics() {
    let _serial = serialization::serial();
    suite::semantics(registered);
}
#[cfg(target_os = "macos")]
#[test]
fn registered_python_lifecycle_and_compatibility() {
    let _serial = serialization::serial();
    suite::lifecycle(registered);
}
#[cfg(target_os = "macos")]
#[test]
fn python_then_cargo_one_test_slot() {
    let _serial = serialization::serial();
    suite::one_test_slot(registered, suite::Peer::Cargo, true);
}
#[cfg(target_os = "macos")]
#[test]
fn cargo_then_python_one_test_slot() {
    let _serial = serialization::serial();
    suite::one_test_slot(registered, suite::Peer::Cargo, false);
}
#[cfg(target_os = "macos")]
#[test]
fn python_then_bun_one_test_slot() {
    let _serial = serialization::serial();
    suite::one_test_slot(registered, suite::Peer::Bun, true);
}
#[cfg(target_os = "macos")]
#[test]
fn bun_then_python_one_test_slot() {
    let _serial = serialization::serial();
    suite::one_test_slot(registered, suite::Peer::Bun, false);
}

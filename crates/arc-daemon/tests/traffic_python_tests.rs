//! Required macOS acceptance: real offline Python/pytest and mixed TEST runners.
#[cfg(not(target_os = "macos"))]
#[test]
fn required_native_platform() {
    panic!("Python TEST acceptance requires macOS and installed Python/pytest/Cargo/Bun");
}
#[cfg(target_os = "macos")]
#[allow(dead_code)]
#[path = "traffic_cli/process_cleanup.rs"]
mod cleanup;
#[cfg(target_os = "macos")]
#[allow(dead_code)]
#[path = "support/native_fixture.rs"]
mod native;
#[cfg(target_os = "macos")]
#[path = "support/mod.rs"]
mod serialization;
#[cfg(target_os = "macos")]
#[allow(dead_code)]
#[path = "traffic_python_tests/mod.rs"]
mod suite;

#[cfg(target_os = "macos")]
#[test]
fn native_python_execution_semantics() {
    let _serial = serialization::serial();
    suite::semantics(suite::direct_read);
}
#[cfg(target_os = "macos")]
#[test]
fn native_python_lifecycle_and_compatibility() {
    let _serial = serialization::serial();
    suite::lifecycle(suite::direct_read);
}
#[cfg(target_os = "macos")]
#[test]
fn python_then_cargo_one_test_slot() {
    let _serial = serialization::serial();
    suite::one_test_slot(suite::direct_read, suite::Peer::Cargo, true);
}
#[cfg(target_os = "macos")]
#[test]
fn cargo_then_python_one_test_slot() {
    let _serial = serialization::serial();
    suite::one_test_slot(suite::direct_read, suite::Peer::Cargo, false);
}
#[cfg(target_os = "macos")]
#[test]
fn python_then_bun_one_test_slot() {
    let _serial = serialization::serial();
    suite::one_test_slot(suite::direct_read, suite::Peer::Bun, true);
}
#[cfg(target_os = "macos")]
#[test]
fn bun_then_python_one_test_slot() {
    let _serial = serialization::serial();
    suite::one_test_slot(suite::direct_read, suite::Peer::Bun, false);
}

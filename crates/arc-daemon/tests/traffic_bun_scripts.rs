//! Required macOS real-tool acceptance. Missing prerequisites fail, never skip.
#[cfg(not(target_os = "macos"))]
#[test]
fn required_native_platform() {
    panic!("Bun-script native acceptance requires macOS and the pinned installed tools");
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
#[path = "traffic_bun_scripts/mod.rs"]
mod suite;

#[cfg(target_os = "macos")]
#[test]
fn real_installed_tools_and_original_bun_scripts() {
    let _serial = serialization::serial();
    suite::outputs::run(suite::direct_read);
}
#[cfg(target_os = "macos")]
#[test]
fn production_script_recovery_and_absolute_outage_bound() {
    let _serial = serialization::serial();
    suite::recovery::run(suite::direct_read);
}

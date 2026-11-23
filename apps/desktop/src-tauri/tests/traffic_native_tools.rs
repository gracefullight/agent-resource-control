//! Same real native scenarios through the actual registered Router. No GUI interaction.
#[cfg(not(target_os = "macos"))]
#[test]
fn required_native_platform() {
    panic!("native-tool-families acceptance requires macOS");
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
#[allow(dead_code, unused_imports)]
#[path = "../../../../crates/arc-daemon/tests/traffic_native_tools/mod.rs"]
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
macro_rules! family {
    ($semantics:ident, $pairs:ident, $family:ident) => {
        #[test]
        fn $semantics() {
            suite::$family::prewarm();
            let _serial = serialization::serial();
            suite::$family::semantics(registered);
        }
        #[test]
        fn $pairs() {
            suite::$family::prewarm();
            let _serial = serialization::serial();
            suite::$family::pairs(registered);
        }
    };
}
#[cfg(target_os = "macos")]
family!(
    registered_go_execution_semantics,
    registered_go_ordered_pairs,
    go
);
#[cfg(target_os = "macos")]
family!(
    registered_swift_execution_semantics,
    registered_swift_ordered_pairs,
    swift
);
#[cfg(target_os = "macos")]
#[test]
fn registered_synthetic_swift_stream_forwarding() {
    let _serial = serialization::serial();
    suite::swift::stream_forwarding(registered);
}
#[cfg(target_os = "macos")]
family!(
    registered_cmake_execution_semantics,
    registered_cmake_ordered_pairs,
    cmake
);

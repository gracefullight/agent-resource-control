//! Required macOS acceptance; compile-only until all family implementations arrive.
#[cfg(not(target_os = "macos"))]
#[test]
fn required_native_platform() {
    panic!("native-tool-families acceptance requires macOS");
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
#[allow(dead_code, unused_imports)]
#[path = "traffic_native_tools/mod.rs"]
mod suite;

#[cfg(target_os = "macos")]
macro_rules! family {
    ($semantics:ident, $pairs:ident, $family:ident) => {
        #[test]
        fn $semantics() {
            suite::$family::prewarm();
            let _serial = serialization::serial();
            suite::$family::semantics(suite::direct_read);
        }
        #[test]
        fn $pairs() {
            suite::$family::prewarm();
            let _serial = serialization::serial();
            suite::$family::pairs(suite::direct_read);
        }
    };
}
#[cfg(target_os = "macos")]
family!(go_execution_semantics, go_ordered_pairs, go);
#[cfg(target_os = "macos")]
family!(swift_execution_semantics, swift_ordered_pairs, swift);
#[cfg(target_os = "macos")]
#[test]
fn synthetic_swift_stream_forwarding() {
    let _serial = serialization::serial();
    suite::swift::stream_forwarding(suite::direct_read);
}
#[cfg(target_os = "macos")]
family!(cmake_execution_semantics, cmake_ordered_pairs, cmake);

#[cfg(target_os = "macos")]
#[test]
fn go_seed_preparation_does_not_wait_for_the_daemon_fixture_lock() {
    use std::{sync::mpsc, time::Duration};

    let serial = serialization::serial();
    let (done, completion) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        suite::go::prewarm();
        done.send(()).unwrap();
    });
    let result = completion.recv_timeout(Duration::from_secs(300));
    // Release even on failure so an accidentally blocked worker can finish.
    drop(serial);
    worker.join().unwrap();
    result.expect("Go seed preparation waited for the daemon fixture lock");
}

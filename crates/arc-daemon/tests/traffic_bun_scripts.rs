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
fn agent_hooks_claude_first_one_build_slot() {
    let _serial = serialization::serial();
    suite::agent_hooks::run_isolated("agent_hooks_claude_first_worker");
}

#[cfg(target_os = "macos")]
#[test]
fn agent_hooks_cursor_first_one_build_slot() {
    let _serial = serialization::serial();
    suite::agent_hooks::run_isolated("agent_hooks_cursor_first_worker");
}

#[cfg(target_os = "macos")]
#[test]
fn agent_hooks_claude_first_worker() {
    suite::agent_hooks::run_worker(suite::direct_read, true);
}

#[cfg(target_os = "macos")]
#[test]
fn agent_hooks_cursor_first_worker() {
    suite::agent_hooks::run_worker(suite::direct_read, false);
}

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

#[cfg(target_os = "macos")]
#[test]
fn native_case_retains_only_its_own_failure_and_reaps_daemon() {
    use arc_core::process_identity::{probe_live, IdentityStatus, ProcessIdentity};
    use std::{fs, panic::AssertUnwindSafe};

    let _serial = serialization::serial();
    let failure = suite::Case::new("expected-unwind", suite::direct_read, false);
    let failure_artifacts = failure.artifacts.clone();
    let failure_runtime = failure.fixture.root.clone();
    let failure_shims = failure.tools.shims.clone();
    let daemon = ProcessIdentity::capture(failure.daemon.child.id()).unwrap();
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let _case = failure;
        panic!("intentional fixture failure");
    }));
    assert!(result.is_err());
    assert!(failure_artifacts.is_dir());
    assert!(failure_runtime.is_dir());
    assert!(failure_shims.is_dir());
    assert!(matches!(
        probe_live(&daemon),
        IdentityStatus::Missing | IdentityStatus::Mismatch
    ));

    let success = suite::Case::new("independent-success", suite::direct_read, false);
    let success_artifacts = success.artifacts.clone();
    let success_runtime = success.fixture.root.clone();
    let success_shims = success.tools.shims.clone();
    success.done();
    assert!(!success_artifacts.exists());
    assert!(!success_runtime.exists());
    assert!(!success_shims.exists());
    assert!(failure_artifacts.is_dir());
    assert!(failure_runtime.is_dir());
    assert!(failure_shims.is_dir());

    fs::remove_dir_all(failure_artifacts).unwrap();
    fs::remove_dir_all(failure_runtime).unwrap();
    fs::remove_dir_all(failure_shims).unwrap();
}

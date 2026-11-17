//! Active macOS accounting acceptance: real private daemon, SQLite, CLI and
//! registered production desktop dispatch. No display or personal data.
#![cfg(target_os = "macos")]
#[path = "job_meter_acceptance/calibration.rs"]
mod calibration;
#[allow(dead_code, unused_imports)]
mod daemon_bridge_support;
#[allow(dead_code)]
#[path = "../../../../crates/arc-daemon/tests/support/native_exec.rs"]
mod execution;
#[path = "../../../../crates/arc-daemon/tests/support/legacy_preparation.rs"]
mod legacy_preparation;
#[path = "job_meter_acceptance/lifecycle.rs"]
mod lifecycle;
#[path = "daemon_bridge_support/meter_router.rs"]
mod meter_router;
#[allow(dead_code)]
#[path = "../../../../crates/arc-daemon/tests/support/native_fixture.rs"]
mod native;
#[path = "job_meter_acceptance/queue.rs"]
mod queue;
#[path = "job_meter_acceptance/recovery.rs"]
mod recovery;
#[allow(dead_code)]
#[path = "../../../../crates/arc-daemon/tests/support/mod.rs"]
mod serialization;
#[path = "job_meter_acceptance/support.rs"]
mod support;

#[test]
fn daemon_evidence_is_owned_by_each_successful_or_failed_case() {
    use arc_core::process_identity::{probe_live, IdentityStatus};
    use std::{fs, panic::AssertUnwindSafe};

    let _serial = serialization::serial();
    let mut success = daemon_bridge_support::RealDaemon::new("cleanup-success");
    let success_root = success.fixture.root.clone();
    assert!(success.evidence.starts_with(&success_root));
    success.stop("completed");
    drop(success);
    assert!(!success_root.exists());

    let failure = daemon_bridge_support::RealDaemon::new("cleanup-failure");
    let failure_root = failure.fixture.root.clone();
    let failure_evidence = failure.evidence.clone();
    let daemon = failure.identity();
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let _case = failure;
        panic!("intentional daemon fixture failure");
    }));
    assert!(result.is_err());
    assert!(failure_root.is_dir());
    assert!(failure_evidence.is_dir());
    assert!(matches!(
        probe_live(&daemon),
        IdentityStatus::Missing | IdentityStatus::Mismatch
    ));
    fs::remove_dir_all(failure_root).unwrap();
}

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
#[path = "job_meter_acceptance/lifecycle.rs"]
mod lifecycle;
#[path = "daemon_bridge_support/meter_router.rs"]
mod meter_router;
#[allow(dead_code)]
#[path = "../../../../crates/arc-daemon/tests/support/native_fixture.rs"]
mod native;
#[path = "job_meter_acceptance/recovery.rs"]
mod recovery;
#[allow(dead_code)]
#[path = "../../../../crates/arc-daemon/tests/support/mod.rs"]
mod serialization;
#[path = "job_meter_acceptance/support.rs"]
mod support;

//! macOS production daemon + arcctl + registered Tauri router acceptance.
//! Only pressure and child barriers are injected. No installed daemon is used.
#![cfg(target_os = "macos")]
#[path = "governor/backlog.rs"]
mod backlog;
#[path = "governor/control.rs"]
mod control;
#[allow(dead_code)]
#[path = "../../../../crates/arc-daemon/tests/support/native_exec.rs"]
mod execution;
#[allow(dead_code)]
#[path = "../../../../crates/arc-daemon/tests/support/native_fixture.rs"]
mod native;
#[allow(dead_code)]
#[path = "memory_pressure/native_support.rs"]
mod native_support;
#[path = "governor/races.rs"]
mod races;
#[path = "daemon_bridge_support/meter_router.rs"]
mod router;
#[path = "governor/sequence.rs"]
mod sequence;
#[path = "../../../../crates/arc-daemon/tests/support/mod.rs"]
mod serialization;
#[path = "governor/support.rs"]
mod support;

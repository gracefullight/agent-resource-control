//! Non-desktop checks for the shared compatibility preparation and lease.
#![cfg(target_os = "macos")]
#[allow(dead_code)]
#[path = "support/legacy_preparation.rs"]
mod legacy_preparation;
#[allow(dead_code)]
#[path = "support/native_fixture.rs"]
mod native;

//! Explicit module root for Cargo and the maintained source inventory.
#[cfg(target_os = "macos")]
mod bun_semantics;
mod compatibility;
mod diagnostics;
pub(super) mod execution;

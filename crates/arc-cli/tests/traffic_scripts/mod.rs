//! Explicit module root for Cargo and the maintained source inventory.
#[cfg(target_os = "macos")]
mod bun_semantics;
mod compatibility;
mod diagnostics;
mod endpoint_compatibility;
pub(super) mod execution;

//! Offline real Go invocations; the shared harness owns admission and reconciliation.
use super::Read;
mod cleanup;
mod direct;
mod execution;
pub(super) mod owned;
mod projects;

/// Build the immutable Go seed before acquiring the single-daemon fixture lock.
pub fn prewarm() {
    static READY: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    READY.get_or_init(|| drop(projects::Tools::installed()));
}

pub fn semantics(read: Read) {
    execution::semantics(read);
}
pub fn pairs(read: Read) {
    execution::pairs(read);
}

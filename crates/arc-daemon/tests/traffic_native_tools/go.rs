//! Offline real Go invocations; the shared harness owns admission and reconciliation.
use super::Read;
mod cleanup;
mod direct;
mod execution;
mod owned;
mod projects;

pub fn semantics(read: Read) {
    execution::semantics(read);
}
pub fn pairs(read: Read) {
    execution::pairs(read);
}

//! One CLI-owned native child, independent of every metadata retry.
mod accounting;
mod capture;
mod ci_context;
mod descriptors;
mod endpoint_context;
mod entry;
mod governor;
mod launch;
mod lifecycle;
mod lineage;
mod metadata;
mod native_environment;
pub(crate) mod policy;
mod preparation;
mod run;
mod signals;
mod terminal;
mod traffic;
pub(crate) mod transport;
mod witness;
#[allow(unused_imports)] // capture_shell is consumed by the next-wave classify route.
pub(crate) use capture::{capture_argv, capture_shell};
#[cfg(test)]
use entry::run_with_clocks;
pub(crate) use entry::{run, run_hook, run_with_environment};
use lifecycle::start_until;
pub(crate) use preparation::invalid;

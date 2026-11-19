//! One CLI-owned native child, independent of every metadata retry.
mod accounting;
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
pub(crate) use entry::{run, run_hook, run_with_environment};
use lifecycle::start_until;
pub(crate) use preparation::invalid;
#[cfg(test)]
use run::run_with_clocks;

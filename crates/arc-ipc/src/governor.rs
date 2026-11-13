//! Additive, owner-authenticated protocol-1 Governor contracts.
//!
//! These types register no handlers. Authentication is the existing local
//! transport's same-user check; job operations also verify peer process identity.
//! A daemon advertises this capability only with the complete durable setter
//! and current-client execution route. Legacy capability/result shapes stay intact.
mod execution;
mod switch;
mod wire;
pub use execution::*;
pub use switch::*;
pub use wire::*;

#[cfg(test)]
mod typescript;

//! Pure memory-admission contracts. No native sampling, database or IPC owner.
//!
//! Task 1 supplies validated types; task 2 implements the evaluator and adapter.
mod decision;
pub mod evaluator;
mod observation;

pub use decision::*;
pub use observation::*;

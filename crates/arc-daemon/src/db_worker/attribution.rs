//! The native process-attribution scan's read on the single writer (lane A:
//! MTR-007, AT-MTR-03, MET-027, NFR-012, R-005 item 2).
//!
//! Lane A's scanner (`process_attribution`, on the Meter cadence) reads the
//! Proven roots here: each RUNNING managed job's verified recorded child
//! identity, vendor and active lease generation. The scan reads them before
//! and after its native pass and trusts only evidence present in both. Its
//! aggregates go to the lane's interval store
//! (`arc_storage::attribution::IntervalStore`), not through this writer.
//! The read is admitted like any other operation: it holds a queue slot and
//! is counted in `DbQueued`, `DbExecuting` and `DbOutstanding`. A refused
//! admission is the scan's skipped pass, not a rejected client request, so
//! it never counts in `DbRejected`. Native reads never run here.
use super::{jobs, DbClient, DbValue};
use arc_storage::{
    attribution::{self, ManagedRootEvidence},
    jobs::JobError,
};
use std::time::Instant;

pub(crate) enum Operation {
    /// The Proven roots: each RUNNING job's verified recorded child and vendor.
    Managed,
}
pub(crate) enum Value {
    Managed(Result<Vec<ManagedRootEvidence>, JobError>),
}

/// Returns `jobs::Value::Attribution(..)`.
pub(crate) fn execute(connection: &mut rusqlite::Connection, operation: Operation) -> jobs::Value {
    jobs::Value::Attribution(match operation {
        Operation::Managed => Value::Managed(attribution::managed_roots(connection)),
    })
}

pub(crate) async fn managed(client: &DbClient, end: Instant) -> Option<Vec<ManagedRootEvidence>> {
    let ticket = client
        .admit(end, Some(jobs::Operation::Attribution(Operation::Managed)))
        .ok()?;
    match ticket.receive_value().await.ok()? {
        DbValue::Lifecycle(value) => match *value {
            jobs::Value::Attribution(Value::Managed(Ok(roots))) => Some(roots),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
#[path = "attribution/tests.rs"]
mod tests;

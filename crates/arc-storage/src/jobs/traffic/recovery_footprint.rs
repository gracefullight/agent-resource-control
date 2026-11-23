//! Bind an unavailable end reading before recovery releases ownership.
use super::{store, JobError, JobResult};
use arc_ipc::jobs::JobId;
use rusqlite::Connection;

/// Run inside the transaction that abandons a recovered job. A delayed
/// completion cannot replace this reading after a successor has been granted.
/// A terminal root of a recovered descendant is fenced as well, because the
/// descendant's recovery may release their shared permit. Prior readings win.
pub(super) fn bind(connection: &Connection, id: &JobId, entry: &store::Entry) -> JobResult<()> {
    if connection.is_autocommit() {
        return Err(JobError::InvalidTransition);
    }
    let bind_one = |job: &JobId| -> JobResult<()> {
        if !crate::footprint::record(
            connection,
            job,
            crate::footprint::Point::End,
            crate::footprint::Reading::Unavailable,
        )? {
            return Err(JobError::StorageFailure);
        }
        Ok(())
    };
    bind_one(id)?;
    if let Some(root) = entry.root.as_ref().filter(|root| *root != id) {
        if store::entry(connection, root)?.is_some_and(|root_entry| {
            matches!(
                root_entry.state.as_str(),
                "COMPLETED" | "CANCELLED" | "ABANDONED"
            )
        }) {
            bind_one(root)?;
        }
    }
    Ok(())
}

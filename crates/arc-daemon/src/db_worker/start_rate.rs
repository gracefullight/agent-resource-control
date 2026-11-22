//! Start-rate re-evaluation on the Traffic grant path (lane C, DEC-013).
//!
//! Runs after each writer operation, beside the opportunistic expiry. A
//! QUEUED entry held only by its class token bucket needs no daemon-side
//! work here: its waiter keeps polling on its existing cadence, and each
//! poll re-evaluates the bucket inside its own admission transaction with
//! fresh owner evidence (`arc_storage::jobs::traffic::start_rate::hold`), so
//! the entry is granted by the first poll after its token is due, with
//! exactly one permit. Granting from here would need owner evidence the
//! writer must not gather, so this hook deliberately changes nothing.
pub(crate) fn after_operation(connection: &mut rusqlite::Connection, now_ms: u64) {
    let _ = (connection, now_ms);
}

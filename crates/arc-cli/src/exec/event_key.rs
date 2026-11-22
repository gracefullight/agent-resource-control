//! The vendor hook event one `hook-exec` run delivers (AT-TRF-09, ADR-014).
//!
//! The installed hook writes `--arc-event <key>` directly after `hook-exec`
//! and before `--`. The option is consumed here, like `--arc-request`: the
//! child's argv, environment and streams never carry it. The key is the
//! opaque digest of `arc_ipc::traffic::hook_event`; the raw session and tool
//! call ids are never given to this process.
//!
//! A key is sent with this run's enqueue only to a daemon that advertised
//! `hook_event::CAPABILITY` by holding `hook_event::ADVERTISEMENT`, exactly
//! as `project_key` gates the project key: the gate costs no request, and a
//! protocol-1 peer or an older daemon receives the bytes it received before.
//!
//! A daemon that already recorded the key answers `DuplicateDelivery`. That
//! run is `traffic::Mode::Duplicate`: it holds no job, queue entry or permit,
//! starts its child exactly once without admission and reports nothing for
//! the existing job. ARC never suppresses a command the vendor runs.
use arc_ipc::{
    client::Continuity,
    traffic::hook_event::{EventKey, ADVERTISEMENT},
};
use std::{cell::RefCell, path::Path};

thread_local! {
    static EVENT: RefCell<Option<EventKey>> = const { RefCell::new(None) };
}

/// The `hook-exec --arc-event` value. Absent is today's route; a malformed
/// value was not written by the hook and is rejected before any child starts.
pub(super) fn parse(value: Option<String>) -> Result<Option<EventKey>, ()> {
    value
        .map(|value| EventKey::try_from(value).map_err(|_| ()))
        .transpose()
}

/// Scope one run to the event it delivers.
pub(super) fn with_event<T>(event: Option<EventKey>, run: impl FnOnce() -> T) -> T {
    struct Reset(Option<EventKey>);
    impl Drop for Reset {
        fn drop(&mut self) {
            EVENT.with(|slot| *slot.borrow_mut() = self.0.take());
        }
    }
    let previous = EVENT.with(|slot| slot.replace(event));
    let _reset = Reset(previous);
    run()
}

/// The key of the scoped run. Read on the thread that entered `with_event`.
fn scoped() -> Option<EventKey> {
    EVENT.with(|slot| slot.borrow().clone())
}

/// The key to send with this run's enqueue, or `None` to send none. `daemon`
/// is the peer that answered this run's capability probe; without the
/// advertisement held by that very process no key is sent.
pub(super) fn negotiated(socket: &Path, daemon: &Continuity) -> Option<EventKey> {
    let key = scoped()?;
    let daemon = daemon.identity()?;
    super::project_key::holds(socket, daemon.pid, ADVERTISEMENT).then_some(key)
}

#[cfg(test)]
mod tests;

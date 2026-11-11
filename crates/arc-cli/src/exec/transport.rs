//! CLI compatibility wrappers over the shared authenticated, bounded client.
pub(super) use arc_ipc::client::Continuity;
pub(crate) use arc_ipc::client::Failure;
use arc_ipc::{client::ExchangeOptions, Request};
use serde_json::Value;
use std::{
    path::Path,
    time::{Duration, Instant},
};

// Read-only status and post-spawn metadata retain their existing bounded API.
pub(crate) fn call(socket: &Path, operation: &str, payload: Value) -> Result<Value, ()> {
    call_until(
        socket,
        operation,
        payload,
        Instant::now() + Duration::from_secs(2),
        &|| false,
    )
    .map_err(|_| ())
}
pub(crate) fn call_until(
    socket: &Path,
    operation: &str,
    payload: Value,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
) -> Result<Value, Failure> {
    call_until_tracked(socket, operation, payload, deadline, cancelled, &mut false)
}

// Recent status shares one deadline across rows. A busy reply for one row
// must not consume the budget needed to inspect the remaining rows.
pub(crate) fn call_until_no_busy_retry(
    socket: &Path,
    operation: &str,
    payload: Value,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
) -> Result<Value, Failure> {
    let mut request = Request::new(crate::request_id(), operation);
    request.payload = payload;
    arc_ipc::client::exchange_until(
        socket,
        &request,
        deadline,
        cancelled,
        &mut false,
        ExchangeOptions {
            continuity: None,
            retry_busy: false,
        },
    )
}

pub(super) fn call_until_tracked(
    socket: &Path,
    operation: &str,
    payload: Value,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
    possibly_committed: &mut bool,
) -> Result<Value, Failure> {
    call_until_with_continuity(
        socket,
        operation,
        payload,
        deadline,
        cancelled,
        possibly_committed,
        None,
    )
}

pub(super) fn call_until_with_continuity(
    socket: &Path,
    operation: &str,
    payload: Value,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
    possibly_committed: &mut bool,
    continuity: Option<&Continuity>,
) -> Result<Value, Failure> {
    let mut request = Request::new(crate::request_id(), operation);
    request.payload = payload;
    arc_ipc::client::exchange_until(
        socket,
        &request,
        deadline,
        cancelled,
        possibly_committed,
        ExchangeOptions {
            continuity,
            retry_busy: true,
        },
    )
}

#[cfg(test)]
mod historical_integrity;
#[cfg(test)]
mod historical_tests;
#[cfg(test)]
mod preparation_tests;

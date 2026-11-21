//! CLI compatibility wrappers over the shared authenticated, bounded client.
pub(super) use arc_ipc::client::Continuity;
pub(crate) use arc_ipc::client::Failure;
use arc_ipc::{client::ExchangeOptions, Request};
use serde_json::Value;
use std::{
    path::Path,
    time::{Duration, Instant},
};
mod negotiated;
mod relay_proof;
pub(super) fn relay_scope() -> relay_proof::Scope {
    relay_proof::Scope::enter()
}
pub(crate) use negotiated::negotiated;

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

// The recent-status fixture supplies its own clock, including to the initial
// request. Production passes Instant::now and retains the same two-second cap.
pub(crate) fn call_with_clock(
    socket: &Path,
    operation: &str,
    payload: Value,
    now: &dyn Fn() -> Instant,
) -> Result<Value, ()> {
    let mut request = Request::new(crate::request_id(), operation);
    request.payload = payload;
    arc_ipc::client::exchange_until_with_clock(
        socket,
        &request,
        now() + Duration::from_secs(2),
        &|| false,
        &mut false,
        ExchangeOptions {
            continuity: None,
            retry_busy: true,
        },
        now,
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
    call_until_no_busy_retry_with_clock(
        socket,
        operation,
        payload,
        deadline,
        cancelled,
        &Instant::now,
    )
}

pub(crate) fn call_until_no_busy_retry_with_clock(
    socket: &Path,
    operation: &str,
    payload: Value,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
    now: &dyn Fn() -> Instant,
) -> Result<Value, Failure> {
    let mut request = Request::new(crate::request_id(), operation);
    request.payload = payload;
    arc_ipc::client::exchange_until_with_clock(
        socket,
        &request,
        deadline,
        cancelled,
        &mut false,
        ExchangeOptions {
            continuity: None,
            retry_busy: false,
        },
        now,
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
    #[cfg(test)]
    if let Some(result) =
        preparation_tests::scripted_exchange(operation, &payload, deadline, cancelled)
    {
        // Match negotiated I/O: a sent native admission with DB_BUSY may have
        // committed even when the test transport returns its reply directly.
        if operation == arc_ipc::traffic::native::OPERATION_JOB
            && matches!(result, Err(Failure::Busy))
        {
            *possibly_committed = true;
        }
        return result;
    }
    let mut request = Request::new(crate::request_id(), operation);
    request.payload = payload;
    // Legacy lifecycle messages may pass through a byte-forwarding local
    // relay. The callback proves the exact request while preserving that wire
    // route. Direct daemon connections remain usable if setup is unavailable.
    if matches!(
        operation,
        arc_ipc::jobs::OPERATION_JOB_START
            | arc_ipc::jobs::OPERATION_JOB_START_LOCALITY
            | arc_ipc::jobs::OPERATION_JOB_COMPLETE
    ) {
        let _ = relay_proof::bind(&mut request);
    }
    if matches!(
        operation,
        arc_ipc::jobs::OPERATION_JOB_START_LOCALITY
            | arc_ipc::traffic::OPERATION_BUN_SCRIPTS_CAPABILITIES
            | arc_ipc::traffic::OPERATION_BUN_SCRIPTS_JOB
            | arc_ipc::traffic::OPERATION_PYTHON_TESTS_CAPABILITIES
            | arc_ipc::traffic::OPERATION_PYTHON_TESTS_JOB
            | arc_ipc::traffic::native::OPERATION_CAPABILITIES
            | arc_ipc::traffic::native::OPERATION_JOB
            | arc_ipc::traffic::endpoint::OPERATION_CAPABILITIES
            | arc_ipc::traffic::endpoint::OPERATION_JOB
    ) {
        return negotiated(
            socket,
            request,
            deadline,
            cancelled,
            possibly_committed,
            continuity,
        );
    }
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

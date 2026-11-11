use super::*;
pub(super) fn call_until(
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
pub(super) fn call_until_no_busy_retry(
    socket: &Path,
    operation: &str,
    payload: Value,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
) -> Result<Value, Failure> {
    call_with_continuity(
        operation,
        payload,
        deadline,
        cancelled,
        &mut false,
        ExchangeOptions {
            continuity: None,
            retry_busy: false,
        },
        |attempt_deadline| DeadlineStream::connect(socket, attempt_deadline, cancelled),
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

fn call_until_with_continuity(
    socket: &Path,
    operation: &str,
    payload: Value,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
    possibly_committed: &mut bool,
    continuity: Option<&Continuity>,
) -> Result<Value, Failure> {
    call_with_continuity(
        operation,
        payload,
        deadline,
        cancelled,
        possibly_committed,
        ExchangeOptions {
            continuity,
            retry_busy: true,
        },
        |attempt_deadline| DeadlineStream::connect(socket, attempt_deadline, cancelled),
    )
}

#[cfg(test)]
pub(super) fn call_with<S: TrackedStream>(
    operation: &str,
    payload: Value,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
    connect: impl FnMut(Instant) -> Result<S, ConnectFailure>,
) -> Result<Value, Failure> {
    call_with_tracked(operation, payload, deadline, cancelled, &mut false, connect)
}

#[cfg(test)]
fn call_with_tracked<S: TrackedStream>(
    operation: &str,
    payload: Value,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
    possibly_committed: &mut bool,
    connect: impl FnMut(Instant) -> Result<S, ConnectFailure>,
) -> Result<Value, Failure> {
    call_with_continuity(
        operation,
        payload,
        deadline,
        cancelled,
        possibly_committed,
        ExchangeOptions {
            continuity: None,
            retry_busy: true,
        },
        connect,
    )
}

fn call_with_continuity<S: TrackedStream>(
    operation: &str,
    payload: Value,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
    possibly_committed: &mut bool,
    options: ExchangeOptions<'_>,
    connect: impl FnMut(Instant) -> Result<S, ConnectFailure>,
) -> Result<Value, Failure> {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let mut request = Request::new(
        format!(
            "shared-test-{}",
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ),
        operation,
    );
    request.payload = payload;
    exchange_with(
        &request,
        deadline,
        cancelled,
        possibly_committed,
        options,
        connect,
    )
}

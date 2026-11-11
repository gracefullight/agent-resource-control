//! Deterministic stage and retry controls; native EACCES is covered separately.
use super::{call_with, endpoint_failure, local_failure, ConnectFailure, Failure, TrackedStream};
use crate::{Request, Response};
use serde_json::Value;
use std::{
    cell::Cell,
    io::{self, Cursor, Read, Write},
    time::{Duration, Instant},
};

struct Stream {
    written: Vec<u8>,
    response: Option<Cursor<Vec<u8>>>,
    reject: bool,
}
impl Stream {
    fn disconnected() -> Self {
        Self {
            written: Vec::new(),
            response: None,
            reject: false,
        }
    }
    fn rejecting() -> Self {
        Self {
            reject: true,
            ..Self::disconnected()
        }
    }
}
impl Read for Stream {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if !self.reject {
            return Ok(0);
        }
        if self.response.is_none() {
            let request: Request = crate::read_message(&mut Cursor::new(&self.written)).unwrap();
            let mut bytes = Vec::new();
            crate::write_response(
                &mut bytes,
                &Response::error(request.request_id, "AUTHENTICATION_FAILED"),
            )
            .unwrap();
            self.response = Some(Cursor::new(bytes));
        }
        self.response.as_mut().unwrap().read(output)
    }
}
impl Write for Stream {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.written.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl TrackedStream for Stream {
    fn sent(&self) -> bool {
        !self.written.is_empty()
    }
}
fn denied() -> ConnectFailure {
    endpoint_failure(io::Error::from_raw_os_error(libc::EACCES))
}
fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(1)
}

#[test]
fn invalid_socket_paths_and_local_resource_errors_do_not_launch() {
    // Both the direct connect error and deferred SO_ERROR use this classifier.
    for errno in [
        libc::ENOTDIR,
        libc::ENOTSOCK,
        libc::ENAMETOOLONG,
        libc::ELOOP,
        libc::EMFILE,
        libc::ENFILE,
        libc::ENOMEM,
        libc::ENOBUFS,
        libc::EINVAL,
        libc::EPROTOTYPE,
        libc::EAFNOSUPPORT,
        libc::EBADF,
        libc::EFAULT,
    ] {
        let classified = endpoint_failure(io::Error::from_raw_os_error(errno));
        let expected = if matches!(
            errno,
            libc::ENOTDIR | libc::ENOTSOCK | libc::ELOOP | libc::ENAMETOOLONG | libc::EPROTOTYPE
        ) {
            ConnectFailure::InvalidEndpoint
        } else {
            ConnectFailure::Local
        };
        assert_eq!(classified, expected, "errno {errno}");
        let attempts = Cell::new(0);
        let result = call_with("REGISTER", Value::Null, deadline(), &|| false, |_| {
            attempts.set(attempts.get() + 1);
            Err::<Stream, _>(classified)
        });
        assert_eq!(attempts.get(), 1, "errno {errno}");
        assert_eq!(
            result,
            Err(if expected == ConnectFailure::InvalidEndpoint {
                Failure::InvalidEndpoint
            } else {
                Failure::Local
            }),
            "errno {errno}"
        );
    }
}

#[test]
fn endpoint_outages_remain_retryable() {
    for errno in [
        libc::EACCES,
        libc::EPERM,
        libc::ENOENT,
        libc::ECONNREFUSED,
        libc::ETIMEDOUT,
        libc::EHOSTUNREACH,
        libc::EHOSTDOWN,
        libc::ENETUNREACH,
        libc::ENETDOWN,
        libc::ENETRESET,
        libc::ECONNRESET,
        libc::ECONNABORTED,
        libc::EPIPE,
        libc::ENOTCONN,
        libc::EAGAIN,
        libc::EINTR,
        libc::EINPROGRESS,
        libc::EALREADY,
    ] {
        let classified = endpoint_failure(io::Error::from_raw_os_error(errno));
        assert_eq!(classified, ConnectFailure::Endpoint, "errno {errno}");
    }
}

#[test]
fn initial_endpoint_access_denial_is_unavailable_but_local_setup_is_not() {
    assert_eq!(denied(), ConnectFailure::Endpoint);
    assert_eq!(
        endpoint_failure(io::Error::from_raw_os_error(libc::EPERM)),
        ConnectFailure::Endpoint
    );
    assert_eq!(
        local_failure(io::Error::from_raw_os_error(libc::EACCES)),
        ConnectFailure::Local
    );
    assert_eq!(
        local_failure(io::Error::from_raw_os_error(libc::EPERM)),
        ConnectFailure::Local
    );
    assert_eq!(
        endpoint_failure(io::Error::from_raw_os_error(libc::EINVAL)),
        ConnectFailure::Local
    );
    let attempts = Cell::new(0);
    let result = call_with("REGISTER", Value::Null, deadline(), &|| false, |_| {
        attempts.set(attempts.get() + 1);
        Err::<Stream, _>(denied())
    });
    assert_eq!(attempts.get(), 3);
    assert_eq!(result, Err(Failure::Unavailable));
    // A PermissionDenied from descriptor setup is classified by its stage.
    assert_eq!(
        call_with("REGISTER", Value::Null, deadline(), &|| false, |_| Err::<
            Stream,
            _,
        >(
            ConnectFailure::Local
        )),
        Err(Failure::Local)
    );
}

#[test]
fn sent_request_then_access_denials_stays_uncertain() {
    let attempts = Cell::new(0);
    let result = call_with("REGISTER", Value::Null, deadline(), &|| false, |_| {
        attempts.set(attempts.get() + 1);
        if attempts.get() == 1 {
            Ok(Stream::disconnected())
        } else {
            Err(denied())
        }
    });
    assert_eq!(attempts.get(), 3);
    assert_eq!(result, Err(Failure::Uncertain));
}

#[test]
fn cancellation_and_authenticated_rejection_remain_nonlaunching() {
    let cancelled = Cell::new(false);
    let attempts = Cell::new(0);
    let result = call_with(
        "REGISTER",
        Value::Null,
        deadline(),
        &|| cancelled.get(),
        |_| {
            attempts.set(attempts.get() + 1);
            if attempts.get() == 1 {
                Ok(Stream::disconnected())
            } else {
                cancelled.set(true);
                Err(denied())
            }
        },
    );
    assert_eq!(attempts.get(), 2);
    assert_eq!(result, Err(Failure::Cancelled));
    let cancelled = Cell::new(false);
    let result = call_with(
        "REGISTER",
        Value::Null,
        deadline(),
        &|| cancelled.get(),
        |_| {
            cancelled.set(true);
            Err::<Stream, _>(endpoint_failure(io::Error::from_raw_os_error(
                libc::ENOTDIR,
            )))
        },
    );
    assert_eq!(result, Err(Failure::Cancelled));
    assert_eq!(
        call_with("REGISTER", Value::Null, deadline(), &|| false, |_| Ok(
            Stream::rejecting()
        )),
        Err(Failure::Rejected)
    );
}

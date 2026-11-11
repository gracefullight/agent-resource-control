use std::io;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ConnectFailure {
    Endpoint,
    InvalidEndpoint,
    Local,
}

// The same classifier handles direct connect and deferred SO_ERROR. Only
// recognized endpoint failures permit fallback; unknown errors stay local.
// Descriptor creation and setup always use local_failure instead.
pub(super) fn endpoint_failure(error: io::Error) -> ConnectFailure {
    if matches!(
        error.raw_os_error(),
        Some(libc::ENOTDIR | libc::ENOTSOCK | libc::ELOOP | libc::ENAMETOOLONG | libc::EPROTOTYPE)
    ) {
        return ConnectFailure::InvalidEndpoint;
    }
    if matches!(
        error.raw_os_error(),
        Some(
            libc::EACCES
                | libc::EPERM
                | libc::ENOENT
                | libc::ECONNREFUSED
                | libc::ETIMEDOUT
                | libc::EHOSTUNREACH
                | libc::EHOSTDOWN
                | libc::ENETUNREACH
                | libc::ENETDOWN
                | libc::ENETRESET
                | libc::ECONNRESET
                | libc::ECONNABORTED
                | libc::EPIPE
                | libc::ENOTCONN
                | libc::EAGAIN
                | libc::EINTR
                | libc::EINPROGRESS
                | libc::EALREADY
        )
    ) {
        ConnectFailure::Endpoint
    } else {
        ConnectFailure::Local
    }
}

pub(super) fn local_failure(_: io::Error) -> ConnectFailure {
    ConnectFailure::Local
}

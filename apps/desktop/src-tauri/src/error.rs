//! Static desktop failures. Peer payloads and OS diagnostics never cross IPC.
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Unavailable,
    Uncertain,
    Busy,
    Rejected,
    Protocol,
    InvalidReply,
    InvalidRequest,
    InvalidEndpoint,
    Local,
    Cancelled,
    Unsupported,
    OpenerFailed,
    WindowUnavailable,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct DesktopError {
    pub code: ErrorCode,
    pub retryable: bool,
}

impl DesktopError {
    pub const fn new(code: ErrorCode) -> Self {
        Self {
            code,
            retryable: matches!(
                code,
                ErrorCode::Unavailable | ErrorCode::Uncertain | ErrorCode::Busy
            ),
        }
    }
}

#[cfg(unix)]
impl From<arc_ipc::client::Failure> for DesktopError {
    fn from(failure: arc_ipc::client::Failure) -> Self {
        use arc_ipc::client::Failure as F;
        Self::new(match failure {
            F::Unavailable => ErrorCode::Unavailable,
            F::Uncertain => ErrorCode::Uncertain,
            F::Busy => ErrorCode::Busy,
            F::Rejected => ErrorCode::Rejected,
            F::Protocol => ErrorCode::Protocol,
            F::InvalidReply => ErrorCode::InvalidReply,
            F::InvalidEndpoint => ErrorCode::InvalidEndpoint,
            F::Local | F::Witness => ErrorCode::Local,
            F::Cancelled => ErrorCode::Cancelled,
            F::Unsupported => ErrorCode::Unsupported,
        })
    }
}

//! Public exchange configuration and local failure classifications.
use super::Continuity;

/// Errors are local decisions, never peer-provided diagnostic text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    Unavailable,
    Uncertain,
    Busy,
    Rejected,
    Protocol,
    InvalidReply,
    InvalidEndpoint,
    Local,
    Witness,
    Cancelled,
    Unsupported,
}
pub struct ExchangeOptions<'a> {
    pub continuity: Option<&'a Continuity>,
    pub retry_busy: bool,
}

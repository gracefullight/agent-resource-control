//! Additive bounded-Reaper vocabulary. Existing lifecycle shapes are unchanged.
//!
//! Registration travels from a managed-exec wrapper to the daemon. An action
//! travels the other way, over the wrapper's private callback socket: only the
//! wrapper, which retains the consuming wait for its direct child, may signal.
use crate::jobs::{JobId, ProcessIdentity};
use serde::{Deserialize, Serialize};

pub const OPERATION_REGISTER: &str = "reaper.register.v1";
pub const ACTION_KIND: &str = "arc.reaper.action.v1";
pub const RESULT_KIND: &str = "arc.reaper.result.v1";
pub const MAX_REGISTER_PAYLOAD_BYTES: usize = 8192;
/// A Unix socket path must also fit the smallest supported `sun_path`.
pub const MAX_ENDPOINT_BYTES: usize = 100;

/// Birth evidence for a direct child: PID, start time, boot, parent,
/// executable path and file identity, and both user IDs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChildEvidence {
    pub process: ProcessIdentity,
    pub parent_pid: u32,
    pub executable: String,
    pub executable_device: u64,
    pub executable_inode: u64,
    pub real_uid: u32,
    pub effective_uid: u32,
}
impl ChildEvidence {
    pub fn is_valid(&self) -> bool {
        self.process.is_valid()
            && self.parent_pid > 0
            && self.parent_pid <= i32::MAX as u32
            && self.executable.starts_with('/')
            && self.executable.len() <= 1024
            && !self.executable.contains('\0')
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegisterRequest {
    pub job_id: JobId,
    pub observer: ProcessIdentity,
    pub child: ChildEvidence,
    /// The process that launched the wrapper: the owning session.
    pub session: ProcessIdentity,
    pub endpoint: String,
    pub token: String,
}
impl RegisterRequest {
    pub fn is_valid(&self) -> bool {
        self.observer.is_valid()
            && self.session.is_valid()
            && self.child.is_valid()
            && self.child.parent_pid == self.observer.pid
            && self.child.process != self.observer
            && self.session != self.observer
            && self.session != self.child.process
            && valid_endpoint(&self.endpoint)
            && valid_nonce(&self.token)
    }
}

pub fn valid_endpoint(path: &str) -> bool {
    path.starts_with('/') && path.len() <= MAX_ENDPOINT_BYTES && !path.contains('\0')
}

pub fn valid_nonce(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Static rejection only: serde diagnostics never reach users or logs.
pub fn decode_register(payload: &serde_json::Value) -> Result<RegisterRequest, &'static str> {
    if !payload.is_object()
        || serde_json::to_vec(payload)
            .map_or(true, |bytes| bytes.len() > MAX_REGISTER_PAYLOAD_BYTES)
    {
        return Err(crate::ERROR_INVALID_REQUEST);
    }
    RegisterRequest::deserialize(payload)
        .ok()
        .filter(RegisterRequest::is_valid)
        .ok_or(crate::ERROR_INVALID_REQUEST)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Signal {
    Term,
    Kill,
}

/// One request authorizes at most one signal. Escalation is a new request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionRequest {
    pub kind: String,
    pub nonce: String,
    pub token: String,
    pub job_id: JobId,
    pub child: ChildEvidence,
    pub signal: Signal,
}

/// Why the supervising wrapper sent no signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Refusal {
    /// The caller is not the daemon instance this wrapper registered with.
    Unauthorized,
    /// The wrapper no longer retains this child: it was reaped or never attached.
    NotSupervised,
    /// Recorded and fresh native evidence disagree.
    IdentityMismatch,
    /// Fresh evidence could not be read, so identity is indeterminate.
    IdentityUnavailable,
    /// The child holds a terminal foreground, or that could not be excluded.
    Foreground,
    /// The kernel rejected the signal.
    SignalFailed,
}
impl Refusal {
    pub fn code(self) -> &'static str {
        match self {
            Self::Unauthorized => "WRAPPER_UNAUTHORIZED",
            Self::NotSupervised => "WRAPPER_NOT_SUPERVISED",
            Self::IdentityMismatch => "WRAPPER_IDENTITY_MISMATCH",
            Self::IdentityUnavailable => "WRAPPER_IDENTITY_UNAVAILABLE",
            Self::Foreground => "WRAPPER_FOREGROUND",
            Self::SignalFailed => "WRAPPER_SIGNAL_FAILED",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "status",
    rename_all = "SCREAMING_SNAKE_CASE",
    deny_unknown_fields
)]
pub enum ActionOutcome {
    Signaled {},
    Refused { reason: Refusal },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionResult {
    pub kind: String,
    pub nonce: String,
    pub outcome: ActionOutcome,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn identity(pid: u32) -> serde_json::Value {
        json!({"pid":pid,"start_token":"7","boot_id":"00000000-0000-0000-0000-000000000000"})
    }
    fn request() -> serde_json::Value {
        json!({
            "job_id":"0123456789abcdef0123456789abcdef",
            "observer":identity(20),
            "child":{"process":identity(30),"parent_pid":20,"executable":"/bin/sleep",
                "executable_device":1,"executable_inode":2,"real_uid":501,"effective_uid":501},
            "session":identity(10),
            "endpoint":"/tmp/arc-r-0/s",
            "token":"0123456789abcdef0123456789abcdef",
        })
    }

    #[test]
    fn registration_requires_a_direct_child_distinct_session_and_bounded_endpoint() {
        assert!(decode_register(&request()).is_ok());
        for (pointer, value) in [
            ("/child/parent_pid", json!(21)),
            ("/child/executable", json!("sleep")),
            ("/child/process", identity(20)),
            ("/session", identity(20)),
            ("/session", identity(30)),
            ("/endpoint", json!("relative/s")),
            ("/endpoint", json!(format!("/{}", "a".repeat(100)))),
            ("/token", json!("0123456789ABCDEF0123456789abcdef")),
            ("/token", json!("short")),
        ] {
            let mut changed = request();
            *changed.pointer_mut(pointer).unwrap() = value;
            assert_eq!(
                decode_register(&changed),
                Err(crate::ERROR_INVALID_REQUEST),
                "{pointer}"
            );
        }
        let mut extra = request();
        extra["argv"] = json!(["secret"]);
        assert!(decode_register(&extra).is_err());
        assert!(decode_register(&json!([])).is_err());
    }

    #[test]
    fn action_results_are_tagged_and_refusals_have_distinct_codes() {
        let signaled = ActionResult {
            kind: RESULT_KIND.into(),
            nonce: "0".repeat(32),
            outcome: ActionOutcome::Signaled {},
        };
        assert_eq!(
            serde_json::to_value(&signaled).unwrap(),
            json!({"kind":RESULT_KIND,"nonce":"0".repeat(32),"outcome":{"status":"SIGNALED"}})
        );
        let refused: ActionResult = serde_json::from_value(json!({
            "kind":RESULT_KIND,"nonce":"0".repeat(32),
            "outcome":{"status":"REFUSED","reason":"IDENTITY_MISMATCH"}
        }))
        .unwrap();
        assert_eq!(
            refused.outcome,
            ActionOutcome::Refused {
                reason: Refusal::IdentityMismatch
            }
        );
        // Unknown members never widen what a reply can mean.
        for invalid in [
            json!({"kind":RESULT_KIND,"nonce":"0","outcome":{"status":"SIGNALED","pid":1}}),
            json!({"kind":RESULT_KIND,"nonce":"0","outcome":{"status":"SIGNALED"},"extra":1}),
            json!({"kind":RESULT_KIND,"nonce":"0","outcome":{"status":"REFUSED"}}),
        ] {
            assert!(serde_json::from_value::<ActionResult>(invalid).is_err());
        }
        let codes: std::collections::BTreeSet<_> = [
            Refusal::Unauthorized,
            Refusal::NotSupervised,
            Refusal::IdentityMismatch,
            Refusal::IdentityUnavailable,
            Refusal::Foreground,
            Refusal::SignalFailed,
        ]
        .into_iter()
        .map(Refusal::code)
        .collect();
        assert_eq!(codes.len(), 6);
    }
}

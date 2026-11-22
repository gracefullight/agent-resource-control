//! Optional usage attribution carried on a job registration (lane B).
//!
//! `StartRequest::Register.usage` and `traffic::JobRequest::Enqueue.usage`
//! are sent only to a daemon that holds `meter::usage::ADVERTISEMENT`; an
//! absent value is omitted, so protocol-1 bytes are unchanged. Raw vendor
//! session and tool-call ids never appear here.
use crate::traffic::fairness::ProjectKey;
use serde::{Deserialize, Serialize};

/// The stored vendor. The hook adapter id `claude` (also the event-key digest
/// input) maps to `claude-code`; `cursor` stays `cursor`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Vendor {
    #[serde(rename = "claude-code")]
    ClaudeCode,
    #[serde(rename = "cursor")]
    Cursor,
}
impl Vendor {
    /// The single adapter-id mapping.
    pub fn from_adapter(adapter: &str) -> Option<Self> {
        match adapter {
            "claude" => Some(Self::ClaudeCode),
            "cursor" => Some(Self::Cursor),
            _ => None,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ClaudeCode => "claude-code",
            Self::Cursor => "cursor",
        }
    }
}

/// The hook process's own measured time, in whole milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u64", into = "u64")]
pub struct HookOverheadMs(u64);
impl HookOverheadMs {
    pub fn get(self) -> u64 {
        self.0
    }
}
impl TryFrom<u64> for HookOverheadMs {
    type Error = &'static str;
    fn try_from(value: u64) -> Result<Self, Self::Error> {
        (value <= super::MAX_TIMESTAMP)
            .then_some(Self(value))
            .ok_or("invalid hook overhead")
    }
}
impl From<HookOverheadMs> for u64 {
    fn from(value: HookOverheadMs) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UsageFields {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vendor: Option<Vendor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hook_overhead_ms: Option<HookOverheadMs>,
    /// For a registration; an enqueue also carries `project` (fairness).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_key: Option<ProjectKey>,
}

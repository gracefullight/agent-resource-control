use super::MAX_SAFE_INTEGER;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum GovernorSwitchKey {
    #[serde(rename = "governor.enabled")]
    GovernorEnabled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct GovernorSwitchState {
    pub key: GovernorSwitchKey,
    pub enabled: bool,
    #[cfg_attr(test, ts(type = "number"))]
    pub revision: u64,
    /// Absent only for the default ON/revision-0 state, never a guessed OFF.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(test, ts(as = "Option<f64>"))]
    pub updated_at_ms: Option<u64>,
}
impl Default for GovernorSwitchState {
    fn default() -> Self {
        Self {
            key: GovernorSwitchKey::GovernorEnabled,
            enabled: true,
            revision: 0,
            updated_at_ms: None,
        }
    }
}
impl GovernorSwitchState {
    pub fn is_valid(&self) -> bool {
        self.revision <= MAX_SAFE_INTEGER
            && match self.updated_at_ms {
                Some(at) => at <= MAX_SAFE_INTEGER,
                None => self.enabled && self.revision == 0,
            }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct GetGovernorSwitchRequest {
    pub key: GovernorSwitchKey,
    /// Read back the committed receipt after an uncertain acknowledgment.
    /// Absence of a receipt is NOT evidence that a write cannot still commit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mutation_id: Option<String>,
}
impl Default for GetGovernorSwitchRequest {
    fn default() -> Self {
        Self {
            key: GovernorSwitchKey::GovernorEnabled,
            mutation_id: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct GovernorSwitchReadResult {
    pub switch: GovernorSwitchState,
    /// If present, the original committed result with replayed=true; the current
    /// switch above can be newer. If absent, the request outcome remains unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mutation_result: Option<GovernorMutationResult>,
}
impl GovernorSwitchReadResult {
    pub fn is_valid_for(&self, query: &GetGovernorSwitchRequest) -> bool {
        if !self.switch.is_valid()
            || query
                .mutation_id
                .as_ref()
                .is_some_and(|id| mutation_base(id).is_none())
        {
            return false;
        }
        match &self.mutation_result {
            None => true,
            Some(
                result @ GovernorMutationResult::Committed {
                    mutation_id,
                    replayed: true,
                    ..
                },
            ) if query.mutation_id.as_ref() == Some(mutation_id) => {
                let Some(expected_revision) = mutation_base(mutation_id) else {
                    return false;
                };
                result.switch().revision <= self.switch.revision
                    && (result.switch().revision != self.switch.revision
                        || result.switch() == &self.switch)
                    && result.is_valid_for(&SetGovernorSwitchRequest {
                        key: GovernorSwitchKey::GovernorEnabled,
                        enabled: result.switch().enabled,
                        expected_revision,
                        mutation_id: mutation_id.clone(),
                    })
            }
            Some(_) => false,
        }
    }
}

pub(super) fn mutation_base(id: &str) -> Option<u64> {
    let (revision, nonce) = id.split_once('-')?;
    let value: u64 = revision.parse().ok()?;
    (value <= MAX_SAFE_INTEGER && revision == value.to_string() && valid_release_id(nonce))
        .then_some(value)
}

/// The mutation ID is `<expected_revision>-<32 lowercase hex digits>`.
/// Binding it to its immutable CAS base makes an evicted old receipt safe:
/// it can only encounter a stale revision, never become a fresh mutation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct SetGovernorSwitchRequest {
    pub key: GovernorSwitchKey,
    pub enabled: bool,
    #[cfg_attr(test, ts(type = "number"))]
    pub expected_revision: u64,
    pub mutation_id: String,
}
impl SetGovernorSwitchRequest {
    pub fn is_valid(&self) -> bool {
        mutation_base(&self.mutation_id) == Some(self.expected_revision)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum GovernorConflictReason {
    RevisionConflict,
    MutationIdConflict,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum GovernorRejectionReason {
    RevisionExhausted,
    ReceiptCapacity,
}

/// Only committed acknowledgments and confirmed no-effect outcomes belong here.
/// Transport loss, timeout and generic DB_BUSY are NOT Rejected results.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "status",
    rename_all = "SCREAMING_SNAKE_CASE",
    deny_unknown_fields
)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub enum GovernorMutationResult {
    Committed {
        mutation_id: String,
        switch: GovernorSwitchState,
        changed: bool,
        /// Replays contain the original committed state, possibly historical.
        replayed: bool,
    },
    Conflict {
        mutation_id: String,
        switch: GovernorSwitchState,
        reason: GovernorConflictReason,
    },
    Rejected {
        mutation_id: String,
        switch: GovernorSwitchState,
        reason: GovernorRejectionReason,
    },
}
impl GovernorMutationResult {
    pub fn switch(&self) -> &GovernorSwitchState {
        match self {
            Self::Committed { switch, .. }
            | Self::Conflict { switch, .. }
            | Self::Rejected { switch, .. } => switch,
        }
    }
    pub fn is_valid_for(&self, request: &SetGovernorSwitchRequest) -> bool {
        if !request.is_valid() || !self.switch().is_valid() {
            return false;
        }
        let id = match self {
            Self::Committed {
                mutation_id,
                switch,
                changed,
                ..
            } => {
                let expected = request.expected_revision.checked_add(u64::from(*changed));
                if switch.enabled != request.enabled || Some(switch.revision) != expected {
                    return false;
                }
                mutation_id
            }
            Self::Conflict {
                mutation_id,
                switch,
                reason,
            } => {
                if *reason == GovernorConflictReason::RevisionConflict
                    && switch.revision == request.expected_revision
                {
                    return false;
                }
                mutation_id
            }
            Self::Rejected {
                mutation_id,
                switch,
                reason,
            } => {
                if switch.revision != request.expected_revision
                    || (*reason == GovernorRejectionReason::RevisionExhausted
                        && (switch.revision != MAX_SAFE_INTEGER
                            || switch.enabled == request.enabled))
                {
                    return false;
                }
                mutation_id
            }
        };
        id == &request.mutation_id
    }
}

pub(super) fn valid_release_id(id: &str) -> bool {
    id.len() == 32
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

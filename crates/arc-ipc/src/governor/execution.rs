//! Current clients opt into these wrappers. No legacy Traffic enum is extended.
use super::{switch::valid_release_id, MAX_LAUNCH_VALID_FOR_MS, MAX_SAFE_INTEGER};
use crate::{
    jobs::{JobId, Outcome, ProcessIdentity},
    policy::launch,
    traffic,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "action",
    rename_all = "SCREAMING_SNAKE_CASE",
    deny_unknown_fields
)]
pub enum GovernorJobRequest {
    /// Enqueue/poll/cancel/fallback/status and permit-backed attach/complete keep
    /// their existing payloads, validated and authorized by the existing owner.
    Traffic { request: traffic::JobRequest },
    Attach {
        job_id: JobId,
        observer: ProcessIdentity,
        release_id: String,
        child: ProcessIdentity,
    },
    Complete {
        job_id: JobId,
        observer: ProcessIdentity,
        release_id: String,
        outcome: Outcome,
    },
}
impl GovernorJobRequest {
    pub fn is_valid(&self) -> bool {
        match self {
            Self::Traffic { request } => request.is_valid(),
            Self::Attach {
                observer,
                release_id,
                child,
                ..
            } => observer.is_valid() && child.is_valid() && valid_release_id(release_id),
            Self::Complete {
                observer,
                release_id,
                outcome,
                ..
            } => observer.is_valid() && outcome.is_valid() && valid_release_id(release_id),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "state",
    rename_all = "SCREAMING_SNAKE_CASE",
    deny_unknown_fields
)]
pub enum GovernorJobResult {
    /// Also carries terminal/running results; it does not imply a permit exists.
    Traffic { result: traffic::JobResult },
    /// Status/StatusV2 keep their distinct legacy schema, without owner context.
    Status { result: traffic::StatusResult },
    /// Eligibility only. This is not a native launch authorization.
    Unreserved {
        job_id: JobId,
        switch_revision: u64,
        ready_at_ms: u64,
    },
    /// A previous release might have started a child. Reconcile; do not spawn.
    Unconfirmed { job_id: JobId },
}
impl GovernorJobResult {
    pub fn is_valid(&self) -> bool {
        match self {
            Self::Traffic { result } => result.is_valid(),
            Self::Status { result } => result.is_valid(),
            Self::Unreserved {
                switch_revision,
                ready_at_ms,
                ..
            } => *switch_revision <= MAX_SAFE_INTEGER && *ready_at_ms <= MAX_SAFE_INTEGER,
            Self::Unconfirmed { .. } => true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum GovernorLaunchRequest {
    Reserved {
        request: launch::LaunchRequest,
    },
    Unreserved {
        job_id: JobId,
        observer: ProcessIdentity,
    },
}
impl GovernorLaunchRequest {
    pub fn job_id(&self) -> &JobId {
        match self {
            Self::Reserved { request } => &request.job_id,
            Self::Unreserved { job_id, .. } => job_id,
        }
    }
    pub fn observer(&self) -> &ProcessIdentity {
        match self {
            Self::Reserved { request } => &request.observer,
            Self::Unreserved { observer, .. } => observer,
        }
    }
    pub fn is_valid(&self) -> bool {
        match self {
            Self::Reserved { request } => request.is_valid(),
            Self::Unreserved { observer, .. } => observer.is_valid(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "state",
    rename_all = "SCREAMING_SNAKE_CASE",
    deny_unknown_fields
)]
pub enum GovernorLaunchResult {
    Reserved {
        result: Box<launch::LaunchResult>,
    },
    /// A RESERVED request can become UNRESERVED if pause won the ordering.
    /// No generation or lineage context is minted for this release.
    Unreserved {
        switch_revision: u64,
        release_id: String,
        valid_for_ms: u64,
    },
    /// State changed before release (e.g. resume). Poll again; do not spawn.
    Retry {},
    Unconfirmed {},
    Cancelled {},
    NotFound {},
}
impl GovernorLaunchResult {
    pub fn is_valid(&self) -> bool {
        match self {
            Self::Reserved { result } => result.is_valid(),
            Self::Unreserved {
                switch_revision,
                release_id,
                valid_for_ms,
            } => {
                *switch_revision <= MAX_SAFE_INTEGER
                    && valid_release_id(release_id)
                    && (1..=MAX_LAUNCH_VALID_FOR_MS).contains(valid_for_ms)
            }
            Self::Retry {} | Self::Unconfirmed {} | Self::Cancelled {} | Self::NotFound {} => true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "state",
    rename_all = "SCREAMING_SNAKE_CASE",
    deny_unknown_fields
)]
pub enum GovernorUnreservedAck {
    Started {
        job_id: JobId,
        observer: ProcessIdentity,
        release_id: String,
        child: ProcessIdentity,
    },
    Unspawned {
        job_id: JobId,
        observer: ProcessIdentity,
        release_id: String,
    },
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
pub enum GovernorLaunchAck {
    Reserved { ack: launch::LaunchAck },
    Unreserved { ack: GovernorUnreservedAck },
}
impl GovernorLaunchAck {
    /// Bind the ACK to both the authenticated request and delivered release.
    pub fn matches(&self, request: &GovernorLaunchRequest, result: &GovernorLaunchResult) -> bool {
        if !request.is_valid() || !result.is_valid() {
            return false;
        }
        match (self, request, result) {
            (
                Self::Reserved { ack },
                GovernorLaunchRequest::Reserved { request },
                GovernorLaunchResult::Reserved { result },
            ) => {
                let generation = match result.as_ref() {
                    launch::LaunchResult::Ready {
                        permit_generation, ..
                    }
                    | launch::LaunchResult::Reused { permit_generation } => *permit_generation,
                    _ => return false,
                };
                generation == request.permit_generation && ack.matches(request)
            }
            (Self::Unreserved { ack }, _, GovernorLaunchResult::Unreserved { release_id, .. }) => {
                let (id, observer, token) = match ack {
                    GovernorUnreservedAck::Started {
                        job_id,
                        observer,
                        release_id,
                        child,
                    } if child.is_valid() => (job_id, observer, release_id),
                    GovernorUnreservedAck::Unspawned {
                        job_id,
                        observer,
                        release_id,
                    } => (job_id, observer, release_id),
                    _ => return false,
                };
                id == request.job_id() && observer == request.observer() && token == release_id
            }
            _ => false,
        }
    }
}

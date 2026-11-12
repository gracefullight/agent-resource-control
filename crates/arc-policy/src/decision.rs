use crate::{ObservationAvailability as A, ObservationSummary};
use arc_core::meter::{MemoryPressure, MAX_SAFE_INTEGER};
use arc_traffic::{ClassPolicy, Policy, Resources, WaitReason};
use serde::{Deserialize, Serialize};

pub const POLICY_SCHEMA_VERSION: u32 = 1;
pub const POLICY_VERSION: &str = "macos-memory-pressure-v1";
pub const POLICY_STATE_KEY: &str = "memory_admission";
pub const L0_PROFILE: &str = "macos-memory-l0-v1";
pub const MAX_POLICY_BYTES: usize = 16_384;
pub const MAX_JOB_DECISIONS: usize = 64;
pub const MAX_GLOBAL_DECISIONS: usize = 128;
pub const MAX_EXPIRY_BATCH: usize = 128;
pub const DECISION_RETENTION_MS: u64 = 90 * 24 * 60 * 60 * 1000;
/// Publication may be fenced only through a bounded native launch handshake.
/// The actual bound is also limited by the observation's remaining freshness.
pub const MAX_LAUNCH_FENCE_MS: u64 = 2000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClassLimits {
    pub window: u32,
    pub reservation: Resources,
}
impl From<ClassPolicy> for ClassLimits {
    fn from(value: ClassPolicy) -> Self {
        Self {
            window: value.window,
            reservation: value.reservation,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PolicyLimits {
    pub build: ClassLimits,
    pub test: ClassLimits,
    pub lint: ClassLimits,
    pub budget: Resources,
}
impl From<Policy> for PolicyLimits {
    fn from(value: Policy) -> Self {
        Self {
            build: value.build.into(),
            test: value.test.into(),
            lint: value.lint.into(),
            budget: value.budget,
        }
    }
}
impl PolicyLimits {
    pub fn is_valid(&self) -> bool {
        // This version does not tune budgets or increase static concurrency.
        *self == PolicyLimits::from(Policy::default())
    }
}
impl Default for PolicyLimits {
    fn default() -> Self {
        Policy::default().into()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyMode {
    L0,
    MemoryHold,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyReason {
    PressureNormal,
    PressureWarning,
    MemoryCritical,
    ObservationMissing,
    ObservationStale,
    ObservationFailed,
    ObservationUnsupported,
    ObservationDenied,
    ObservationInvalid,
    SamplerMismatch,
    ClockInvalid,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectivePolicy {
    pub schema_version: u32,
    pub policy_version: String,
    pub profile: String,
    pub mode: PolicyMode,
    pub reason: PolicyReason,
    pub observation: ObservationSummary,
    pub limits: PolicyLimits,
}
impl EffectivePolicy {
    pub fn is_valid(&self) -> bool {
        if self.schema_version != POLICY_SCHEMA_VERSION
            || self.policy_version != POLICY_VERSION
            || self.profile != L0_PROFILE
            || !self.observation.is_valid()
            || !self.limits.is_valid()
        {
            return false;
        }
        let expected = match (self.observation.availability, self.observation.pressure) {
            (A::Available, Some(MemoryPressure::Critical)) => {
                (PolicyMode::MemoryHold, PolicyReason::MemoryCritical)
            }
            (A::Available, Some(MemoryPressure::Normal)) => {
                (PolicyMode::L0, PolicyReason::PressureNormal)
            }
            (A::Available, Some(MemoryPressure::Warning)) => {
                (PolicyMode::L0, PolicyReason::PressureWarning)
            }
            (A::Missing, _) => (PolicyMode::L0, PolicyReason::ObservationMissing),
            (A::Stale, _) => (PolicyMode::L0, PolicyReason::ObservationStale),
            (A::Failed, _) => (PolicyMode::L0, PolicyReason::ObservationFailed),
            (A::Unsupported, _) => (PolicyMode::L0, PolicyReason::ObservationUnsupported),
            (A::Denied, _) => (PolicyMode::L0, PolicyReason::ObservationDenied),
            (A::Invalid, _) => {
                return self.mode == PolicyMode::L0
                    && matches!(
                        self.reason,
                        PolicyReason::ObservationInvalid
                            | PolicyReason::SamplerMismatch
                            | PolicyReason::ClockInvalid
                    )
            }
            _ => return false,
        };
        (self.mode, self.reason) == expected
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionKind {
    Admit,
    Wait,
    Reuse,
    Preserve,
    Bypass,
    Cancel,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionReason {
    MemoryPressure,
    Fifo,
    ClassWindow,
    CpuBudget,
    MemoryBudget,
    Admitted,
    ExistingReservation,
    Running,
    Remote,
    Unknown,
    Unsupported,
    Cancelled,
}
impl DecisionReason {
    /// Keep all original Traffic protocol-1 reply objects and enums decodable.
    pub fn legacy_wait(self) -> Option<WaitReason> {
        Some(match self {
            Self::MemoryPressure | Self::MemoryBudget => WaitReason::MemoryBudget,
            Self::Fifo => WaitReason::Fifo,
            Self::ClassWindow => WaitReason::ClassWindow,
            Self::CpuBudget => WaitReason::CpuBudget,
            _ => return None,
        })
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecordedDecision {
    pub policy_version: String,
    pub kind: DecisionKind,
    pub reason: DecisionReason,
    pub decided_at_ms: u64,
    pub permit_generation: Option<u64>,
    pub observation: ObservationSummary,
}
impl RecordedDecision {
    pub fn is_valid(&self) -> bool {
        self.policy_version == POLICY_VERSION
            && self.decided_at_ms <= MAX_SAFE_INTEGER
            && self
                .permit_generation
                .is_none_or(|value| (1..=MAX_SAFE_INTEGER).contains(&value))
            && self.observation.is_valid()
            && (self.kind != DecisionKind::Admit
                || (self.permit_generation.is_some()
                    && !(self.observation.availability == A::Available
                        && self.observation.pressure == Some(MemoryPressure::Critical))))
            && matches!(
                (self.kind, self.reason),
                (DecisionKind::Admit, DecisionReason::Admitted)
                    | (
                        DecisionKind::Wait,
                        DecisionReason::MemoryPressure
                            | DecisionReason::Fifo
                            | DecisionReason::ClassWindow
                            | DecisionReason::CpuBudget
                            | DecisionReason::MemoryBudget
                    )
                    | (DecisionKind::Reuse, DecisionReason::ExistingReservation)
                    | (DecisionKind::Preserve, DecisionReason::Running)
                    | (
                        DecisionKind::Bypass,
                        DecisionReason::Remote
                            | DecisionReason::Unknown
                            | DecisionReason::Unsupported
                    )
                    | (DecisionKind::Cancel, DecisionReason::Cancelled)
            )
            && (self.reason != DecisionReason::MemoryPressure
                || (self.observation.availability == A::Available
                    && self.observation.pressure == Some(MemoryPressure::Critical)))
    }
}

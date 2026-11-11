//! Pure, deterministic admission policy. Persistence and launch belong to callers.

use arc_detector::{Classification, JobClass, Locality, Reason};
use serde::{Deserialize, Serialize};

/// Largest value representable by the SQLite integer columns used by storage.
pub const MAX_RESOURCE_VALUE: u64 = i64::MAX as u64;

/// Resource estimates, in millicores and bytes. These are not operating-system caps.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resources {
    pub cpu_millis: u64,
    pub memory_bytes: u64,
}

impl Resources {
    fn valid_positive(self) -> bool {
        self.cpu_millis > 0
            && self.memory_bytes > 0
            && self.cpu_millis <= MAX_RESOURCE_VALUE
            && self.memory_bytes <= MAX_RESOURCE_VALUE
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TrafficClass {
    Build,
    Test,
    Lint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClassPolicy {
    pub window: u32,
    pub reservation: Resources,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Policy {
    pub build: ClassPolicy,
    pub test: ClassPolicy,
    pub lint: ClassPolicy,
    pub budget: Resources,
}

impl Default for Policy {
    fn default() -> Self {
        let gib = 1024 * 1024 * 1024;
        Self {
            build: ClassPolicy {
                window: 1,
                reservation: Resources {
                    cpu_millis: 2_000,
                    memory_bytes: 2 * gib,
                },
            },
            test: ClassPolicy {
                window: 1,
                reservation: Resources {
                    cpu_millis: 1_000,
                    memory_bytes: gib,
                },
            },
            lint: ClassPolicy {
                window: 1,
                reservation: Resources {
                    cpu_millis: 1_000,
                    memory_bytes: gib,
                },
            },
            budget: Resources {
                cpu_millis: 2_000,
                memory_bytes: 4 * gib,
            },
        }
    }
}

impl Policy {
    /// Reject impossible configurations before placing eligible jobs in a queue.
    pub fn validate(&self) -> Result<(), PolicyError> {
        if !self.budget.valid_positive() {
            return Err(PolicyError::InvalidBudget);
        }
        for class in [self.build, self.test, self.lint] {
            if class.window == 0 || !class.reservation.valid_positive() {
                return Err(PolicyError::InvalidClassPolicy);
            }
            if class.reservation.cpu_millis > self.budget.cpu_millis
                || class.reservation.memory_bytes > self.budget.memory_bytes
            {
                return Err(PolicyError::ReservationExceedsBudget);
            }
        }
        Ok(())
    }

    pub fn class(&self, class: TrafficClass) -> ClassPolicy {
        match class {
            TrafficClass::Build => self.build,
            TrafficClass::Test => self.test,
            TrafficClass::Lint => self.lint,
        }
    }
}

/// Snapshot of active permits, obtained inside the same transaction as a grant.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Active {
    pub build: u32,
    pub test: u32,
    pub lint: u32,
    pub resources: Resources,
}

impl Active {
    fn count(self, class: TrafficClass) -> u32 {
        match class {
            TrafficClass::Build => self.build,
            TrafficClass::Test => self.test,
            TrafficClass::Lint => self.lint,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BypassReason {
    Remote,
    Unknown,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WaitReason {
    Fifo,
    ClassWindow,
    CpuBudget,
    MemoryBudget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Bypass(BypassReason),
    Wait(WaitReason),
    Admit {
        class: TrafficClass,
        reservation: Resources,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyError {
    InvalidClassification,
    InvalidBudget,
    InvalidClassPolicy,
    ReservationExceedsBudget,
    ResourceOverflow,
}

/// Evaluate an independently classified direct invocation.
///
/// The caller must determine `is_fifo_head` from the durable queue in the same
/// transaction as its active-permit snapshot and eventual grant. Bypass decisions
/// must not insert a queue row or reserve resources. Invalid inputs fail closed.
pub fn decide(
    classification: Classification,
    active: Active,
    policy: &Policy,
    is_fifo_head: bool,
) -> Result<Decision, PolicyError> {
    if !classification.is_consistent() {
        return Err(PolicyError::InvalidClassification);
    }
    let class = match (classification.classification, classification.reason) {
        (JobClass::Build, Reason::CargoBuild) => TrafficClass::Build,
        (JobClass::Test, Reason::CargoTest | Reason::BunTest) => TrafficClass::Test,
        (JobClass::Lint, Reason::CargoClippy | Reason::CargoFmt) => TrafficClass::Lint,
        _ if classification.locality == Locality::Remote => {
            return Ok(Decision::Bypass(BypassReason::Remote));
        }
        _ if classification.classification == JobClass::Unknown => {
            return Ok(Decision::Bypass(BypassReason::Unknown));
        }
        _ => return Ok(Decision::Bypass(BypassReason::Unsupported)),
    };
    policy.validate()?;
    if !is_fifo_head {
        return Ok(Decision::Wait(WaitReason::Fifo));
    }
    let config = policy.class(class);
    if active.count(class) >= config.window {
        return Ok(Decision::Wait(WaitReason::ClassWindow));
    }
    let cpu = active
        .resources
        .cpu_millis
        .checked_add(config.reservation.cpu_millis)
        .filter(|sum| *sum <= MAX_RESOURCE_VALUE)
        .ok_or(PolicyError::ResourceOverflow)?;
    let memory = active
        .resources
        .memory_bytes
        .checked_add(config.reservation.memory_bytes)
        .filter(|sum| *sum <= MAX_RESOURCE_VALUE)
        .ok_or(PolicyError::ResourceOverflow)?;
    if cpu > policy.budget.cpu_millis {
        return Ok(Decision::Wait(WaitReason::CpuBudget));
    }
    if memory > policy.budget.memory_bytes {
        return Ok(Decision::Wait(WaitReason::MemoryBudget));
    }
    Ok(Decision::Admit {
        class,
        reservation: config.reservation,
    })
}

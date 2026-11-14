//! Negotiated four-class limits. The existing memory-policy v1 DTO stays frozen.
use crate::{ClassLimits, PolicyLimits};
use arc_traffic::{ClassPolicy, Policy, Resources};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptPolicyLimits {
    pub build: ClassLimits,
    pub test: ClassLimits,
    pub lint: ClassLimits,
    pub typecheck: ClassLimits,
    pub budget: Resources,
}
impl From<Policy> for ScriptPolicyLimits {
    fn from(value: Policy) -> Self {
        Self {
            build: value.build.into(),
            test: value.test.into(),
            lint: value.lint.into(),
            typecheck: value.typecheck.into(),
            budget: value.budget,
        }
    }
}
impl Default for ScriptPolicyLimits {
    fn default() -> Self {
        Policy::default().into()
    }
}
impl ScriptPolicyLimits {
    pub fn is_valid(&self) -> bool {
        let class = |limit: ClassLimits| ClassPolicy {
            window: limit.window,
            reservation: limit.reservation,
        };
        let policy = Policy {
            build: class(self.build),
            test: class(self.test),
            lint: class(self.lint),
            typecheck: class(self.typecheck),
            budget: self.budget,
        };
        policy.validate().is_ok()
            && [
                self.budget,
                self.build.reservation,
                self.test.reservation,
                self.lint.reservation,
                self.typecheck.reservation,
            ]
            .iter()
            .all(|r| {
                r.cpu_millis <= arc_core::meter::MAX_SAFE_INTEGER
                    && r.memory_bytes <= arc_core::meter::MAX_SAFE_INTEGER
            })
    }
    pub fn legacy(self) -> PolicyLimits {
        PolicyLimits {
            build: self.build,
            test: self.test,
            lint: self.lint,
            budget: self.budget,
        }
    }
}

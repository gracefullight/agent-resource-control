//! Test-only projections keep the inherited protocol source byte-identical.
//! Exhaustive conversions bind every field to the real Rust lifecycle types;
//! serialization equality below guards their serde names and representation.
use crate::jobs::{Outcome, SpawnReason};
use serde::Serialize;
use ts_rs::TS;

#[derive(Serialize, TS)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[ts(rename = "SpawnReason")]
pub(crate) enum SpawnReasonType {
    NotFound,
    PermissionDenied,
    InvalidInput,
    ResourceUnavailable,
    Other,
}

impl From<SpawnReason> for SpawnReasonType {
    fn from(reason: SpawnReason) -> Self {
        match reason {
            SpawnReason::NotFound => Self::NotFound,
            SpawnReason::PermissionDenied => Self::PermissionDenied,
            SpawnReason::InvalidInput => Self::InvalidInput,
            SpawnReason::ResourceUnavailable => Self::ResourceUnavailable,
            SpawnReason::Other => Self::Other,
        }
    }
}

#[derive(Serialize, TS)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE", deny_unknown_fields)]
#[ts(rename = "Outcome")]
pub(crate) enum OutcomeType {
    Exited {
        code: u8,
    },
    Signaled {
        signal: u8,
        core_dumped: bool,
    },
    SpawnFailed {
        reason: SpawnReasonType,
        os_code: Option<i32>,
    },
}

impl From<&Outcome> for OutcomeType {
    fn from(outcome: &Outcome) -> Self {
        match outcome {
            Outcome::Exited { code } => Self::Exited { code: *code },
            Outcome::Signaled {
                signal,
                core_dumped,
            } => Self::Signaled {
                signal: *signal,
                core_dumped: *core_dumped,
            },
            Outcome::SpawnFailed { reason, os_code } => Self::SpawnFailed {
                reason: (*reason).into(),
                os_code: *os_code,
            },
        }
    }
}

#[test]
fn declarations_preserve_actual_legacy_serde_for_every_variant_and_nullable_code() {
    let mut outcomes = vec![];
    for code in [0, 17, u8::MAX] {
        outcomes.push(Outcome::Exited { code });
    }
    for core_dumped in [false, true] {
        outcomes.push(Outcome::Signaled {
            signal: 15,
            core_dumped,
        });
    }
    for reason in [
        SpawnReason::NotFound,
        SpawnReason::PermissionDenied,
        SpawnReason::InvalidInput,
        SpawnReason::ResourceUnavailable,
        SpawnReason::Other,
    ] {
        assert_eq!(
            serde_json::to_value(SpawnReasonType::from(reason)).unwrap(),
            serde_json::to_value(reason).unwrap()
        );
        for os_code in [None, Some(0), Some(i32::MIN), Some(i32::MAX)] {
            outcomes.push(Outcome::SpawnFailed { reason, os_code });
        }
    }
    for actual in outcomes {
        let projected = serde_json::to_value(OutcomeType::from(&actual)).unwrap();
        assert_eq!(projected, serde_json::to_value(&actual).unwrap());
        assert_eq!(
            serde_json::from_value::<Outcome>(projected).unwrap(),
            actual
        );
    }
}

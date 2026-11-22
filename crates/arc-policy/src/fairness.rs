//! Additive, negotiated-only admission vocabulary for project fair queuing
//! (AT-TRF-08).
//!
//! `DecisionReason` is the vocabulary of protocol-1 replies, v1 diagnostics,
//! policy replies and the generated desktop bindings, so it gains no variant.
//! A fairness reason is recorded on the grant decision of the job that
//! received a fair turn. Every reader that did not opt in receives
//! [`FairnessReason::projection`]; only an operation that negotiated it
//! carries [`NegotiatedReason`].
use crate::{DecisionKind, DecisionReason};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FairnessReason {
    /// Granted ahead of earlier entries of a more recently granted project.
    ProjectFairTurn,
}

impl FairnessReason {
    pub const ALL: [Self; 1] = [Self::ProjectFairTurn];

    /// The valid existing pair a reader without the negotiated read receives.
    pub const fn projection(self) -> (DecisionKind, DecisionReason) {
        match self {
            Self::ProjectFairTurn => (DecisionKind::Admit, DecisionReason::Admitted),
        }
    }
}

/// The reason as recorded: the existing vocabulary or an additive reason. The
/// two serialized vocabularies are disjoint, so the untagged form is exact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum NegotiatedReason {
    Fairness(FairnessReason),
    Recorded(DecisionReason),
}

impl NegotiatedReason {
    /// The existing-vocabulary reason for every reader that did not opt in.
    pub const fn projected(self) -> DecisionReason {
        match self {
            Self::Fairness(reason) => reason.projection().1,
            Self::Recorded(reason) => reason,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ObservationAvailability, ObservationSummary, RecordedDecision, POLICY_VERSION};
    use serde_json::json;

    const EXISTING: [&str; 12] = [
        "memory_pressure",
        "fifo",
        "class_window",
        "cpu_budget",
        "memory_budget",
        "admitted",
        "existing_reservation",
        "running",
        "remote",
        "unknown",
        "unsupported",
        "cancelled",
    ];

    #[test]
    fn fairness_reasons_project_to_a_valid_existing_decision() {
        for reason in FairnessReason::ALL {
            let raw = json!(reason);
            assert!(!EXISTING.contains(&raw.as_str().unwrap()), "{raw}");
            let (kind, projected) = reason.projection();
            assert!(EXISTING.contains(&json!(projected).as_str().unwrap()));
            let observation = ObservationSummary {
                availability: ObservationAvailability::Missing,
                pressure: None,
                age_ms: None,
                stale_after_ms: None,
                sampler: None,
                sequence: None,
                observed_at_ms: None,
                observed_monotonic_ms: None,
            };
            let decision = RecordedDecision {
                policy_version: POLICY_VERSION.into(),
                kind,
                reason: projected,
                decided_at_ms: 1,
                permit_generation: Some(1),
                observation,
            };
            assert!(decision.is_valid(), "{decision:?}");
            let negotiated: NegotiatedReason = serde_json::from_value(raw.clone()).unwrap();
            assert_eq!(negotiated, NegotiatedReason::Fairness(reason));
            assert_eq!(json!(negotiated), raw);
            assert_eq!(negotiated.projected(), projected);
            // The existing type cannot carry the additive vocabulary at all.
            assert!(serde_json::from_value::<DecisionReason>(raw).is_err());
        }
    }

    #[test]
    fn every_existing_reason_round_trips_unchanged_through_the_negotiated_form() {
        for tag in EXISTING {
            let recorded: DecisionReason = serde_json::from_value(json!(tag)).unwrap();
            let negotiated: NegotiatedReason = serde_json::from_value(json!(tag)).unwrap();
            assert_eq!(negotiated, NegotiatedReason::Recorded(recorded));
            assert_eq!(negotiated.projected(), recorded);
            assert_eq!(json!(negotiated), json!(tag));
        }
    }
}

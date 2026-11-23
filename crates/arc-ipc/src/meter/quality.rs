//! Quality flags of one reported Meter value (MTR-015 item 3, R-005 item 3,
//! NFR-012). Shared by the v2 usage summary, the attribution section and the
//! job footprint.
//!
//! Every CPU, memory, I/O, disk-written, footprint and per-vendor total value
//! carries its availability and its basis; a missing value is `None` with
//! [`Availability::Unavailable`] and a reason, never a silent zero. A value
//! that is only a lower bound (waited-only CPU, an exited process missed
//! between scans, a bounded walk that stopped) is [`Availability::Partial`].
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Availability {
    /// Measured over the whole stated scope.
    Complete,
    /// Measured over part of the scope: a lower bound.
    Partial,
    /// Not measured; the value is `None`.
    Unavailable,
}

/// One flagged value. `basis` names how it was obtained, for example
/// `tree_samples`, `waited_rusage`, `scan_deltas` or `allocated`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Flagged<T> {
    pub value: Option<T>,
    pub availability: Availability,
    pub basis: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

impl<T> Flagged<T> {
    pub fn unavailable(basis: &str, reason: &str) -> Self {
        Self {
            value: None,
            availability: Availability::Unavailable,
            basis: basis.to_owned(),
            reason: Some(reason.to_owned()),
        }
    }
    /// `None` exactly when unavailable, and an unavailable value names why.
    pub fn is_consistent(&self) -> bool {
        match self.availability {
            Availability::Unavailable => self.value.is_none() && self.reason.is_some(),
            Availability::Complete | Availability::Partial => self.value.is_some(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn an_unavailable_value_is_null_with_a_reason_and_never_zero() {
        let missing = Flagged::<f64>::unavailable("tree_samples", "no_samples");
        assert!(missing.is_consistent());
        assert_eq!(
            serde_json::to_value(&missing).unwrap(),
            json!({"value":null,"availability":"unavailable","basis":"tree_samples","reason":"no_samples"})
        );
        let zero = Flagged {
            value: Some(0.0),
            availability: Availability::Unavailable,
            basis: "tree_samples".into(),
            reason: Some("x".into()),
        };
        assert!(!zero.is_consistent());
        let partial = Flagged::<u64> {
            value: None,
            availability: Availability::Partial,
            basis: "allocated".into(),
            reason: None,
        };
        assert!(!partial.is_consistent());
    }
}

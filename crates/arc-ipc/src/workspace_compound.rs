//! Negotiated representation of a resolved workspace with unlike local
//! classes. Existing protocol-1 classifications retain their projection.
use crate::{jobs::JobId, Classification, JobClass, Locality, Reason, Uncertainty};
use serde::{Deserialize, Serialize};

pub const OPERATION: &str = "detector.workspace-compound.v1";
pub const MAX_MEMBERS: usize = 33;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Member {
    pub member: String,
    pub script: String,
    pub classification: Classification,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Compound {
    pub kind: String,
    pub locality: String,
    pub members: Vec<Member>,
    /// The scheduler cannot safely choose one class window for this graph.
    pub admission: Classification,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum CompoundRequest {
    Probe,
    Analyze {
        members: Vec<Member>,
        job_id: Option<JobId>,
    },
    Read {
        job_id: JobId,
    },
}

impl Compound {
    /// Only a complete, finite, all-local graph with different classes is
    /// represented here. An unresolved or mixed-locality graph keeps its
    /// existing classification and scheduling behavior.
    pub fn local(members: Vec<Member>) -> Option<Self> {
        if !(2..=MAX_MEMBERS).contains(&members.len()) {
            return None;
        }
        let first = members[0].classification.classification;
        if !members.iter().all(|m| {
            let path = &m.member;
            let relative = path == "."
                || (!path.is_empty()
                    && path.len() <= 4096
                    && path.split('/').all(|part| {
                        !part.is_empty() && part != "." && part != ".." && !part.contains('\0')
                    }));
            relative
                && !m.script.is_empty()
                && m.script.len() <= 4096
                && !m.script.contains('\0')
                && m.classification.is_consistent()
                && m.classification.locality.is_local_compute()
                && m.classification.uncertainty == Uncertainty::None
                && matches!(
                    m.classification.classification,
                    JobClass::Build | JobClass::Test | JobClass::Lint | JobClass::Typecheck
                )
        }) || members
            .iter()
            .all(|m| m.classification.classification == first)
        {
            return None;
        }
        Some(Self {
            kind: "WORKSPACE_COMPOUND".into(),
            locality: "LOCAL_COMPUTE".into(),
            members,
            admission: Classification {
                classification: JobClass::Unknown,
                locality: Locality::Unknown,
                reason: Reason::UnresolvedCompound,
                uncertainty: Uncertainty::Observe,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(name: &str, classification: JobClass, locality: Locality, reason: Reason) -> Member {
        Member {
            member: name.into(),
            script: "ci".into(),
            classification: Classification {
                classification,
                locality,
                reason,
                uncertainty: Uncertainty::None,
            },
        }
    }

    #[test]
    fn local_compound_keeps_leaves_and_rejects_incomplete_graphs() {
        let test = member(
            "packages/a",
            JobClass::Test,
            Locality::HostLocal,
            Reason::BunTest,
        );
        let build = member(
            "packages/b",
            JobClass::Build,
            Locality::HostLocal,
            Reason::BunScriptBuild,
        );
        let compound = Compound::local(vec![test.clone(), build.clone()]).unwrap();
        assert_eq!(compound.members, vec![test.clone(), build.clone()]);
        assert_eq!(compound.admission.classification, JobClass::Unknown);
        assert_eq!(compound.admission.uncertainty, Uncertainty::Observe);
        assert_eq!(serde_json::to_vec(&compound.admission).unwrap(), br#"{"classification":"UNKNOWN","locality":"UNKNOWN","reason":"UNRESOLVED_COMPOUND","uncertainty":"OBSERVE"}"#);
        assert!(Compound::local(vec![test.clone(), test.clone()]).is_none());
        let remote = member(
            "packages/b",
            JobClass::RemoteCi,
            Locality::Remote,
            Reason::GhWorkflowRun,
        );
        assert!(Compound::local(vec![test.clone(), remote]).is_none());
        let container = member(
            "packages/container",
            JobClass::Build,
            Locality::LocalContainer,
            Reason::LocalEndpointBuild,
        );
        let mixed_locations = Compound::local(vec![test.clone(), container]).unwrap();
        assert_eq!(
            mixed_locations.members[1].classification.locality,
            Locality::LocalContainer
        );
        assert!(crate::compute_location::negotiated(OPERATION));
        let mut invalid = build;
        invalid.member = "../outside".into();
        assert!(Compound::local(vec![test, invalid]).is_none());
    }
}

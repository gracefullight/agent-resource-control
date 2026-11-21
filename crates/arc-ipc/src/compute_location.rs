//! Negotiation carrier for the additive `LOCAL_CONTAINER` compute location.
//!
//! The operation is the capability: an earlier peer answers
//! `UNSUPPORTED_CAPABILITY`. `LOCAL_CONTAINER` crosses the wire only in a
//! reply to an operation named here. Every request and every other reply
//! carries the `HOST_LOCAL` projection, and protocol-1 lifecycle replies keep
//! `Classification::legacy`. A receiver stores the canonical form.
use crate::Classification;
use serde_json::Value;

/// Endpoint-aware classification whose reply may name `LOCAL_CONTAINER`.
pub const OPERATION_CLASSIFY: &str = "job.classify.container.v1";
/// Lifecycle status whose snapshot may name `LOCAL_CONTAINER`.
pub const OPERATION_JOB_STATUS: &str = "job.status.container.v1";

const NEGOTIATED: &str = "LOCAL_CONTAINER";
const PROJECTED: &str = "HOST_LOCAL";

/// Only these operations opt in to the unprojected vocabulary.
pub fn negotiated(operation: &str) -> bool {
    matches!(operation, OPERATION_CLASSIFY | OPERATION_JOB_STATUS)
}

/// The one typed projection for a peer that did not negotiate.
pub fn project(classification: Classification) -> Classification {
    classification.host_local_projection()
}

/// The form admission, duplicate detection and storage use. Validate the wire
/// form first: this never repairs an inconsistent record.
pub fn canonical(classification: Classification) -> Classification {
    classification.canonical()
}

/// Whether any classification in a reply names the negotiated location.
pub fn carries_container(reply: &Value) -> bool {
    match reply {
        Value::Object(fields) => fields
            .iter()
            .any(|(key, value)| is_negotiated(key, value) || carries_container(value)),
        Value::Array(items) => items.iter().any(carries_container),
        _ => false,
    }
}

/// Project every classification in a reply, whatever its result shape.
pub fn project_reply(reply: &mut Value) {
    match reply {
        Value::Object(fields) => {
            for (key, value) in fields {
                if is_negotiated(key, value) {
                    *value = Value::from(PROJECTED);
                } else {
                    project_reply(value);
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(project_reply),
        _ => (),
    }
}

fn is_negotiated(key: &str, value: &Value) -> bool {
    key == "locality" && value == NEGOTIATED
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{JobClass, Locality, Reason, Uncertainty};
    use serde_json::json;

    fn container() -> Classification {
        Classification {
            classification: JobClass::Build,
            locality: Locality::LocalContainer,
            reason: Reason::LocalEndpointBuild,
            uncertainty: Uncertainty::None,
        }
    }

    #[test]
    fn projected_local_container_record_is_host_local() {
        let projected = project(container());
        assert_eq!(projected.locality, Locality::HostLocal);
        assert_eq!(
            json!(projected),
            json!({"classification":"BUILD","locality":"HOST_LOCAL",
                "reason":"LOCAL_ENDPOINT_BUILD","uncertainty":"NONE"})
        );
        // Both wire forms of the one record validate; neither is protocol-1.
        assert!(projected.is_consistent());
        assert_eq!(projected.legacy().classification, JobClass::Unknown);
        assert_eq!(project(projected), projected);
    }

    #[test]
    fn projection_leaves_every_other_location_unchanged() {
        for (classification, locality, reason, uncertainty) in [
            (
                JobClass::RemoteExec,
                Locality::Remote,
                Reason::RemoteEndpointExec,
                Uncertainty::None,
            ),
            (
                JobClass::RemoteCi,
                Locality::Remote,
                Reason::RemoteCiDispatch,
                Uncertainty::None,
            ),
            (
                JobClass::Mixed,
                Locality::Mixed,
                Reason::CompoundMixed,
                Uncertainty::None,
            ),
            (
                JobClass::Unknown,
                Locality::Unknown,
                Reason::EndpointAmbiguous,
                Uncertainty::Observe,
            ),
            (
                JobClass::Test,
                Locality::HostLocal,
                Reason::CargoTest,
                Uncertainty::None,
            ),
        ] {
            let record = Classification {
                classification,
                locality,
                reason,
                uncertainty,
            };
            assert!(record.is_consistent(), "{record:?}");
            assert_eq!(project(record), record);
            assert_eq!(canonical(record), record);
            let mut reply = json!({"job":{"classification":record}});
            assert!(!carries_container(&reply));
            let before = reply.clone();
            project_reply(&mut reply);
            assert_eq!(reply, before);
        }
    }

    #[test]
    fn reply_projection_reaches_every_result_shape() {
        let record = json!(container());
        let mut reply = json!({
            "job": {"classification": record},
            "jobs": [{"classification": record}, {"locality": "REMOTE"}],
            "classification": record,
            "locality": "LOCAL_CONTAINER",
            "note": "LOCAL_CONTAINER",
        });
        assert!(carries_container(&reply));
        project_reply(&mut reply);
        assert!(!carries_container(&reply));
        assert_eq!(reply["job"]["classification"]["locality"], "HOST_LOCAL");
        assert_eq!(reply["jobs"][0]["classification"]["locality"], "HOST_LOCAL");
        assert_eq!(reply["jobs"][1]["locality"], "REMOTE");
        assert_eq!(reply["classification"]["locality"], "HOST_LOCAL");
        assert_eq!(reply["locality"], "HOST_LOCAL");
        assert_eq!(reply["note"], "LOCAL_CONTAINER");
        assert!(!carries_container(&json!("LOCAL_CONTAINER")));
    }

    #[test]
    fn only_the_named_operations_negotiate() {
        assert!(negotiated(OPERATION_CLASSIFY) && negotiated(OPERATION_JOB_STATUS));
        for earlier in [
            crate::OPERATION_JOB_CLASSIFY,
            crate::locality::OPERATION_CLASSIFY,
            crate::jobs::OPERATION_JOB_STATUS,
            crate::jobs::OPERATION_JOB_STATUS_LOCALITY,
            crate::jobs::OPERATION_JOB_START_LOCALITY,
            crate::traffic::endpoint::OPERATION_JOB,
            crate::traffic::diagnostics::OPERATION_JOB,
        ] {
            assert!(!negotiated(earlier), "{earlier}");
        }
    }
}

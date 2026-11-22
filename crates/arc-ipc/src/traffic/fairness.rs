//! Negotiation carrier for project fair queuing (AT-TRF-08).
//!
//! The capability gates two additions and nothing else. A client sends the
//! optional `project` field of `JobRequest::Enqueue` only after the daemon
//! advertised [`CAPABILITY`]; an absent key is omitted, so protocol-1 bytes
//! are unchanged. The raw recorded reason crosses the wire only in a reply to
//! [`OPERATION_DECISION`]. Protocol-1 replies, v1 diagnostics and policy
//! replies keep `arc_policy::DecisionReason`, which cannot name a fairness
//! reason. These declarations enable no daemon handler.
use crate::{
    jobs::JobId, Request, Response, ERROR_INVALID_REQUEST, ERROR_UNSUPPORTED_CAPABILITY,
    PROTOCOL_VERSION, STATUS_ERROR, STATUS_OK,
};
pub use arc_policy::{FairnessReason, NegotiatedReason};
use serde::{Deserialize, Serialize};
use std::fmt;

pub const CAPABILITY: &str = "traffic.fairness.v1";
pub const OPERATION_CAPABILITIES: &str = "traffic.fairness.capabilities";
/// Opt-in read of the last recorded reason, including additive vocabulary.
pub const OPERATION_DECISION: &str = "traffic.fairness.decision";
pub const MAX_QUERY_BYTES: usize = 512;
/// The daemon's standing advertisement of [`CAPABILITY`]: a file beside its
/// socket on which it holds a POSIX write lock for its whole lifetime. A
/// client that has authenticated the daemon reads the lock holder with
/// `F_GETLK` and compares it with that peer, so a wrapper learns the
/// capability without another request before its enqueue, and a file left by
/// an earlier daemon advertises nothing. The file holds no data.
pub const ADVERTISEMENT: &str = ".traffic-fairness-v1";

impl super::Capabilities {
    pub fn fairness_supported() -> Self {
        Self {
            capability: CAPABILITY.to_owned(),
        }
    }
}

/// An opaque project identity: 64 lowercase hexadecimal digits of a digest
/// computed by the client. Never a path, a name or reversible text.
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ProjectKey(String);

impl ProjectKey {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for ProjectKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ProjectKey([OPAQUE])")
    }
}
impl TryFrom<String> for ProjectKey {
    type Error = &'static str;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.len() == 64
            && value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            Ok(Self(value))
        } else {
            Err("invalid project key")
        }
    }
}
impl From<ProjectKey> for String {
    fn from(value: ProjectKey) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionRequest {
    pub job_id: JobId,
}

/// The last recorded reason of one job. `None` when no decision is recorded,
/// available or retained; a failed read is an error, never `None`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionResult {
    pub schema_version: u32,
    pub job_id: JobId,
    #[serde(deserialize_with = "crate::jobs::required_option")]
    pub reason: Option<NegotiatedReason>,
}

impl DecisionResult {
    pub fn is_valid(&self) -> bool {
        self.schema_version == 1
    }
}

pub fn decode_request(payload: &serde_json::Value) -> Result<DecisionRequest, &'static str> {
    crate::policy::decode(payload, MAX_QUERY_BYTES)
}

pub fn decode_result(
    payload: &serde_json::Value,
    job_id: &JobId,
) -> Result<DecisionResult, &'static str> {
    let result: DecisionResult = crate::policy::decode(payload, MAX_QUERY_BYTES)?;
    (result.job_id == *job_id && result.is_valid())
        .then_some(result)
        .ok_or(ERROR_INVALID_REQUEST)
}

/// Validate a reply to the opt-in read. Only the explicit unsupported reply
/// lets a reader fall back to the projected v1 reason.
pub fn interpret_decision(
    request: &Request,
    response: &Response,
) -> Result<DecisionResult, &'static str> {
    if request.operation != OPERATION_DECISION {
        return Err(ERROR_UNSUPPORTED_CAPABILITY);
    }
    if request.protocol_version != PROTOCOL_VERSION || response.protocol_version != PROTOCOL_VERSION
    {
        return Err(crate::ERROR_PROTOCOL_INCOMPATIBLE);
    }
    let query = decode_request(&request.payload)?;
    if request.request_id != response.request_id {
        return Err(ERROR_INVALID_REQUEST);
    }
    if response.status == STATUS_ERROR
        && response.result.is_none()
        && response
            .error
            .as_ref()
            .is_some_and(|error| error.code == ERROR_UNSUPPORTED_CAPABILITY)
    {
        return Err(ERROR_UNSUPPORTED_CAPABILITY);
    }
    if response.status != STATUS_OK || response.error.is_some() {
        return Err(ERROR_INVALID_REQUEST);
    }
    decode_result(
        response.result.as_ref().ok_or(ERROR_INVALID_REQUEST)?,
        &query.job_id,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traffic::{decode_job, interpret_probe_for, JobRequest, ProbeResult};
    use serde_json::{json, Value};

    const ID: &str = "0123456789abcdef0123456789abcdef";
    const KEY: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";
    /// Every `DecisionReason` a protocol-1 peer and the desktop reader know.
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

    fn protocol_one_enqueue() -> Value {
        json!({"action":"ENQUEUE","job_id":ID,
            "observer":{"pid":1,"start_token":"1","boot_id":"01234567-89ab-cdef-0123-456789abcdef"},
            "classification":{"classification":"BUILD","locality":"HOST_LOCAL",
                "reason":"CARGO_BUILD","uncertainty":"NONE"},
            "lineage_context":null})
    }

    /// The exact bytes a protocol-1 client writes today, in field order.
    const PROTOCOL_ONE_BYTES: &str = concat!(
        r#"{"action":"ENQUEUE","job_id":"0123456789abcdef0123456789abcdef","#,
        r#""observer":{"pid":1,"start_token":"1","boot_id":"01234567-89ab-cdef-0123-456789abcdef"},"#,
        r#""classification":{"classification":"BUILD","locality":"HOST_LOCAL","#,
        r#""reason":"CARGO_BUILD","uncertainty":"NONE"},"lineage_context":null}"#
    );

    #[test]
    fn absent_project_leaves_the_enqueue_request_bytes_unchanged() {
        let wire: Value = serde_json::from_str(PROTOCOL_ONE_BYTES).unwrap();
        assert_eq!(wire, protocol_one_enqueue());
        let request = decode_job(&wire).unwrap();
        let JobRequest::Enqueue { project, .. } = &request else {
            panic!("{request:?}");
        };
        assert_eq!(*project, None);
        assert_eq!(serde_json::to_string(&request).unwrap(), PROTOCOL_ONE_BYTES);
    }

    #[test]
    fn project_key_is_an_opaque_digest_and_round_trips_on_enqueue() {
        let mut wire = protocol_one_enqueue();
        wire["project"] = json!(KEY);
        let request = decode_job(&wire).unwrap();
        let JobRequest::Enqueue { project, .. } = &request else {
            panic!("{request:?}");
        };
        assert_eq!(project.as_ref().map(ProjectKey::as_str), Some(KEY));
        assert_eq!(json!(request), wire);
        assert_eq!(
            format!("{:?}", project.as_ref().unwrap()),
            "ProjectKey([OPAQUE])"
        );
        for invalid in [
            json!("/Users/someone/project"),
            json!(KEY.to_uppercase()),
            json!(&KEY[..63]),
            json!(format!("{KEY}0")),
            json!(""),
            json!(7),
        ] {
            let mut wire = protocol_one_enqueue();
            wire["project"] = invalid.clone();
            assert!(decode_job(&wire).is_err(), "{invalid}");
        }
    }

    #[test]
    fn v1_projection_serializes_only_existing_vocabulary() {
        for reason in FairnessReason::ALL {
            let raw = json!(reason);
            assert!(!EXISTING.contains(&raw.as_str().unwrap()), "{raw}");
            let (kind, projected) = reason.projection();
            assert!(EXISTING.contains(&json!(projected).as_str().unwrap()));
            assert_eq!(json!(kind), json!("admit"));
            assert_eq!(NegotiatedReason::Fairness(reason).projected(), projected);
            // The v1 diagnostics and policy reply types cannot decode the raw
            // reason, so a reply built from them cannot carry it.
            assert!(serde_json::from_value::<arc_policy::DecisionReason>(raw.clone()).is_err());
            // Only the opt-in read names it.
            let result = DecisionResult {
                schema_version: 1,
                job_id: JobId::try_from(ID.to_owned()).unwrap(),
                reason: Some(NegotiatedReason::Fairness(reason)),
            };
            let wire = json!(result);
            assert_eq!(wire["reason"], raw);
            assert_eq!(decode_result(&wire, &result.job_id).unwrap(), result);
        }
        for tag in EXISTING {
            let wire = json!({"schema_version":1,"job_id":ID,"reason":tag});
            let result = decode_result(&wire, &JobId::try_from(ID.to_owned()).unwrap()).unwrap();
            assert_eq!(json!(result.reason.unwrap().projected()), json!(tag));
        }
    }

    #[test]
    fn decision_read_is_bound_to_its_operation_job_and_schema() {
        let job = JobId::try_from(ID.to_owned()).unwrap();
        let mut request = Request::new("fairness", OPERATION_DECISION);
        request.payload = json!(DecisionRequest {
            job_id: job.clone()
        });
        let none = json!({"schema_version":1,"job_id":ID,"reason":null});
        let ok = Response::ok(&request.request_id, none.clone());
        assert_eq!(interpret_decision(&request, &ok).unwrap().reason, None);
        let unsupported = Response::error(&request.request_id, ERROR_UNSUPPORTED_CAPABILITY);
        assert_eq!(
            interpret_decision(&request, &unsupported),
            Err(ERROR_UNSUPPORTED_CAPABILITY)
        );
        for invalid in [
            json!({"schema_version":2,"job_id":ID,"reason":null}),
            json!({"schema_version":1,"job_id":ID}),
            json!({"schema_version":1,"job_id":"f".repeat(32),"reason":null}),
            json!({"schema_version":1,"job_id":ID,"reason":"not_a_reason"}),
            json!({"schema_version":1,"job_id":ID,"reason":null,"path":"/tmp"}),
        ] {
            let response = Response::ok(&request.request_id, invalid.clone());
            assert!(
                interpret_decision(&request, &response).is_err(),
                "{invalid}"
            );
        }
        let mut other = request.clone();
        other.operation = super::super::diagnostics::OPERATION_JOB.into();
        assert_eq!(
            interpret_decision(&other, &ok),
            Err(ERROR_UNSUPPORTED_CAPABILITY)
        );
    }

    #[test]
    fn capability_probe_is_selected_locally_and_named_once() {
        assert_eq!(
            json!(crate::traffic::Capabilities::fairness_supported()),
            json!({"capability":"traffic.fairness.v1"})
        );
        let request = Request::new("probe", OPERATION_CAPABILITIES);
        let supported = Response::ok(&request.request_id, json!({"capability":CAPABILITY}));
        let probe = |response: &Response| {
            interpret_probe_for(&request, response, OPERATION_CAPABILITIES, CAPABILITY)
        };
        assert_eq!(probe(&supported), ProbeResult::Supported);
        let earlier = Response::error(&request.request_id, ERROR_UNSUPPORTED_CAPABILITY);
        assert_eq!(probe(&earlier), ProbeResult::Unsupported);
        let other = Response::ok(&request.request_id, json!({"capability":"traffic.v1"}));
        assert_eq!(probe(&other), ProbeResult::Invalid);
    }
}

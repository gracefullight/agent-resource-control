//! Additive stage linkage: which job records one hook or exec request started.
//!
//! The operation is the capability: an earlier daemon answers
//! `UNSUPPORTED_CAPABILITY` and the stage runs unlinked. A link is metadata
//! only. It never admits, reserves, delays or launches work, and it carries
//! no command, path or environment text.
use crate::jobs::{JobId, ProcessIdentity};
use serde::{Deserialize, Serialize};

pub const OPERATION: &str = "job.request.v1";
/// One request names at most this many stage records.
pub const MAX_STAGES: usize = 16;

/// Same alphabet as a job id. On the `arcctl exec` route it is the job id of
/// the shell that remains the request record; the hook generates its own.
pub type RequestId = JobId;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "action",
    rename_all = "SCREAMING_SNAKE_CASE",
    deny_unknown_fields
)]
pub enum StageRequest {
    /// Sent by the stage's own wrapper after its job is registered.
    Link {
        request_id: RequestId,
        job_id: JobId,
        observer: ProcessIdentity,
    },
    /// Read the stage records of one request in link order.
    Stages { request_id: RequestId },
}
impl StageRequest {
    pub fn is_valid(&self) -> bool {
        match self {
            // A request record is never a stage of itself.
            Self::Link {
                request_id,
                job_id,
                observer,
            } => observer.is_valid() && request_id != job_id,
            Self::Stages { .. } => true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StagesResult {
    pub request_id: RequestId,
    pub jobs: Vec<JobId>,
}
impl StagesResult {
    pub fn is_valid(&self) -> bool {
        self.jobs.len() <= MAX_STAGES
            && !self.jobs.contains(&self.request_id)
            && self
                .jobs
                .iter()
                .enumerate()
                .all(|(index, job)| !self.jobs[..index].contains(job))
    }
}

/// Bounded like every lifecycle payload; decode failures are static.
pub fn decode(payload: &serde_json::Value) -> Result<StageRequest, &'static str> {
    let request: StageRequest = crate::jobs::decode_payload(payload)?;
    request
        .is_valid()
        .then_some(request)
        .ok_or(crate::ERROR_INVALID_REQUEST)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::LifecycleRequest;
    use serde_json::json;

    fn id(n: u32) -> JobId {
        JobId::try_from(format!("{n:032x}")).unwrap()
    }

    #[test]
    fn stage_link_wire_shape_is_closed_and_routed_as_a_lifecycle_request() {
        let observer = ProcessIdentity::capture(std::process::id()).unwrap();
        let link = StageRequest::Link {
            request_id: id(1),
            job_id: id(2),
            observer: observer.clone(),
        };
        let wire = json!(link);
        assert_eq!(wire["action"], "LINK");
        assert_eq!(wire["request_id"], id(1).as_str());
        assert_eq!(decode(&wire), Ok(link.clone()));
        assert!(matches!(
            LifecycleRequest::decode(OPERATION, &wire),
            Some(Ok(LifecycleRequest::Request(decoded))) if decoded == link
        ));
        let read = json!({"action":"STAGES","request_id":id(1)});
        assert_eq!(
            decode(&read),
            Ok(StageRequest::Stages { request_id: id(1) })
        );

        let mut extra = wire.clone();
        extra["command"] = json!("bun test");
        let own = json!(StageRequest::Link {
            request_id: id(2),
            job_id: id(2),
            observer,
        });
        for rejected in [
            extra,
            own,
            json!({"action":"STAGES","request_id":"not-a-request"}),
            json!({"action":"STAGES"}),
            json!([]),
        ] {
            assert_eq!(
                decode(&rejected),
                Err(crate::ERROR_INVALID_REQUEST),
                "{rejected}"
            );
        }
    }

    #[test]
    fn stage_result_is_bounded_and_names_each_stage_once() {
        let result = |jobs: Vec<JobId>| StagesResult {
            request_id: id(1),
            jobs,
        };
        assert!(result(vec![]).is_valid());
        assert!(result(vec![id(2), id(3)]).is_valid());
        assert!(!result(vec![id(2), id(2)]).is_valid());
        assert!(!result(vec![id(1)]).is_valid());
        assert!(result((2..2 + MAX_STAGES as u32).map(id).collect()).is_valid());
        assert!(!result((2..3 + MAX_STAGES as u32).map(id).collect()).is_valid());
    }
}

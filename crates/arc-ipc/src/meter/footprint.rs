//! Negotiated job footprint (lane B: AT-MTR-02, G-01 item 4).
//!
//! At job start (after a committed registration, before the child is
//! spawned) and at job end (after a committed completion) the daemon
//! measures the allocated bytes (`st_blocks * 512`) under the job's working
//! directory: a bounded walk that stays on one device and follows no
//! symlink. `end` is the Current Footprint (what remains); `delta_bytes` is
//! `end - start`, defined only when both readings are complete. Neither is
//! ever derived from Disk Written, the cumulative written bytes of the
//! process-tree report, which stays its own field.
//!
//! The v1 `meter.summary` and `meter.job_tree` replies are unchanged:
//! `arcctl meter summary --job <ID> --footprint --json` adds the reply to
//! [`OPERATION_JOB_FOOTPRINT`] under [`SUMMARY_KEY`] only after the daemon
//! advertised [`CAPABILITY`] by holding [`ADVERTISEMENT`]. No path is sent
//! or stored.
use super::quality::Flagged;
use crate::{jobs::JobId, ERROR_INVALID_REQUEST};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CAPABILITY: &str = "meter.job-footprint.v1";
pub const OPERATION_JOB_FOOTPRINT: &str = "meter.job_footprint";
pub const ADVERTISEMENT: &str = ".meter-job-footprint-v1";
pub const SUMMARY_KEY: &str = "footprint";
pub const SCHEMA_VERSION: u32 = 1;
pub const BASIS_ALLOCATED: &str = "allocated";
pub const MAX_JOB_FOOTPRINT_BYTES: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobFootprintRequest {
    pub job_id: JobId,
}

/// Each reading has basis [`BASIS_ALLOCATED`]; a walk that hit its bound is
/// `partial` (a lower bound), one that could not run is `unavailable`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobFootprint {
    pub start_bytes: Flagged<u64>,
    /// Current Footprint: allocated bytes remaining at job end.
    pub end_bytes: Flagged<u64>,
    /// `end - start`; unavailable unless both readings are complete.
    pub delta_bytes: Flagged<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobFootprintReply {
    pub schema_version: u32,
    pub job_id: JobId,
    pub footprint: JobFootprint,
}

impl JobFootprintReply {
    pub fn answers(&self, job: &JobId) -> bool {
        let f = &self.footprint;
        self.schema_version == SCHEMA_VERSION
            && self.job_id == *job
            && [
                &f.start_bytes.basis,
                &f.end_bytes.basis,
                &f.delta_bytes.basis,
            ]
            .iter()
            .all(|basis| basis.as_str() == BASIS_ALLOCATED)
            && f.start_bytes.is_consistent()
            && f.end_bytes.is_consistent()
            && f.delta_bytes.is_consistent()
    }
}

pub fn decode_request(payload: &Value) -> Result<JobFootprintRequest, &'static str> {
    if !payload.is_object() || payload.to_string().len() > 256 {
        return Err(ERROR_INVALID_REQUEST);
    }
    JobFootprintRequest::deserialize(payload).map_err(|_| ERROR_INVALID_REQUEST)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_reply_answers_only_its_job_with_allocated_flagged_readings() {
        let job = JobId::try_from("a".repeat(32)).unwrap();
        let missing = || Flagged::unavailable(BASIS_ALLOCATED, "not_measured");
        let reply = JobFootprintReply {
            schema_version: SCHEMA_VERSION,
            job_id: job.clone(),
            footprint: JobFootprint {
                start_bytes: missing(),
                end_bytes: missing(),
                delta_bytes: Flagged::unavailable(BASIS_ALLOCATED, "not_measured"),
            },
        };
        assert!(reply.answers(&job));
        assert!(!reply.answers(&JobId::try_from("b".repeat(32)).unwrap()));
        let mut written = reply.clone();
        written.footprint.delta_bytes.basis = "disk_written".into();
        assert!(!written.answers(&job));
        assert!(decode_request(&json!({"job_id": "a".repeat(32)})).is_ok());
        assert!(decode_request(&json!({"job_id": "a".repeat(32), "path": "/"})).is_err());
    }
}

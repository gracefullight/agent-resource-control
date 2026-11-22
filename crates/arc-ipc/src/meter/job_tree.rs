//! Negotiated job process-tree report (lane A, MTR-002/MTR-003/MET-002..010).
//!
//! The v1 `meter.summary` reply stays byte-identical: tree-sampled values
//! travel only in a reply to [`OPERATION_JOB_TREE`], requested by `arcctl`
//! after the daemon advertised [`CAPABILITY`] by holding [`ADVERTISEMENT`].
//! `arcctl meter summary --job --json` merges that reply under the one
//! top-level key [`SUMMARY_KEY`]. The desktop reader never requests it.
use crate::{jobs::JobId, ERROR_INVALID_REQUEST};
pub use arc_core::job_meter::{Completeness, MissingCoverageReason};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CAPABILITY: &str = "meter.job-tree.v1";
pub const OPERATION_JOB_TREE: &str = "meter.job_tree";
/// Standing advertisement beside the socket; see `traffic::fairness::ADVERTISEMENT`.
pub const ADVERTISEMENT: &str = ".meter-job-tree-v1";
/// The key `arcctl meter summary --job --json` adds beside the v1 summary.
pub const SUMMARY_KEY: &str = "process_tree";
pub const SCHEMA_VERSION: u32 = 1;
pub const MAX_JOB_TREE_BYTES: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobTreeRequest {
    pub job_id: JobId,
}

/// How one value was obtained. A value that cannot be observed is `None`
/// with basis `unobserved`, never a silent zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TreeBasis {
    /// The root's own waited rusage (attribution A).
    WaitedRusage,
    /// Identity-revalidated process-group and ppid samples (attribution C, Estimated).
    TreeSamples,
    Unobserved,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TreeValue<T> {
    pub value: Option<T>,
    pub basis: TreeBasis,
}

/// The tree report stored in `job_meter_summaries.process_tree_json`: a
/// running snapshot on each Meter pass, then the terminal report once the job
/// left RUNNING (`terminal`), completed with the root's waited usage when that
/// v1 report is recorded.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProcessTreeReport {
    pub schema_version: u32,
    pub job_id: String,
    pub terminal: bool,
    /// Job CPU including descendants the wrapper does not wait for: the
    /// waited usage (or, until it is recorded, the root's last sample) plus
    /// `unwaited_cpu_core_seconds`. Basis `tree_samples` (Estimated).
    pub cpu_core_seconds: TreeValue<f64>,
    /// The root's waited rusage from the v1 job Meter report.
    pub waited_cpu_core_seconds: TreeValue<f64>,
    /// The root's last sampled own and reaped-children CPU.
    pub root_sampled_cpu_core_seconds: TreeValue<f64>,
    /// Last sampled CPU of descendants no tree process can reap.
    pub unwaited_cpu_core_seconds: TreeValue<f64>,
    /// `job.memory.peak_bytes`: the highest summed RSS of one sample.
    pub memory_peak_bytes: TreeValue<u64>,
    /// Summed last disk read bytes of every sampled process.
    pub disk_read_bytes: TreeValue<u64>,
    /// Summed cumulative disk bytes written by every sampled process.
    pub disk_write_bytes: TreeValue<u64>,
    pub completeness: Completeness,
    pub missing_coverage: Vec<MissingCoverageReason>,
    /// Meter passes that sampled this job.
    pub samples: u64,
    /// Distinct process identities sampled.
    pub processes: u64,
}

/// `process_tree` is `None` when no tree report is recorded for the job.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobTreeReply {
    pub schema_version: u32,
    pub job_id: JobId,
    pub process_tree: Option<ProcessTreeReport>,
}

pub fn decode_request(payload: &Value) -> Result<JobTreeRequest, &'static str> {
    if !payload.is_object() || payload.to_string().len() > 512 {
        return Err(ERROR_INVALID_REQUEST);
    }
    JobTreeRequest::deserialize(payload).map_err(|_| ERROR_INVALID_REQUEST)
}

pub fn decode_reply(value: &Value, job: &JobId) -> Result<JobTreeReply, &'static str> {
    if !value.is_object() || value.to_string().len() > MAX_JOB_TREE_BYTES {
        return Err(ERROR_INVALID_REQUEST);
    }
    let reply = JobTreeReply::deserialize(value).map_err(|_| ERROR_INVALID_REQUEST)?;
    (reply.schema_version == SCHEMA_VERSION && reply.job_id == *job)
        .then_some(reply)
        .ok_or(ERROR_INVALID_REQUEST)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn requests_and_replies_are_bound_to_their_job_and_schema() {
        let job = JobId::try_from("a".repeat(32)).unwrap();
        assert_eq!(
            decode_request(&json!({"job_id": "a".repeat(32)})).unwrap(),
            JobTreeRequest {
                job_id: job.clone()
            }
        );
        for payload in [
            json!(null),
            json!({}),
            json!({"job_id": "a".repeat(32), "pid": 1}),
        ] {
            assert_eq!(decode_request(&payload), Err(ERROR_INVALID_REQUEST));
        }
        let reply = json!({"schema_version": 1, "job_id": "a".repeat(32), "process_tree": null});
        assert_eq!(decode_reply(&reply, &job).unwrap().process_tree, None);
        let other = JobId::try_from("b".repeat(32)).unwrap();
        assert!(decode_reply(&reply, &other).is_err());
        let newer = json!({"schema_version": 2, "job_id": "a".repeat(32), "process_tree": null});
        assert!(decode_reply(&newer, &job).is_err());
    }

    #[test]
    fn an_unobserved_value_serializes_as_null_with_its_basis() {
        let value: TreeValue<u64> = TreeValue {
            value: None,
            basis: TreeBasis::Unobserved,
        };
        assert_eq!(json!(value), json!({"value": null, "basis": "unobserved"}));
        assert_eq!(json!(TreeBasis::WaitedRusage), json!("waited_rusage"));
        assert_eq!(json!(TreeBasis::TreeSamples), json!("tree_samples"));
    }
}

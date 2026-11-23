//! Individually flagged process-tree values, negotiated separately from v1.
use super::{
    job_tree::{Completeness, ProcessTreeReport, TreeBasis, TreeValue},
    quality::{Availability, Flagged},
};
use crate::jobs::JobId;
use serde::{Deserialize, Serialize};

pub const CAPABILITY: &str = "meter.job-tree-quality.v1";
pub const ADVERTISEMENT: &str = ".meter-job-tree-quality-v1";
pub const OPERATION: &str = "meter.job_tree_quality";
pub const SUMMARY_KEY: &str = "process_tree_quality";
pub const SCHEMA_VERSION: u32 = 1;
pub const MAX_BYTES: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Values {
    pub cpu_core_seconds: Flagged<f64>,
    pub waited_cpu_core_seconds: Flagged<f64>,
    pub root_sampled_cpu_core_seconds: Flagged<f64>,
    pub unwaited_cpu_core_seconds: Flagged<f64>,
    pub memory_peak_bytes: Flagged<u64>,
    pub disk_read_bytes: Flagged<u64>,
    pub disk_write_bytes: Flagged<u64>,
}

/// One snapshot: `process_tree` is the very v1 report (or `null`) whose
/// values `process_tree_quality` flags, so the two can never disagree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reply {
    pub schema_version: u32,
    pub job_id: JobId,
    pub process_tree: Option<ProcessTreeReport>,
    pub process_tree_quality: Values,
}

fn flagged<T: Copy>(value: Option<&TreeValue<T>>, completeness: Completeness) -> Flagged<T> {
    let Some(value) = value else {
        return Flagged::unavailable("unobserved", "not_recorded");
    };
    let basis = match value.basis {
        TreeBasis::WaitedRusage => "waited_rusage",
        TreeBasis::TreeSamples => "tree_samples",
        TreeBasis::Unobserved => "unobserved",
    };
    let Some(measured) = value.value else {
        return Flagged::unavailable(basis, "not_observed");
    };
    if value.basis == TreeBasis::Unobserved {
        return Flagged::unavailable(basis, "not_observed");
    }
    Flagged {
        value: Some(measured),
        availability: if completeness == Completeness::Complete
            && value.basis == TreeBasis::TreeSamples
        {
            Availability::Complete
        } else {
            Availability::Partial
        },
        basis: basis.to_owned(),
        reason: None,
    }
}

fn valid<T>(value: &Flagged<T>) -> bool {
    value.is_consistent()
        && matches!(
            value.basis.as_str(),
            "unobserved" | "tree_samples" | "waited_rusage"
        )
        && !(value.value.is_some() && value.basis == "unobserved")
        && value
            .reason
            .as_ref()
            .is_none_or(|reason| !reason.is_empty())
}

impl Reply {
    pub fn from_report(job_id: JobId, report: Option<&ProcessTreeReport>) -> Self {
        // Neither waited rusage nor the v1 tree's completeness proves that an
        // unwaited child did not run and exit between samples. Preserve the
        // v1 report, but project every sampled CPU value as a lower bound.
        let field = |get: fn(&ProcessTreeReport) -> &TreeValue<f64>| {
            flagged(report.map(get), Completeness::Partial)
        };
        // Peaks and I/O can also miss processes or spikes between samples.
        let integer = |get: fn(&ProcessTreeReport) -> &TreeValue<u64>| {
            flagged(report.map(get), Completeness::Partial)
        };
        Self {
            schema_version: SCHEMA_VERSION,
            job_id,
            process_tree: report.cloned(),
            process_tree_quality: Values {
                cpu_core_seconds: field(|r| &r.cpu_core_seconds),
                waited_cpu_core_seconds: field(|r| &r.waited_cpu_core_seconds),
                root_sampled_cpu_core_seconds: field(|r| &r.root_sampled_cpu_core_seconds),
                unwaited_cpu_core_seconds: field(|r| &r.unwaited_cpu_core_seconds),
                memory_peak_bytes: integer(|r| &r.memory_peak_bytes),
                disk_read_bytes: integer(|r| &r.disk_read_bytes),
                disk_write_bytes: integer(|r| &r.disk_write_bytes),
            },
        }
    }

    pub fn answers(&self, job: &JobId) -> bool {
        let v = &self.process_tree_quality;
        self.schema_version == SCHEMA_VERSION
            && self.job_id == *job
            && *self == Self::from_report(job.clone(), self.process_tree.as_ref())
            && [
                &v.cpu_core_seconds,
                &v.waited_cpu_core_seconds,
                &v.root_sampled_cpu_core_seconds,
                &v.unwaited_cpu_core_seconds,
            ]
            .iter()
            .all(|v| valid(v))
            && [
                &v.memory_peak_bytes,
                &v.disk_read_bytes,
                &v.disk_write_bytes,
            ]
            .iter()
            .all(|v| valid(v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absent_tree_values_have_reasons_and_invalid_flags_are_rejected() {
        let job = JobId::try_from("a".repeat(32)).unwrap();
        let mut reply = Reply::from_report(job.clone(), None);
        assert!(reply.answers(&job));
        assert_eq!(
            reply.process_tree_quality.disk_read_bytes.reason.as_deref(),
            Some("not_recorded")
        );
        reply.process_tree_quality.disk_read_bytes.reason = None;
        assert!(!reply.answers(&job));
        reply.process_tree_quality.disk_read_bytes = Flagged {
            value: Some(0),
            availability: Availability::Complete,
            basis: "unobserved".into(),
            reason: None,
        };
        assert!(!reply.answers(&job));
    }

    #[test]
    fn a_legacy_complete_tree_does_not_prove_sampled_cpu_completeness() {
        let job = JobId::try_from("a".repeat(32)).unwrap();
        let report: ProcessTreeReport = serde_json::from_value(serde_json::json!({
            "schema_version": 1, "job_id": job.as_str(), "terminal": true,
            "cpu_core_seconds": {"value": 1.5, "basis": "tree_samples"},
            "waited_cpu_core_seconds": {"value": 1.5, "basis": "waited_rusage"},
            "root_sampled_cpu_core_seconds": {"value": 1.5, "basis": "tree_samples"},
            "unwaited_cpu_core_seconds": {"value": 0.0, "basis": "tree_samples"},
            "memory_peak_bytes": {"value": 16, "basis": "tree_samples"},
            "disk_read_bytes": {"value": 8, "basis": "tree_samples"},
            "disk_write_bytes": {"value": 4, "basis": "tree_samples"},
            "completeness": "complete", "missing_coverage": [], "samples": 2, "processes": 1
        }))
        .unwrap();
        let reply = Reply::from_report(job.clone(), Some(&report));
        assert!(reply.answers(&job));
        assert_eq!(reply.process_tree.as_ref(), Some(&report));
        let v = &reply.process_tree_quality;
        for value in [
            &v.cpu_core_seconds,
            &v.unwaited_cpu_core_seconds,
            &v.root_sampled_cpu_core_seconds,
            &v.waited_cpu_core_seconds,
        ] {
            assert_eq!(value.availability, Availability::Partial);
            assert!(value.value.is_some());
        }
        for value in [
            &v.memory_peak_bytes,
            &v.disk_read_bytes,
            &v.disk_write_bytes,
        ] {
            assert_eq!(value.availability, Availability::Partial);
            assert!(value.value.is_some());
        }
        // A reply whose flags do not derive from its own tree is rejected.
        let mut split = reply.clone();
        split.process_tree.as_mut().unwrap().disk_write_bytes.value = Some(5);
        assert!(!split.answers(&job));
    }
}

use super::*;
use crate::meter::MAX_SAFE_INTEGER;

/// Same canonical opaque ID grammar as protocol-1 lifecycle JobId. IDs contain
/// no argv, paths, vendor name, project identity or client-provided diagnostics.
pub fn valid_id(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn seconds(value: Option<f64>) -> bool {
    value.is_none_or(|v| v.is_finite() && v >= 0.0 && v <= MAX_SAFE_INTEGER as f64)
}
impl CpuCoreSeconds {
    pub fn is_valid(&self) -> bool {
        if ![self.user, self.system, self.total]
            .into_iter()
            .all(seconds)
        {
            return false;
        }
        match (self.user, self.system, self.total) {
            (Some(user), Some(system), Some(total)) => {
                (user + system - total).abs() <= 1e-9_f64.max(total * f64::EPSILON * 2.0)
            }
            (_, _, None) => self.user.is_none() || self.system.is_none(),
            _ => false,
        }
    }
}
impl QueueWait {
    pub fn from_wall_timestamps(queued_at_ms: Option<u64>, reserved_at_ms: Option<u64>) -> Self {
        match (queued_at_ms, reserved_at_ms) {
            (Some(queued), Some(reserved))
                if queued <= reserved && reserved <= MAX_SAFE_INTEGER =>
            {
                Self {
                    seconds: Some((reserved - queued) as f64 / 1000.0),
                    clock: QueueClock::DaemonWallClock,
                    basis: QueueWaitBasis::QueuedToReserved,
                }
            }
            _ => Self::unknown(),
        }
    }
    pub fn unknown() -> Self {
        Self {
            seconds: None,
            clock: QueueClock::Unknown,
            basis: QueueWaitBasis::Unavailable,
        }
    }
    pub fn lifecycle_only() -> Self {
        Self {
            seconds: Some(0.0),
            clock: QueueClock::NotApplicable,
            basis: QueueWaitBasis::LifecycleOnly,
        }
    }
    pub fn is_valid(&self) -> bool {
        seconds(self.seconds)
            && match (self.basis, self.clock, self.seconds) {
                (QueueWaitBasis::QueuedToReserved, QueueClock::DaemonWallClock, Some(_)) => true,
                (QueueWaitBasis::LifecycleOnly, QueueClock::NotApplicable, Some(value)) => {
                    value == 0.0
                }
                (QueueWaitBasis::Unavailable, QueueClock::Unknown, None) => true,
                _ => false,
            }
    }
}
impl JobMeterReport {
    pub fn is_valid(&self) -> bool {
        let relation_valid = match &self.accounting_relation {
            AccountingRelation::CanonicalRoot { root_job_id } => {
                root_job_id == &self.job_id
                    && !self
                        .missing_coverage
                        .contains(&MissingCoverageReason::LineageUnverified)
            }
            AccountingRelation::IncludedInRoot { root_job_id } => {
                valid_id(root_job_id)
                    && root_job_id != &self.job_id
                    && !self
                        .missing_coverage
                        .contains(&MissingCoverageReason::LineageUnverified)
            }
            AccountingRelation::Unverified {} => {
                self.completeness != Completeness::Complete
                    && self
                        .missing_coverage
                        .contains(&MissingCoverageReason::LineageUnverified)
            }
        };
        let quality_valid = match self.completeness {
            Completeness::Complete => {
                self.missing_coverage.is_empty()
                    && self.cpu_core_seconds.total.is_some()
                    && self.run_elapsed.seconds.is_some()
            }
            Completeness::Partial => !self.missing_coverage.is_empty(),
            Completeness::Unknown => {
                !self.missing_coverage.is_empty()
                    && self.cpu_core_seconds
                        == (CpuCoreSeconds {
                            user: None,
                            system: None,
                            total: None,
                        })
            }
        };
        let has = |reason| self.missing_coverage.contains(&reason);
        let cpu_unavailable = has(MissingCoverageReason::NativeUsageUnavailable)
            || has(MissingCoverageReason::Interrupted)
            || has(MissingCoverageReason::Unsupported);
        let duration_unavailable = has(MissingCoverageReason::RunDurationUnavailable)
            || has(MissingCoverageReason::Interrupted)
            || has(MissingCoverageReason::Unsupported);
        self.schema_version == JOB_METER_SCHEMA_VERSION
            && valid_id(&self.job_id)
            && valid_id(&self.report_id)
            && self.observer.is_valid()
            && self.child.is_valid()
            && self.observer.pid != self.child.pid
            && self.observer.boot_id == self.child.boot_id
            && self.cpu_core_seconds.is_valid()
            && seconds(self.run_elapsed.seconds)
            && (self.cpu_core_seconds.total.is_some() || cpu_unavailable)
            && (self.run_elapsed.seconds.is_some() || duration_unavailable)
            && (!has(MissingCoverageReason::NativeUsageUnavailable)
                || self.cpu_core_seconds.total.is_none())
            && (!has(MissingCoverageReason::RunDurationUnavailable)
                || self.run_elapsed.seconds.is_none())
            && (!has(MissingCoverageReason::Unsupported) || self.cpu_core_seconds.total.is_none())
            && self.run_elapsed.includes_stopped
            && self.terminated_at_ms.is_none_or(|v| v <= MAX_SAFE_INTEGER)
            && self.missing_coverage.len() <= MAX_MISSING_COVERAGE_REASONS
            && self
                .missing_coverage
                .iter()
                .enumerate()
                .all(|(i, reason)| !self.missing_coverage[..i].contains(reason))
            && match self.outcome {
                TerminalOutcome::Exited { .. } => true,
                TerminalOutcome::Signaled { signal, .. } => (1..=64).contains(&signal),
            }
            && relation_valid
            && quality_valid
    }
}
impl JobMeterSummary {
    pub fn is_valid(&self) -> bool {
        self.schema_version == JOB_METER_SCHEMA_VERSION
            && valid_id(&self.job_id)
            && self.queue_wait.is_valid()
            && match &self.accounting {
                SummaryAccounting::Recorded { report } => {
                    report.job_id == self.job_id && report.is_valid()
                }
                SummaryAccounting::Unknown { .. } => true,
                SummaryAccounting::Missing {} | SummaryAccounting::Expired {} => {
                    self.queue_wait == QueueWait::unknown()
                }
            }
    }
}

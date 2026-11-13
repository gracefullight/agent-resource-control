//! Immutable terminal metadata. This module can never spawn or grant execution.
use super::{signals::Termination, transport::Failure};
use arc_core::{
    job_meter::*,
    process_identity::{random_job_id, ProcessIdentity},
};
use arc_ipc::jobs::JobId;
use std::{
    path::Path,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub(super) const REPORT_WINDOW: Duration = Duration::from_millis(500);

pub(super) fn lineage(
    socket: &Path,
    observer: &ProcessIdentity,
    job: &JobId,
    tracking: &super::traffic::Mode,
) -> (
    AccountingRelation,
    Option<arc_platform::job_lineage::Record>,
) {
    let confirmed = match tracking {
        super::traffic::Mode::Legacy(super::preparation::Tracking::Managed) => true,
        super::traffic::Mode::Scheduled { continuity, .. }
        | super::traffic::Mode::Unreserved { continuity, .. } => continuity.confirmed(),
        _ => false,
    };
    let relation = if confirmed {
        arc_platform::job_lineage::relation(socket, observer, job.as_str())
    } else {
        // A live native ancestor is insufficient when that ancestor's durable
        // registration/permit is unconfirmed. Its children must stay non-additive.
        AccountingRelation::Unverified {}
    };
    let record =
        arc_platform::job_lineage::Record::create(socket, observer, job.as_str(), &relation).ok();
    // Optional ancestry metadata must not veto fail-open execution. Failed
    // publication cannot support a canonical/additive accounting claim.
    let relation = if record.is_some() {
        relation
    } else {
        AccountingRelation::Unverified {}
    };
    (relation, record)
}

pub(super) fn report(
    job: &JobId,
    observer: &ProcessIdentity,
    child: Option<&ProcessIdentity>,
    relation: AccountingRelation,
    spawned_at: Instant,
    terminal: &Termination,
) -> Option<JobMeterReport> {
    use std::os::unix::process::ExitStatusExt;
    let child = child?.clone();
    let cpu = terminal.usage.map_or(
        CpuCoreSeconds {
            user: None,
            system: None,
            total: None,
        },
        |usage| CpuCoreSeconds {
            user: Some(usage.user_core_seconds),
            system: Some(usage.system_core_seconds),
            total: Some(usage.total_core_seconds),
        },
    );
    let elapsed = terminal
        .observed_at
        .checked_duration_since(spawned_at)
        .map(|v| v.as_secs_f64());
    // An opaque target cannot establish coverage of escaped/unwaited work.
    // Its actual waited contribution remains useful, explicitly Partial.
    let mut missing = vec![MissingCoverageReason::UnobservedDescendants];
    if cpu.total.is_none() {
        missing.push(MissingCoverageReason::NativeUsageUnavailable);
    }
    if elapsed.is_none() {
        missing.push(MissingCoverageReason::RunDurationUnavailable);
    }
    if matches!(relation, AccountingRelation::Unverified {}) {
        missing.push(MissingCoverageReason::LineageUnverified);
    }
    let outcome = match terminal.status.code() {
        Some(code) => TerminalOutcome::Exited { code: code as u8 },
        None => TerminalOutcome::Signaled {
            signal: terminal.status.signal()? as u8,
            core_dumped: terminal.status.core_dumped(),
        },
    };
    let report = JobMeterReport {
        schema_version: JOB_METER_SCHEMA_VERSION,
        report_id: random_job_id().ok()?,
        job_id: job.as_str().to_owned(),
        observer: observer.clone(),
        child,
        cpu_core_seconds: cpu,
        run_elapsed: RunElapsed {
            seconds: elapsed,
            clock: RunClock::Monotonic,
            includes_stopped: true,
        },
        attribution_basis: AttributionBasis::NativeOwnedChildWait4,
        scope: AccountingScope::NativeWaitedDescendants,
        completeness: if terminal.usage.is_some() {
            Completeness::Partial
        } else {
            Completeness::Unknown
        },
        missing_coverage: missing,
        accounting_relation: relation,
        outcome,
        terminated_at_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .and_then(|d| d.as_millis().try_into().ok()),
    };
    report.is_valid().then_some(report)
}

/// Reserve time for read-after-uncertain-ACK reconciliation. Every write uses
/// the same request/report ID, exact payload and terminal-derived deadline.
pub(super) fn deliver(socket: &Path, report: &JobMeterReport, deadline: Instant) -> bool {
    deliver_with(
        report,
        deadline,
        |request_id, report, end, committed| {
            arc_ipc::meter::report_until(socket, request_id, report, end, &|| false, committed)
                .map(|_| ())
        },
        |job, end| {
            arc_ipc::meter::summary_until(
                socket,
                format!("job-meter-read-{}", report.report_id),
                job,
                end,
                &|| false,
            )
        },
    )
}

fn deliver_with(
    report: &JobMeterReport,
    deadline: Instant,
    mut write: impl FnMut(&str, &JobMeterReport, Instant, &mut bool) -> Result<(), Failure>,
    mut read: impl FnMut(&str, Instant) -> Result<JobMeterSummary, Failure>,
) -> bool {
    if Instant::now() >= deadline {
        return false;
    }
    let request_id = format!("job-meter-{}", report.report_id);
    let mut committed = false;
    let first_end = deadline.min(Instant::now() + Duration::from_millis(200));
    match write(&request_id, report, first_end, &mut committed) {
        Ok(()) if !committed => return true,
        Ok(()) => (),
        Err(Failure::Uncertain | Failure::Busy | Failure::InvalidReply) if committed => (),
        Err(_) => return false,
    }
    if Instant::now() >= deadline {
        return false;
    }
    match read(
        &report.job_id,
        deadline.min(Instant::now() + Duration::from_millis(150)),
    ) {
        Ok(summary) if summary.is_valid() && summary.job_id == report.job_id => {
            match summary.accounting {
                SummaryAccounting::Recorded { report: stored } => return *stored == *report,
                SummaryAccounting::Expired {} => return false,
                _ => (),
            }
        }
        Ok(_)
        | Err(
            Failure::Unsupported
            | Failure::Protocol
            | Failure::InvalidEndpoint
            | Failure::Rejected
            | Failure::Cancelled,
        ) => return false,
        Err(_) => (),
    }
    if Instant::now() >= deadline {
        return false;
    }
    write(&request_id, report, deadline, &mut committed).is_ok()
}

#[cfg(test)]
mod tests;

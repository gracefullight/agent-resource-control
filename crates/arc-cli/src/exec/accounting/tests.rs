use super::*;
use arc_platform::child_wait::{CpuTimeval, TerminalUsage};
use std::{cell::RefCell, os::unix::process::ExitStatusExt, process::ExitStatus};

fn terminal(usage: bool) -> Termination {
    let zero = CpuTimeval {
        seconds: 0,
        microseconds: 0,
    };
    Termination {
        status: ExitStatus::from_raw(17 << 8),
        observed_at: Instant::now(),
        usage: usage.then(|| TerminalUsage::from_timevals(zero, zero).unwrap()),
    }
}
fn fixture() -> JobMeterReport {
    serde_json::from_str(include_str!(
        "../../../../arc-core/tests/fixtures/job-meter-report.json"
    ))
    .unwrap()
}
fn summary(report: &JobMeterReport) -> JobMeterSummary {
    JobMeterSummary {
        schema_version: 1,
        job_id: report.job_id.clone(),
        accounting: SummaryAccounting::Recorded {
            report: Box::new(report.clone()),
        },
        queue_wait: QueueWait::lifecycle_only(),
    }
}
#[test]
fn terminal_builder_preserves_zero_unknown_scope_outcome_and_run_endpoint() {
    let source = fixture();
    let id = JobId::try_from(source.job_id.clone()).unwrap();
    let start = Instant::now() - Duration::from_millis(120);
    for known in [true, false] {
        let end = terminal(known);
        let report = report(
            &id,
            &source.observer,
            Some(&source.child),
            source.accounting_relation.clone(),
            start,
            &end,
        )
        .unwrap();
        assert!(report.is_valid());
        assert_eq!(report.outcome, TerminalOutcome::Exited { code: 17 });
        assert_eq!(report.scope, AccountingScope::NativeWaitedDescendants);
        assert_eq!(report.cpu_core_seconds.total, known.then_some(0.0));
        assert_eq!(
            report.completeness,
            if known {
                Completeness::Partial
            } else {
                Completeness::Unknown
            }
        );
        assert!(report
            .missing_coverage
            .contains(&MissingCoverageReason::UnobservedDescendants));
        assert_eq!(
            report.run_elapsed.seconds,
            Some(end.observed_at.duration_since(start).as_secs_f64())
        );
        assert!(report.run_elapsed.includes_stopped);
        assert_eq!(report.cpu_core_seconds.user, known.then_some(0.0));
        assert_eq!(report.cpu_core_seconds.system, known.then_some(0.0));
    }
    assert!(report(
        &id,
        &source.observer,
        None,
        source.accounting_relation.clone(),
        start,
        &terminal(true)
    )
    .is_none());
}
#[test]
fn invalid_duration_and_uncertain_lineage_are_explicit_and_never_additive() {
    let source = fixture();
    let id = JobId::try_from(source.job_id.clone()).unwrap();
    let mut end = terminal(true);
    end.status = ExitStatus::from_raw(libc::SIGTERM);
    let report = report(
        &id,
        &source.observer,
        Some(&source.child),
        AccountingRelation::Unverified {},
        end.observed_at + Duration::from_secs(1),
        &end,
    )
    .unwrap();
    assert_eq!(report.run_elapsed.seconds, None);
    assert!(report
        .missing_coverage
        .contains(&MissingCoverageReason::RunDurationUnavailable));
    assert!(report
        .missing_coverage
        .contains(&MissingCoverageReason::LineageUnverified));
    assert!(!report.accounting_relation.is_additive());
    assert_eq!(
        report.outcome,
        TerminalOutcome::Signaled {
            signal: libc::SIGTERM as u8,
            core_dumped: false
        }
    );
    assert!(!AccountingRelation::IncludedInRoot {
        root_job_id: "f".repeat(32)
    }
    .is_additive());
}
#[test]
fn lost_ack_is_reconciled_without_another_write() {
    let report = fixture();
    let end = Instant::now() + REPORT_WINDOW;
    let stored = summary(&report);
    let writes = RefCell::new(0);
    assert!(deliver_with(
        &report,
        end,
        |request, actual, budget, committed| {
            *writes.borrow_mut() += 1;
            assert_eq!(request, format!("job-meter-{}", report.report_id));
            assert_eq!(actual, &report);
            assert!(budget <= end);
            *committed = true;
            Err(Failure::Uncertain)
        },
        |job, budget| {
            assert_eq!(job, report.job_id);
            assert!(budget <= end);
            Ok(stored.clone())
        }
    ));
    assert_eq!(*writes.borrow(), 1);
}
#[test]
fn missing_ack_retries_only_identical_bytes_and_keeps_absolute_deadline() {
    let report = fixture();
    // Observe the exact deadline passed to retries without making this unit
    // test depend on being scheduled inside the native 500 ms report window.
    let end = Instant::now() + Duration::from_secs(60);
    let requests = RefCell::new(Vec::new());
    assert!(deliver_with(
        &report,
        end,
        |request, actual, budget, committed| {
            let mut requests = requests.borrow_mut();
            requests.push((request.to_owned(), serde_json::to_vec(actual).unwrap()));
            assert!(budget <= end);
            if requests.len() == 1 {
                *committed = true;
                Err(Failure::InvalidReply)
            } else {
                assert!(*committed);
                assert_eq!(budget, end, "retry renewed the terminal-derived deadline");
                Ok(())
            }
        },
        |_, budget| {
            assert!(budget <= end);
            Ok(JobMeterSummary {
                schema_version: 1,
                job_id: report.job_id.clone(),
                accounting: SummaryAccounting::Unknown {
                    reason: UnknownReason::Unavailable,
                },
                queue_wait: QueueWait::unknown(),
            })
        }
    ));
    let requests = requests.borrow();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0], requests[1]);
}
#[test]
fn conflicts_expiry_unsupported_and_expired_budget_cannot_retry() {
    let report = fixture();
    for mut stored in [summary(&report), summary(&report), summary(&report)] {
        if let SummaryAccounting::Recorded { report } = &mut stored.accounting {
            report.report_id = "f".repeat(32);
        }
        assert!(!deliver_with(
            &report,
            Instant::now() + REPORT_WINDOW,
            |_, _, _, committed| {
                *committed = true;
                Err(Failure::Uncertain)
            },
            |_, _| Ok(stored.clone())
        ));
    }
    for failure in [
        Failure::Unsupported,
        Failure::Unavailable,
        Failure::Rejected,
        Failure::Protocol,
        Failure::InvalidEndpoint,
        Failure::Cancelled,
    ] {
        assert!(!deliver_with(
            &report,
            Instant::now() + REPORT_WINDOW,
            |_, _, _, _| Err(failure),
            |_, _| panic!("must not read/retry definitive failure")
        ));
    }
    assert!(!deliver_with(
        &report,
        Instant::now() - Duration::from_millis(1),
        |_, _, _, _| panic!("exhausted deadline must not write"),
        |_, _| unreachable!()
    ));
    assert!(!deliver_with(
        &report,
        Instant::now() + REPORT_WINDOW,
        |_, _, _, committed| {
            *committed = true;
            Err(Failure::Busy)
        },
        |_, _| Ok(JobMeterSummary {
            schema_version: 1,
            job_id: report.job_id.clone(),
            accounting: SummaryAccounting::Expired {},
            queue_wait: QueueWait::unknown()
        })
    ));
}

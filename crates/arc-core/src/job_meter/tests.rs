use super::*;

fn fixture() -> JobMeterReport {
    serde_json::from_str(include_str!("../../tests/fixtures/job-meter-report.json")).unwrap()
}

#[test]
fn controlled_scope_fixture_round_trips_and_real_zero_is_known() {
    let mut report = fixture();
    assert!(report.is_valid());
    assert_eq!(
        serde_json::from_str::<JobMeterReport>(&serde_json::to_string(&report).unwrap()).unwrap(),
        report
    );
    assert!(report.accounting_relation.is_additive());
    report.cpu_core_seconds = CpuCoreSeconds {
        user: Some(0.0),
        system: Some(0.0),
        total: Some(0.0),
    };
    report.run_elapsed.seconds = Some(0.0);
    assert!(report.is_valid());
    assert_eq!(JOB_METADATA_RETENTION_MS, 7_776_000_000);
}

#[test]
fn numeric_and_identity_inconsistencies_fail_closed() {
    type MutateReport = Box<dyn Fn(&mut JobMeterReport)>;
    let mutations: Vec<MutateReport> = vec![
        Box::new(|r| r.schema_version = 2),
        Box::new(|r| r.job_id = "path/to/job".into()),
        Box::new(|r| r.report_id = "A".repeat(32)),
        Box::new(|r| r.observer.pid = r.child.pid),
        Box::new(|r| r.child.pid = 0),
        Box::new(|r| r.child.boot_id = "00000000-0000-0000-0000-000000000002".into()),
        Box::new(|r| r.cpu_core_seconds.user = Some(f64::NAN)),
        Box::new(|r| r.cpu_core_seconds.total = Some(f64::INFINITY)),
        Box::new(|r| r.cpu_core_seconds.system = Some(-0.1)),
        Box::new(|r| r.cpu_core_seconds.total = Some(8.0)),
        Box::new(|r| r.cpu_core_seconds.user = None),
        Box::new(|r| r.cpu_core_seconds.total = None),
        Box::new(|r| r.run_elapsed.seconds = None),
        Box::new(|r| r.run_elapsed.seconds = Some(-1.0)),
        Box::new(|r| r.run_elapsed.includes_stopped = false),
        Box::new(|r| r.terminated_at_ms = Some(crate::meter::MAX_SAFE_INTEGER + 1)),
        Box::new(|r| {
            r.outcome = TerminalOutcome::Signaled {
                signal: 0,
                core_dumped: false,
            }
        }),
        Box::new(|r| r.missing_coverage = vec![MissingCoverageReason::UnobservedDescendants]),
    ];
    for change in mutations {
        let mut report = fixture();
        change(&mut report);
        assert!(!report.is_valid(), "accepted invalid report: {report:?}");
    }
    assert!(!CpuCoreSeconds {
        user: Some(1.0),
        system: Some(1.0),
        total: None
    }
    .is_valid());
    assert!(!CpuCoreSeconds {
        user: None,
        system: None,
        total: Some(0.0)
    }
    .is_valid());
}

#[test]
fn unavailable_partial_and_unverified_reports_never_become_complete() {
    let mut report = fixture();
    report.completeness = Completeness::Partial;
    report.missing_coverage = vec![MissingCoverageReason::UnobservedDescendants];
    assert!(report.is_valid());
    report.accounting_relation = AccountingRelation::Unverified {};
    assert!(!report.is_valid());
    report
        .missing_coverage
        .push(MissingCoverageReason::LineageUnverified);
    assert!(report.is_valid());
    assert!(!report.accounting_relation.is_additive());
    report
        .missing_coverage
        .push(MissingCoverageReason::LineageUnverified);
    assert!(!report.is_valid());
    report.missing_coverage = vec![MissingCoverageReason::NativeUsageUnavailable];
    report.accounting_relation = fixture().accounting_relation;
    assert!(!report.is_valid());
    report.cpu_core_seconds = CpuCoreSeconds {
        user: Some(1.0),
        system: None,
        total: None,
    };
    assert!(report.is_valid());
    report.completeness = Completeness::Unknown;
    assert!(!report.is_valid());
    report.cpu_core_seconds.user = None;
    assert!(report.is_valid());
    report.run_elapsed.seconds = None;
    assert!(!report.is_valid());
    report
        .missing_coverage
        .push(MissingCoverageReason::RunDurationUnavailable);
    assert!(report.is_valid());
}

#[test]
fn nested_observations_are_inclusive_and_nonadditive() {
    let mut report = fixture();
    report.accounting_relation = AccountingRelation::IncludedInRoot {
        root_job_id: report.job_id.clone(),
    };
    assert!(!report.is_valid());
    report.accounting_relation = AccountingRelation::IncludedInRoot {
        root_job_id: "33333333333333333333333333333333".into(),
    };
    assert!(report.is_valid());
    assert!(!report.accounting_relation.is_additive());
    report.accounting_relation = AccountingRelation::CanonicalRoot {
        root_job_id: "33333333333333333333333333333333".into(),
    };
    assert!(!report.is_valid());
}

#[test]
fn queue_wait_is_independently_identified_and_never_inferred_from_run() {
    let known = QueueWait::from_wall_timestamps(Some(1000), Some(2500));
    assert_eq!(known.seconds, Some(1.5));
    assert_eq!(known.basis, QueueWaitBasis::QueuedToReserved);
    assert_eq!(known.clock, QueueClock::DaemonWallClock);
    assert!(known.is_valid());
    assert!(QueueWait::lifecycle_only().is_valid());
    for (queued, reserved) in [
        (None, None),
        (Some(2), None),
        (Some(2), Some(1)),
        (None, Some(2)),
        (Some(1), Some(u64::MAX)),
    ] {
        assert_eq!(
            QueueWait::from_wall_timestamps(queued, reserved),
            QueueWait::unknown()
        );
    }
    assert!(!QueueWait {
        seconds: Some(1.0),
        ..QueueWait::lifecycle_only()
    }
    .is_valid());
    assert!(!QueueWait {
        seconds: Some(0.0),
        ..QueueWait::unknown()
    }
    .is_valid());
}

#[test]
fn missing_legacy_interrupted_and_expired_remain_distinct() {
    let mut summary = JobMeterSummary {
        schema_version: 1,
        job_id: fixture().job_id,
        accounting: SummaryAccounting::Recorded {
            report: Box::new(fixture()),
        },
        queue_wait: QueueWait::lifecycle_only(),
    };
    assert!(summary.is_valid());
    summary.job_id = "33333333333333333333333333333333".into();
    assert!(!summary.is_valid());
    for reason in [
        UnknownReason::Legacy,
        UnknownReason::Interrupted,
        UnknownReason::Unavailable,
        UnknownReason::NotTerminated,
        UnknownReason::Unsupported,
    ] {
        summary.accounting = SummaryAccounting::Unknown { reason };
        assert!(summary.is_valid());
    }
    for accounting in [SummaryAccounting::Missing {}, SummaryAccounting::Expired {}] {
        summary.accounting = accounting;
        assert!(!summary.is_valid());
        summary.queue_wait = QueueWait::unknown();
        assert!(summary.is_valid());
        let value = serde_json::to_value(&summary).unwrap();
        assert!(!value.to_string().contains("cpu_core_seconds"));
        assert_eq!(
            serde_json::from_value::<JobMeterSummary>(value).unwrap(),
            summary
        );
        summary.queue_wait = QueueWait::lifecycle_only();
    }
}

#[test]
fn nullable_wire_fields_are_required_and_unknown_fields_rejected() {
    let value = serde_json::to_value(fixture()).unwrap();
    for key in ["user", "system", "total"] {
        let mut bad = value.clone();
        bad["cpu_core_seconds"].as_object_mut().unwrap().remove(key);
        assert!(serde_json::from_value::<JobMeterReport>(bad).is_err());
    }
    let mut bad = value.clone();
    bad.as_object_mut().unwrap().remove("terminated_at_ms");
    assert!(serde_json::from_value::<JobMeterReport>(bad).is_err());
    let mut bad = value;
    bad["argv"] = serde_json::json!(["secret"]);
    assert!(serde_json::from_value::<JobMeterReport>(bad).is_err());
}

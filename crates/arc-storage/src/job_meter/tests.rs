//! Synthetic durable fixtures; these tests establish storage invariants, not
//! native CPU accuracy. The combined native gate owns that measurement.
use super::*;
use arc_core::process_identity::ProcessIdentity;

mod lineage;
mod migration;
mod retention;

fn db() -> Connection {
    let mut c = Connection::open_in_memory().unwrap();
    c.execute_batch("PRAGMA foreign_keys=ON").unwrap();
    crate::migration::migrate(&mut c, crate::MIGRATIONS).unwrap();
    c
}
fn fixture(n: u32) -> JobMeterReport {
    let mut r: JobMeterReport = serde_json::from_str(include_str!(
        "../../../arc-core/tests/fixtures/job-meter-report.json"
    ))
    .unwrap();
    r.job_id = format!("{n:032x}");
    r.report_id = format!("{:032x}", n as u64 + 1_000_000);
    r.observer.pid = n * 2 + 100;
    r.child.pid = n * 2 + 101;
    r.accounting_relation = AccountingRelation::CanonicalRoot {
        root_job_id: r.job_id.clone(),
    };
    r
}
fn seed(c: &Connection, n: u32) -> JobMeterReport {
    let r = fixture(n);
    c.execute(
        "INSERT INTO jobs(id,job_class,locality,enforcement,state,created_at,finished_at,exit_code)
        VALUES(?1,'UNKNOWN','UNKNOWN','OBSERVE','SUCCEEDED',100,200,0)",
        [&r.job_id],
    )
    .unwrap();
    c.execute("INSERT INTO job_lifecycle(job_id,registered_at_ms,observer_pid,observer_start_token,
        observer_boot_id,observer_verified_at_ms,child_pid,child_start_token,child_boot_id,
        child_verified_at_ms,classification_reason,classification_uncertainty,launch_claimed_at_ms,
        observation_target,observation_status,observation_at_ms,observation_reason,outcome_kind,
        revision,updated_at_ms,reason) VALUES(?1,100,?2,?3,?4,100,?5,?6,?7,150,
        'UNRESOLVED_COMMAND','OBSERVE',120,'CHILD','VERIFIED_MATCH',150,'OBSERVED_EXIT','EXITED',3,200,'OBSERVED_EXIT')",
        params![r.job_id,r.observer.pid,r.observer.start_token,r.observer.boot_id,
            r.child.pid,r.child.start_token,r.child.boot_id]).unwrap();
    c.execute(
        "INSERT INTO job_meter_summaries(job_id,measurement_aware,relation,root_job_id,
        child_bound,terminal_at_ms) VALUES(?1,1,'canonical_root',?1,1,200)",
        [&r.job_id],
    )
    .unwrap();
    assert!(
        crate::jobs::get(c, &JobId::try_from(r.job_id.clone()).unwrap())
            .unwrap()
            .is_some()
    );
    r
}
fn payload(c: &Connection, id: &str) -> Option<String> {
    c.query_row(
        "SELECT report_json FROM job_meter_summaries WHERE job_id=?1",
        [id],
        |r| r.get(0),
    )
    .unwrap()
}
fn unknown(c: &mut Connection, r: &JobMeterReport, reason: UnknownReason) {
    assert_eq!(
        summary(c, &r.job_id, 300).unwrap().accounting,
        SummaryAccounting::Unknown { reason }
    );
}

#[test]
fn placeholder_refines_once_exact_retry_is_noop_and_conflicts_preserve_every_byte() {
    let mut c = db();
    let r = seed(&c, 1);
    unknown(&mut c, &r, UnknownReason::Unavailable);
    assert!(payload(&c, &r.job_id).is_none());
    assert_eq!(
        report(&mut c, &r, 300).unwrap().disposition,
        ReportDisposition::Accepted
    );
    let accepted = payload(&c, &r.job_id).unwrap();
    assert_eq!(
        report(&mut c, &r, 301).unwrap().disposition,
        ReportDisposition::Unchanged
    );
    let mut changed = r.clone();
    changed.report_id = "f".repeat(32);
    assert_eq!(report(&mut c, &changed, 302), Err(MeterError::Conflict));
    changed = r.clone();
    changed.cpu_core_seconds.user = Some(3.25);
    changed.cpu_core_seconds.total = Some(4.0);
    assert_eq!(report(&mut c, &changed, 303), Err(MeterError::Conflict));
    changed = r.clone();
    changed.terminated_at_ms = Some(9000);
    assert_eq!(report(&mut c, &changed, 304), Err(MeterError::Conflict));
    assert_eq!(payload(&c, &r.job_id).as_ref(), Some(&accepted));
    let s = summary(&mut c, &r.job_id, 305).unwrap();
    assert_eq!(
        s.accounting,
        SummaryAccounting::Recorded {
            report: Box::new(r.clone())
        }
    );
    assert_eq!(s.queue_wait, QueueWait::lifecycle_only());
    assert_eq!(binding(&c, &r.job_id).unwrap().unwrap().terminal, Some(200));
    assert_eq!(
        c.query_row("SELECT count(*) FROM resource_samples", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        0
    );
}

#[test]
fn accepted_unknown_report_is_immutable_but_a_synthetic_unknown_has_no_identity() {
    let mut c = db();
    let mut r = seed(&c, 1);
    r.cpu_core_seconds = CpuCoreSeconds {
        user: None,
        system: None,
        total: None,
    };
    r.completeness = Completeness::Unknown;
    r.missing_coverage = vec![MissingCoverageReason::NativeUsageUnavailable];
    report(&mut c, &r, 300).unwrap();
    let improved = fixture(1);
    assert_eq!(report(&mut c, &improved, 301), Err(MeterError::Conflict));
    assert_eq!(
        summary(&mut c, &r.job_id, 302).unwrap().accounting,
        SummaryAccounting::Recorded {
            report: Box::new(r)
        }
    );
}

#[test]
fn wrong_observer_child_outcome_unbound_child_and_nonterminal_are_rejected() {
    let mut c = db();
    let r = seed(&c, 1);
    for observer in [true, false] {
        let mut bad = r.clone();
        if observer {
            bad.observer.start_token = "9999".into();
        } else {
            bad.child.start_token = "9999".into();
        }
        assert_eq!(report(&mut c, &bad, 300), Err(MeterError::IdentityMismatch));
    }
    let mut bad = r.clone();
    bad.outcome = TerminalOutcome::Exited { code: 17 };
    assert_eq!(report(&mut c, &bad, 300), Err(MeterError::Conflict));
    c.execute("UPDATE job_meter_summaries SET child_bound=0", [])
        .unwrap();
    assert_eq!(report(&mut c, &r, 300), Err(MeterError::IdentityMismatch));
    c.execute("UPDATE job_meter_summaries SET child_bound=1", [])
        .unwrap();
    c.execute(
        "UPDATE jobs SET state='RUNNING',finished_at=NULL,exit_code=NULL",
        [],
    )
    .unwrap();
    c.execute("UPDATE job_lifecycle SET outcome_kind=NULL,reason='CHILD_VERIFIED',observation_reason='CHILD_VERIFIED'", []).unwrap();
    assert_eq!(report(&mut c, &r, 300), Err(MeterError::NotTerminal));
    unknown(&mut c, &r, UnknownReason::NotTerminated);
    assert!(payload(&c, &r.job_id).is_none());
}

#[test]
fn invalid_values_and_keys_are_bounded_and_never_write() {
    let mut c = db();
    let r = seed(&c, 1);
    for number in [f64::NAN, f64::INFINITY, -1.0, 1e30] {
        let mut bad = r.clone();
        bad.run_elapsed.seconds = Some(number);
        assert_eq!(report(&mut c, &bad, 300), Err(MeterError::InvalidRequest));
    }
    let mut bad = r.clone();
    bad.cpu_core_seconds.total = Some(90.0);
    assert_eq!(report(&mut c, &bad, 300), Err(MeterError::InvalidRequest));
    for id in ["", "123", &"a".repeat(16385), &"A".repeat(32)] {
        assert_eq!(summary(&mut c, id, 300), Err(MeterError::InvalidRequest));
    }
    assert_eq!(
        summary(&mut c, &r.job_id, MAX_TIMESTAMP + 1),
        Err(MeterError::InvalidRequest)
    );
    assert_eq!(
        report(&mut c, &r, MAX_TIMESTAMP + 1),
        Err(MeterError::InvalidRequest)
    );
    assert!(payload(&c, &r.job_id).is_none());
}

#[test]
fn queue_is_authoritative_separate_and_legacy_absence_never_becomes_zero() {
    let mut c = db();
    let r = seed(&c, 1);
    c.execute(
        "INSERT INTO traffic_entries(job_id,state,queued_at_ms,granted_at_ms,generation,context)
        VALUES(?1,'COMPLETED',100,1600,1,?2)",
        params![r.job_id, "a".repeat(64)],
    )
    .unwrap();
    report(&mut c, &r, 2000).unwrap();
    let s = summary(&mut c, &r.job_id, 2001).unwrap();
    assert_eq!(s.queue_wait.seconds, Some(1.5));
    assert_eq!(s.queue_wait.clock, QueueClock::DaemonWallClock);
    assert_eq!(s.queue_wait.basis, QueueWaitBasis::QueuedToReserved);
    let SummaryAccounting::Recorded { report } = s.accounting else {
        panic!()
    };
    assert_eq!(report.run_elapsed.seconds, Some(3.0));
    c.execute(
        "UPDATE traffic_entries SET granted_at_ms=NULL,generation=NULL,context=NULL",
        [],
    )
    .unwrap();
    assert_eq!(
        summary(&mut c, &r.job_id, 2002).unwrap().queue_wait,
        QueueWait::unknown()
    );
    c.execute("UPDATE job_meter_summaries SET measurement_aware=0", [])
        .unwrap();
    c.execute("DELETE FROM traffic_entries", []).unwrap();
    assert_eq!(
        summary(&mut c, &r.job_id, 2003).unwrap().queue_wait,
        QueueWait::unknown()
    );
}

#[test]
fn missing_legacy_interrupted_and_unavailable_are_distinct() {
    let mut c = db();
    let absent = summary(&mut c, &fixture(100).job_id, 300).unwrap();
    assert_eq!(absent.accounting, SummaryAccounting::Missing {});
    assert_eq!(absent.queue_wait, QueueWait::unknown());
    let historical = fixture(101);
    c.execute(
        "INSERT INTO jobs(id,job_class,locality,enforcement,state,created_at,finished_at)
        VALUES(?1,'UNKNOWN','UNKNOWN','OBSERVE','COMPLETED',100,200)",
        [&historical.job_id],
    )
    .unwrap();
    unknown(&mut c, &historical, UnknownReason::Legacy);
    let r = seed(&c, 1);
    c.execute("UPDATE job_meter_summaries SET measurement_aware=0", [])
        .unwrap();
    unknown(&mut c, &r, UnknownReason::Legacy);
    c.execute(
        "UPDATE jobs SET state='OUTCOME_UNKNOWN',finished_at=NULL,exit_code=NULL",
        [],
    )
    .unwrap();
    c.execute("UPDATE job_lifecycle SET outcome_kind=NULL,observation_status='MISSING',reason='PROCESS_MISSING',observation_reason='PROCESS_MISSING'", []).unwrap();
    unknown(&mut c, &r, UnknownReason::Interrupted);
}

#[test]
fn transaction_failure_rolls_back_acceptance_and_next_request_can_succeed() {
    let mut c = db();
    let r = seed(&c, 1);
    c.execute_batch(
        "CREATE TRIGGER inject_report_failure BEFORE UPDATE OF report_json ON job_meter_summaries
        BEGIN SELECT RAISE(ABORT,'injected'); END;",
    )
    .unwrap();
    assert_eq!(report(&mut c, &r, 300), Err(MeterError::Busy));
    assert!(c.is_autocommit());
    assert!(payload(&c, &r.job_id).is_none());
    c.execute_batch("DROP TRIGGER inject_report_failure")
        .unwrap();
    report(&mut c, &r, 301).unwrap();
    assert!(c
        .execute("UPDATE job_meter_summaries SET report_json='broken'", [])
        .is_err());
    assert!(c
        .execute(
            "UPDATE job_meter_summaries SET report_json=?1",
            [format!("\"{}\"", "x".repeat(16384))]
        )
        .is_err());
    c.execute("UPDATE job_meter_summaries SET report_json='{}'", [])
        .unwrap();
    assert_eq!(summary(&mut c, &r.job_id, 302), Err(MeterError::Busy));
}

use super::*;
use arc_ipc::traffic::{JobRequest, JobResult};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn scheduled_attachment_and_report_share_the_same_writer_and_queue_facts() {
    let fixture = Fixture::start();
    let db = fixture.worker.client();
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let job_id = JobId::try_from(arc_core::process_identity::random_job_id().unwrap()).unwrap();
    let classification = serde_json::from_value(serde_json::json!({"classification":"BUILD",
        "locality":"HOST_LOCAL","reason":"CARGO_BUILD","uncertainty":"NONE"}))
    .unwrap();
    let granted: JobResult = serde_json::from_value(
        super::super::super::traffic::handle(
            db.clone(),
            JobRequest::Enqueue {
                job_id: job_id.clone(),
                observer: observer.clone(),
                classification,
                lineage_context: None,
            },
            observer.pid,
            deadline(),
        )
        .await
        .unwrap(),
    )
    .unwrap();
    let JobResult::Granted {
        permit_generation, ..
    } = granted
    else {
        panic!("expected grant")
    };
    let mut child = Child(Command::new("/bin/sleep").arg("20").spawn().unwrap());
    let identity = ProcessIdentity::capture(child.0.id()).unwrap();
    super::super::super::traffic::handle(
        db.clone(),
        JobRequest::Attach {
            job_id: job_id.clone(),
            observer: observer.clone(),
            child: identity.clone(),
            permit_generation,
        },
        observer.pid,
        deadline(),
    )
    .await
    .unwrap();
    child.0.kill().unwrap();
    child.0.wait().unwrap();
    super::super::super::traffic::handle(
        db.clone(),
        JobRequest::Complete {
            job_id: job_id.clone(),
            observer: observer.clone(),
            outcome: Outcome::Signaled {
                signal: 9,
                core_dumped: false,
            },
            permit_generation,
        },
        observer.pid,
        deadline(),
    )
    .await
    .unwrap();
    let mut report: JobMeterReport = serde_json::from_str(include_str!(
        "../../../arc-core/tests/fixtures/job-meter-report.json"
    ))
    .unwrap();
    report.job_id = job_id.as_str().into();
    report.report_id = arc_core::process_identity::random_job_id().unwrap();
    report.observer = observer.clone();
    report.child = identity;
    report.completeness = Completeness::Partial;
    report.missing_coverage = vec![MissingCoverageReason::UnobservedDescendants];
    report.accounting_relation = if chain(&observer).1 {
        AccountingRelation::CanonicalRoot {
            root_job_id: job_id.as_str().into(),
        }
    } else {
        report
            .missing_coverage
            .push(MissingCoverageReason::LineageUnverified);
        AccountingRelation::Unverified {}
    };
    report.outcome = TerminalOutcome::Signaled {
        signal: 9,
        core_dumped: false,
    };
    submit(&db, &report).await.unwrap();
    let json = handle(
        db.clone(),
        JobMeterRequest::Summary(SummaryRequest {
            job_id: report.job_id.clone(),
        }),
        observer.pid,
        deadline(),
    )
    .await
    .unwrap();
    let summary = decode_summary(&json, &report.job_id).unwrap();
    assert_eq!(summary.queue_wait.clock, QueueClock::DaemonWallClock);
    assert_eq!(summary.queue_wait.basis, QueueWaitBasis::QueuedToReserved);
    assert!(summary.queue_wait.seconds.is_some());
    let SummaryAccounting::Recorded { report: persisted } = summary.accounting else {
        panic!()
    };
    assert_eq!(persisted.run_elapsed.seconds, Some(3.0));
    let c = rusqlite::Connection::open_with_flags(
        fixture.root.join("arc.sqlite3"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    let (queued, reserved): (i64, i64) = c
        .query_row(
            "SELECT queued_at_ms,granted_at_ms FROM traffic_entries WHERE job_id=?1",
            [&report.job_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        summary.queue_wait.seconds,
        Some((reserved - queued) as f64 / 1000.0)
    );
    c.close().unwrap();
    drop(db);
    fixture.finish().await;
}

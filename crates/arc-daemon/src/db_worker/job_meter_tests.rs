//! Real owned child/IPC identities with synthetic CPU values. Native calibration
//! and process restart acceptance remain the combined backend gate's ownership.
use super::*;
use crate::{
    admission::Requests,
    connection,
    db_worker::{DbConfig, DbWorker},
    limits::RuntimeLimits,
    runtime_metrics::{Gauge, RuntimeMetrics},
    transport_test_support::request,
};
use arc_core::job_meter::*;
use arc_ipc::{
    jobs::{CompleteRequest, JobId, LifecycleRequest, Outcome, StartRequest},
    meter::*,
};
use std::{num::NonZeroUsize, path::PathBuf, process::Command, sync::Arc, time::Duration};
use tokio::net::UnixStream;

#[path = "job_meter_scheduled_tests.rs"]
mod scheduled;
#[path = "job_meter_short_lived_tests.rs"]
mod short_lived;

struct Fixture {
    worker: DbWorker,
    root: PathBuf,
    metrics: Arc<RuntimeMetrics>,
}
impl Fixture {
    fn start() -> Self {
        let root = std::env::temp_dir().join(format!(
            "arc-meter-worker-{}",
            arc_core::process_identity::random_job_id().unwrap()
        ));
        let connection = arc_storage::open_and_migrate(root.join("arc.sqlite3")).unwrap();
        let limits = RuntimeLimits::default();
        let metrics = Arc::new(RuntimeMetrics::new(&limits));
        let worker = DbWorker::start(
            connection,
            DbConfig {
                queue_capacity: NonZeroUsize::new(4).unwrap(),
                busy_timeout: Duration::from_millis(30),
                execution_timeout: Duration::from_secs(1),
            },
            metrics.clone(),
        )
        .unwrap();
        Self {
            worker,
            root,
            metrics,
        }
    }
    async fn finish(mut self) {
        self.worker
            .shutdown(tokio::time::Instant::now() + Duration::from_secs(3))
            .await
            .unwrap();
        assert!(Gauge::ALL
            .iter()
            .all(|g| self.metrics.snapshot().gauge(*g).current == 0));
        std::fs::remove_dir_all(&self.root).unwrap();
    }
}
struct Child(std::process::Child);
impl Drop for Child {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(3)
}

#[test]
fn native_ancestry_uses_the_same_pid_one_boundary_as_cli_lineage() {
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let expected = arc_platform::job_lineage::ancestors(&observer).unwrap();
    let (actual, complete) = chain(&observer);
    assert!(
        complete,
        "stable userspace ancestry must not probe protected init"
    );
    assert_eq!(actual, expected);
    assert!(actual.iter().all(|identity| identity.pid != 1));
}
async fn terminal_job(client: &DbClient) -> JobMeterReport {
    let id = JobId::try_from(arc_core::process_identity::random_job_id().unwrap()).unwrap();
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let classification = serde_json::from_value(serde_json::json!({"classification":"UNKNOWN",
        "locality":"UNKNOWN","reason":"UNRESOLVED_COMMAND","uncertainty":"OBSERVE"}))
    .unwrap();
    for input in [
        StartRequest::Register {
            job_id: id.clone(),
            observer: observer.clone(),
            classification,
        },
        StartRequest::Claim {
            job_id: id.clone(),
            observer: observer.clone(),
        },
    ] {
        jobs::handle(client.clone(), LifecycleRequest::Start(input), deadline())
            .await
            .unwrap();
    }
    let mut child = Child(Command::new("/bin/sleep").arg("20").spawn().unwrap());
    let identity = ProcessIdentity::capture(child.0.id()).unwrap();
    jobs::handle(
        client.clone(),
        LifecycleRequest::Start(StartRequest::Attach {
            job_id: id.clone(),
            observer: observer.clone(),
            child: identity.clone(),
        }),
        deadline(),
    )
    .await
    .unwrap();
    child.0.kill().unwrap();
    child.0.wait().unwrap();
    // It is already reaped before reporting: any fresh child probe would fail.
    assert_ne!(probe(&identity), IdentityStatus::VerifiedMatch);
    jobs::handle(
        client.clone(),
        LifecycleRequest::Complete(CompleteRequest {
            job_id: id.clone(),
            observer: observer.clone(),
            outcome: Outcome::Signaled {
                signal: 9,
                core_dumped: false,
            },
        }),
        deadline(),
    )
    .await
    .unwrap();
    let mut report: JobMeterReport = serde_json::from_str(include_str!(
        "../../../arc-core/tests/fixtures/job-meter-report.json"
    ))
    .unwrap();
    report.job_id = id.as_str().into();
    report.report_id = arc_core::process_identity::random_job_id().unwrap();
    report.observer = observer.clone();
    report.child = identity;
    report.completeness = Completeness::Partial;
    report.missing_coverage = vec![MissingCoverageReason::UnobservedDescendants];
    report.accounting_relation = if chain(&observer).1 {
        AccountingRelation::CanonicalRoot {
            root_job_id: id.as_str().into(),
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
    report
}
async fn submit(
    client: &DbClient,
    report: &JobMeterReport,
) -> Result<serde_json::Value, &'static str> {
    handle(
        client.clone(),
        JobMeterRequest::Report(Box::new(report.clone())),
        std::process::id(),
        deadline(),
    )
    .await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn reports_bind_real_peer_identity_and_reaped_child_on_one_writer() {
    let fixture = Fixture::start();
    let client = fixture.worker.client();
    let report = terminal_job(&client).await;
    assert_eq!(
        handle(
            client.clone(),
            JobMeterRequest::Report(Box::new(report.clone())),
            std::process::id() + 1,
            deadline()
        )
        .await,
        Err(ERROR_METER_IDENTITY_MISMATCH)
    );
    let mut bad = report.clone();
    bad.observer.start_token = "1".into();
    assert_eq!(
        submit(&client, &bad).await,
        Err(ERROR_METER_IDENTITY_MISMATCH)
    );
    assert_eq!(
        submit(&client, &report).await.unwrap()["disposition"],
        "accepted"
    );
    assert_eq!(
        submit(&client, &report).await.unwrap()["disposition"],
        "unchanged"
    );
    let mut conflict = report.clone();
    conflict.report_id = "f".repeat(32);
    assert_eq!(
        submit(&client, &conflict).await,
        Err(ERROR_METER_REPORT_CONFLICT)
    );
    let json = handle(
        client.clone(),
        JobMeterRequest::Summary(SummaryRequest {
            job_id: report.job_id.clone(),
        }),
        std::process::id(),
        deadline(),
    )
    .await
    .unwrap();
    let summary = decode_summary(&json, &report.job_id).unwrap();
    assert_eq!(
        summary.accounting,
        SummaryAccounting::Recorded {
            report: Box::new(report.clone())
        }
    );
    assert_eq!(summary.queue_wait, QueueWait::lifecycle_only());
    // Independent read-only SQLite inspection: this is not another writer.
    let c = rusqlite::Connection::open_with_flags(
        fixture.root.join("arc.sqlite3"),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    let persisted: (String, f64, bool) = c
        .query_row(
            "SELECT report_id,json_extract(report_json,'$.cpu_core_seconds.total'),child_bound
         FROM job_meter_summaries WHERE job_id=?1",
            [&report.job_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(persisted, (report.report_id, 2.0, true));
    assert_eq!(
        fixture
            .metrics
            .snapshot()
            .gauge(Gauge::DbOutstanding)
            .current,
        0
    );
    c.close().unwrap();
    drop(client);
    fixture.finish().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn production_dispatch_correlates_validates_and_preserves_old_operations() {
    let fixture = Fixture::start();
    let db = fixture.worker.client();
    let report = terminal_job(&db).await;
    let limits = RuntimeLimits::test_profile();
    let requests = Requests::new(&limits, fixture.metrics.clone()).unwrap();
    let (server, client) = UnixStream::pair().unwrap();
    let handler = tokio::spawn(connection::handle_with_job_meter(
        server,
        limits,
        requests,
        fixture.metrics.clone(),
        |_| async { panic!("not a diagnostic") },
        |_, _| async { panic!("not lifecycle") },
        (
            |_, _, _| async { panic!("not Traffic") },
            None,
            Some(Arc::new({
                let db = db.clone();
                move |input, peer, deadline| Box::pin(handle(db.clone(), input, peer, deadline))
            })),
        ),
    ));
    let mut input = summary_request("summary-correlation", &report.job_id).unwrap();
    let output = request(&client, &input).await;
    assert_eq!(output.request_id, "summary-correlation");
    assert!(matches!(
        interpret_summary(&input, &output).unwrap().accounting,
        SummaryAccounting::Unknown {
            reason: UnknownReason::Unavailable
        }
    ));
    input.protocol_version = 99;
    assert_eq!(
        request(&client, &input).await.error.unwrap().code,
        arc_ipc::ERROR_PROTOCOL_INCOMPATIBLE
    );
    input.protocol_version = 1;
    for payload in [
        serde_json::json!({"job_id":"bad"}),
        serde_json::json!([report.job_id]),
        serde_json::json!({"job_id":report.job_id,"extra":true}),
        serde_json::json!({"job_id":"a".repeat(MAX_JOB_METER_BYTES + 1)}),
    ] {
        input.payload = payload;
        assert_eq!(
            request(&client, &input).await.error.unwrap().code,
            arc_ipc::ERROR_INVALID_REQUEST
        );
    }
    let input = report_request("report-correlation", &report).unwrap();
    let output = request(&client, &input).await;
    assert_eq!(
        interpret_report(&input, &output).unwrap().disposition,
        ReportDisposition::Accepted
    );
    assert_eq!(
        request(&client, &input).await.request_id,
        "report-correlation"
    );
    let input = summary_request("persisted", &report.job_id).unwrap();
    let output = request(&client, &input).await;
    assert_eq!(
        interpret_summary(&input, &output).unwrap().accounting,
        SummaryAccounting::Recorded {
            report: Box::new(report)
        }
    );
    let snapshot = arc_ipc::meter::snapshot_request("host");
    assert_eq!(
        request(&client, &snapshot).await.error.unwrap().code,
        ERROR_METER_UNAVAILABLE
    );
    assert_eq!(
        request(
            &client,
            &arc_ipc::Request::new("health", arc_ipc::OPERATION_HEALTH)
        )
        .await
        .status,
        arc_ipc::STATUS_OK
    );
    drop(client);
    handler.await.unwrap();
    drop(db);
    fixture.finish().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn expired_ticket_and_invalid_read_are_rejected_without_replacing_a_summary() {
    let fixture = Fixture::start();
    let db = fixture.worker.client();
    let report = terminal_job(&db).await;
    let input = JobMeterRequest::Report(Box::new(report.clone()));
    assert_eq!(
        handle(
            db.clone(),
            input,
            std::process::id(),
            Instant::now() - Duration::from_secs(1)
        )
        .await,
        Err(arc_ipc::ERROR_DB_BUSY)
    );
    assert_eq!(
        handle(
            db.clone(),
            JobMeterRequest::Summary(SummaryRequest {
                job_id: "bad".into()
            }),
            std::process::id(),
            deadline()
        )
        .await,
        Err(arc_ipc::ERROR_INVALID_REQUEST)
    );
    assert_eq!(
        submit(&db, &report).await.unwrap()["disposition"],
        "accepted"
    );
    drop(db);
    fixture.finish().await;
}

use super::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unreaped_terminated_child_can_bind_before_attach_in_both_execution_modes() {
    for scheduled in [false, true] {
        let fixture = Fixture::start();
        let db = fixture.worker.client();
        let observer = ProcessIdentity::capture(std::process::id()).unwrap();
        let job_id = JobId::try_from(arc_core::process_identity::random_job_id().unwrap()).unwrap();
        let mut child = Child(Command::new("/bin/sleep").arg("20").spawn().unwrap());
        let identity = ProcessIdentity::capture(child.0.id()).unwrap();
        child.0.kill().unwrap();
        let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
        // SAFETY: this test owns the PID and output buffer. WNOWAIT observes
        // termination without consuming the event; Child::wait is sole reaper.
        assert_eq!(
            unsafe {
                libc::waitid(
                    libc::P_PID,
                    identity.pid,
                    &mut info,
                    libc::WEXITED | libc::WNOWAIT,
                )
            },
            0
        );
        assert_eq!(probe(&identity), IdentityStatus::VerifiedMatch);
        assert!(owned_child(&identity, &observer));
        let classification = serde_json::from_value(serde_json::json!({"classification":"BUILD",
            "locality":"HOST_LOCAL","reason":"CARGO_BUILD","uncertainty":"NONE"}))
        .unwrap();
        let generation = if scheduled {
            use arc_ipc::traffic::{JobRequest, JobResult};
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
                panic!()
            };
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
            Some(permit_generation)
        } else {
            for input in [
                StartRequest::Register {
                    job_id: job_id.clone(),
                    observer: observer.clone(),
                    classification,
                },
                StartRequest::Claim {
                    job_id: job_id.clone(),
                    observer: observer.clone(),
                },
                StartRequest::Attach {
                    job_id: job_id.clone(),
                    observer: observer.clone(),
                    child: identity.clone(),
                },
            ] {
                jobs::handle(db.clone(), LifecycleRequest::Start(input), deadline())
                    .await
                    .unwrap();
            }
            None
        };
        child.0.wait().unwrap();
        let outcome = Outcome::Signaled {
            signal: 9,
            core_dumped: false,
        };
        if let Some(permit_generation) = generation {
            super::super::super::traffic::handle(
                db.clone(),
                arc_ipc::traffic::JobRequest::Complete {
                    job_id: job_id.clone(),
                    observer: observer.clone(),
                    outcome,
                    permit_generation,
                },
                observer.pid,
                deadline(),
            )
            .await
            .unwrap();
        } else {
            jobs::handle(
                db.clone(),
                LifecycleRequest::Complete(CompleteRequest {
                    job_id: job_id.clone(),
                    observer: observer.clone(),
                    outcome,
                }),
                deadline(),
            )
            .await
            .unwrap();
        }
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
        assert_eq!(
            submit(&db, &report).await.unwrap()["disposition"],
            "accepted"
        );
        drop(db);
        fixture.finish().await;
    }
}

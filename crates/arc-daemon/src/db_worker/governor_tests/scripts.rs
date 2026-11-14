//! Deterministic protocol/admission checks, with an explicit injected sampler.
//! Real Bun tools and registered desktop acceptance belong to task-9.
use super::*;
use arc_ipc::{
    traffic::{self as wire, diagnostics as diagnostic, JobResult as Reply},
    Request, Response,
};
use tokio::net::UnixStream;

async fn connect(f: &Fixture, supported: bool) -> (UnixStream, tokio::task::JoinHandle<()>) {
    let (server, client) = UnixStream::pair().unwrap();
    let limits = crate::limits::RuntimeLimits::default();
    let metrics = Arc::new(crate::runtime_metrics::RuntimeMetrics::new(&limits));
    let requests = crate::admission::Requests::new(&limits, metrics.clone()).unwrap();
    let db = f.client.clone();
    let handler: crate::connection::policy_dispatch::Handler = Arc::new(move |input, peer, end| {
        Box::pin(crate::db_worker::policy::handle(
            db.clone(),
            input,
            peer,
            end,
        ))
    });
    let jobs = f.client.clone();
    let traffic = f.client.clone();
    let task = tokio::spawn(crate::connection::handle_with_policy(
        server,
        limits,
        requests,
        metrics,
        |_| async { panic!("no doctor request") },
        move |input, end| crate::db_worker::jobs::handle(jobs.clone(), input, end),
        (
            move |input, peer, end| {
                crate::db_worker::traffic::handle(traffic.clone(), input, peer, end)
            },
            None,
            None,
        ),
        supported.then_some(handler),
    ));
    (client, task)
}
async fn close(client: UnixStream, task: tokio::task::JoinHandle<()>) {
    drop(client);
    tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap();
}
fn command(f: &Fixture, n: u32, class: &str) -> Request {
    let mut request = Request::new("script", wire::OPERATION_BUN_SCRIPTS_JOB);
    request.payload = serde_json::json!({"action":"TRAFFIC","request":{
        "action":"ENQUEUE","job_id":f.id(n),"observer":f.owner,
        "classification":{"classification":class,"locality":"HOST_LOCAL",
            "reason":format!("BUN_SCRIPT_{class}"),"uncertainty":"NONE"},
        "lineage_context":null}});
    request
}
async fn exchange(client: &UnixStream, request: &Request) -> Response {
    crate::transport_test_support::request(client, request).await
}
async fn diagnostic(client: &UnixStream, f: &Fixture, n: u32) -> diagnostic::DiagnosticResult {
    let mut request = Request::new("diagnostic", diagnostic::OPERATION_JOB);
    request.payload = serde_json::json!({"job_id":f.id(n)});
    diagnostic::interpret_result(&request, &exchange(client, &request).await).unwrap()
}
async fn action(client: &UnixStream, request: JobRequest) -> GovernorJobResult {
    let mut q = Request::new("action", wire::OPERATION_BUN_SCRIPTS_JOB);
    q.payload = serde_json::json!(GovernorJobRequest::Traffic { request });
    decode_job_result(exchange(client, &q).await.result.as_ref().unwrap()).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn negotiated_script_routes_reject_new_vocabulary_on_each_old_surface() {
    let f = Fixture::new(None).await;
    let (client, task) = connect(&f, true).await;
    for (operation, capability) in [
        (wire::OPERATION_TRAFFIC_CAPABILITIES, wire::CAPABILITY),
        (wire::OPERATION_BUN_CAPABILITIES, wire::BUN_CAPABILITY),
        (
            wire::OPERATION_BUN_SCRIPTS_CAPABILITIES,
            wire::BUN_SCRIPTS_CAPABILITY,
        ),
        (diagnostic::OPERATION_CAPABILITIES, diagnostic::CAPABILITY),
    ] {
        let q = Request::new("capability", operation);
        let response = exchange(&client, &q).await;
        assert_eq!(
            response.result,
            Some(serde_json::json!({"capability":capability}))
        );
        assert_eq!(
            wire::interpret_probe_for(&q, &response, operation, capability),
            wire::ProbeResult::Supported
        );
    }
    for (index, class) in ["BUILD", "TEST", "LINT", "TYPECHECK"]
        .into_iter()
        .enumerate()
    {
        let q = command(&f, index as u32 + 1, class);
        for op in [
            wire::OPERATION_TRAFFIC_JOB,
            wire::OPERATION_BUN_JOB,
            OPERATION_JOB,
            arc_ipc::jobs::OPERATION_JOB_START,
        ] {
            let mut old = q.clone();
            old.operation = op.into();
            if op != OPERATION_JOB {
                old.payload = q.payload["request"].clone();
            }
            if op == arc_ipc::jobs::OPERATION_JOB_START {
                old.payload["action"] = serde_json::json!("REGISTER");
                old.payload
                    .as_object_mut()
                    .unwrap()
                    .remove("lineage_context");
            }
            assert_eq!(
                exchange(&client, &old).await.error.unwrap().code,
                arc_ipc::ERROR_INVALID_REQUEST
            );
        }
        let response = exchange(&client, &q).await;
        let GovernorJobResult::Traffic {
            result: Reply::Granted {
                permit_generation, ..
            },
        } = decode_job_result(response.result.as_ref().unwrap()).unwrap()
        else {
            panic!("script grant: {response:?}")
        };
        let n = index as u32 + 1;
        let record = diagnostic(&client, &f, n).await;
        assert_eq!(
            serde_json::json!(record.classification)["classification"],
            class
        );
        assert!(record.admission.unwrap().permit_active);
        let mut status = Request::new("legacy-status", arc_ipc::jobs::OPERATION_JOB_STATUS);
        status.payload = serde_json::json!(arc_ipc::jobs::StatusRequest::ById { job_id: f.id(n) });
        let snapshot = exchange(&client, &status).await;
        assert_eq!(
            snapshot.result.as_ref().unwrap()["job"]["classification"],
            serde_json::json!({"classification":"UNKNOWN","locality":"UNKNOWN","reason":"UNRESOLVED_COMMAND","uncertainty":"OBSERVE"})
        );
        // Legacy completion cannot sidestep Traffic's generation checks.
        let mut completion = Request::new("legacy-complete", arc_ipc::jobs::OPERATION_JOB_COMPLETE);
        completion.payload = serde_json::json!(arc_ipc::jobs::CompleteRequest {
            job_id: f.id(n),
            observer: f.owner.clone(),
            outcome: Outcome::Exited { code: 17 }
        });
        let reply = exchange(&client, &completion).await;
        assert_eq!(
            reply.error.unwrap().code,
            arc_ipc::jobs::ERROR_INVALID_JOB_TRANSITION
        );
        action(
            &client,
            JobRequest::Complete {
                job_id: f.id(n),
                observer: f.owner.clone(),
                permit_generation,
                outcome: Outcome::Exited { code: 17 },
            },
        )
        .await;
        let terminal = exchange(&client, &status).await;
        assert_eq!(
            terminal.result.as_ref().unwrap()["job"]["classification"]["classification"],
            "UNKNOWN"
        );
        assert_eq!(
            terminal.result.as_ref().unwrap()["job"]["outcome"]["code"],
            17
        );
        let actual = diagnostic(&client, &f, n).await;
        assert_eq!(
            serde_json::json!(actual.classification)["classification"],
            class
        );
        let admission = actual.admission.unwrap();
        assert_eq!(admission.state, diagnostic::AdmissionState::Completed);
        assert_eq!(admission.outcome, Some(Outcome::Exited { code: 17 }));
        assert_eq!(admission.permit_generation, Some(permit_generation));
        assert!(!admission.permit_active);
    }
    assert_eq!(counts(&f), (4, 0));
    close(client, task).await;
    f.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn typecheck_script_route_keeps_governor_pressure_and_cancel_semantics() {
    let f = Fixture::new(None).await;
    let (client, task) = connect(&f, true).await;
    let first = exchange(&client, &command(&f, 1, "TYPECHECK")).await;
    let GovernorJobResult::Traffic {
        result: Reply::Granted {
            permit_generation, ..
        },
    } = decode_job_result(first.result.as_ref().unwrap()).unwrap()
    else {
        panic!()
    };
    exchange(&client, &command(&f, 2, "TYPECHECK")).await;
    assert_eq!(
        diagnostic(&client, &f, 2)
            .await
            .admission
            .unwrap()
            .wait_reason,
        Some(arc_policy::DecisionReason::ClassWindow)
    );
    f.level(1).await;
    action(
        &client,
        JobRequest::Poll {
            job_id: f.id(2),
            observer: f.owner.clone(),
        },
    )
    .await;
    assert_eq!(
        diagnostic(&client, &f, 2)
            .await
            .admission
            .unwrap()
            .wait_reason,
        Some(arc_policy::DecisionReason::MemoryPressure)
    );
    let mut launch = Request::new("held", OPERATION_LAUNCH);
    launch.payload = serde_json::json!(GovernorLaunchRequest::Reserved {
        request: arc_ipc::policy::launch::LaunchRequest {
            job_id: f.id(1),
            observer: f.owner.clone(),
            permit_generation
        }
    });
    assert!(
        matches!(decode_launch_result(exchange(&client, &launch).await.result.as_ref().unwrap()).unwrap(), GovernorLaunchResult::Reserved { result } if matches!(*result, arc_ipc::policy::launch::LaunchResult::Held { .. }))
    );
    set(&f, intent(0, false, 1)).await;
    assert_eq!(counts(&f), (1, 0));
    for n in [1, 2] {
        assert!(matches!(
            action(
                &client,
                JobRequest::Poll {
                    job_id: f.id(n),
                    observer: f.owner.clone()
                }
            )
            .await,
            GovernorJobResult::Unreserved { .. }
        ));
        let observed = diagnostic(&client, &f, n).await.admission.unwrap();
        assert_eq!(observed.state, diagnostic::AdmissionState::Unreserved);
        assert!(!observed.permit_active);
    }
    launch.payload = serde_json::json!(GovernorLaunchRequest::Unreserved {
        job_id: f.id(2),
        observer: f.owner.clone()
    });
    assert!(matches!(
        decode_launch_result(exchange(&client, &launch).await.result.as_ref().unwrap()).unwrap(),
        GovernorLaunchResult::Unreserved { .. }
    ));
    // Disconnect without ACK; the durable claim remains uncertain.
    close(client, task).await;
    let (client, task) = connect(&f, true).await;
    assert_eq!(
        diagnostic(&client, &f, 2).await.admission.unwrap().state,
        diagnostic::AdmissionState::Unconfirmed
    );
    assert!(matches!(
        decode_launch_result(exchange(&client, &launch).await.result.as_ref().unwrap()).unwrap(),
        GovernorLaunchResult::Unconfirmed {}
    ));
    for n in [1, 2] {
        action(
            &client,
            JobRequest::Cancel {
                job_id: f.id(n),
                observer: f.owner.clone(),
                permit_generation: None,
            },
        )
        .await;
    }
    set(&f, intent(1, true, 2)).await;
    assert!(matches!(
        exchange(&client, &command(&f, 3, "TYPECHECK"))
            .await
            .result
            .as_ref()
            .map(|p| decode_job_result(p).unwrap()),
        Some(GovernorJobResult::Traffic {
            result: Reply::Queued { .. }
        })
    ));
    f.level(0).await;
    assert!(matches!(
        action(
            &client,
            JobRequest::Poll {
                job_id: f.id(3),
                observer: f.owner.clone()
            }
        )
        .await,
        GovernorJobResult::Traffic {
            result: Reply::Granted { .. }
        }
    ));
    action(
        &client,
        JobRequest::Cancel {
            job_id: f.id(3),
            observer: f.owner.clone(),
            permit_generation: None,
        },
    )
    .await;
    assert_eq!(counts(&f), (2, 0));
    close(client, task).await;
    f.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lost_script_enqueue_ack_replacement_and_restart_preserve_one_durable_id() {
    let mut f = Fixture::new(None).await;
    f.worker
        .shutdown(tokio::time::Instant::now() + Duration::from_secs(2))
        .await
        .unwrap();
    let (hook, entered, release) = gate();
    f.worker = DbWorker::start_inner(
        arc_storage::open_and_migrate(f.path()).unwrap(),
        DbConfig {
            queue_capacity: std::num::NonZeroUsize::new(8).unwrap(),
            busy_timeout: Duration::from_millis(50),
            execution_timeout: Duration::from_secs(10),
        },
        Arc::new(crate::runtime_metrics::RuntimeMetrics::new(
            &crate::limits::RuntimeLimits::default(),
        )),
        crate::db_worker::execution::Hooks {
            after_operation: Some(hook),
            ..Default::default()
        },
    )
    .unwrap();
    f.client = f.worker.client();
    let q = command(&f, 1, "TYPECHECK");
    // First probe a current peer, then replace it before enqueue.
    let (client, task) = connect(&f, true).await;
    assert_eq!(
        exchange(
            &client,
            &Request::new("probe", wire::OPERATION_BUN_SCRIPTS_CAPABILITIES)
        )
        .await
        .result
        .unwrap()["capability"],
        wire::BUN_SCRIPTS_CAPABILITY
    );
    close(client, task).await;
    let (old, old_task) = connect(&f, false).await;
    assert_eq!(
        exchange(&old, &q).await.error.unwrap().code,
        arc_ipc::ERROR_UNSUPPORTED_CAPABILITY
    );
    close(old, old_task).await;
    assert_eq!(counts(&f), (0, 0));
    // Install a commit/ACK barrier at the writer; direct execution avoids the
    // route's preceding recovery reads consuming this one-use test hook.
    let input = wire::scripts::decode_job(&q.operation, &q.payload).unwrap();
    let GovernorJobRequest::Traffic { request } = input else {
        panic!()
    };
    let ticket = f
        .client
        .try_job(
            Instant::now() + Duration::from_secs(10),
            jobs::Operation::Governor(Operation::Enqueue {
                request,
                owner: repository::OwnerEvidence::gather(&f.owner),
                context: traffic::context().unwrap(),
                proof: None,
            }),
        )
        .unwrap();
    entered.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(counts(&f), (1, 1));
    let stored = arc_storage::jobs::get(&ro(&f), &f.id(1)).unwrap().unwrap();
    drop(ticket);
    release.send(()).unwrap();
    let (old, old_task) = connect(&f, false).await;
    assert_eq!(
        exchange(&old, &q).await.error.unwrap().code,
        arc_ipc::ERROR_UNSUPPORTED_CAPABILITY
    );
    close(old, old_task).await;
    assert_eq!(
        counts(&f),
        (1, 1),
        "unsupported replacement is not rollback evidence"
    );
    f.worker
        .shutdown(tokio::time::Instant::now() + Duration::from_secs(2))
        .await
        .unwrap();
    f.worker = DbWorker::start(
        arc_storage::open_and_migrate(f.path()).unwrap(),
        DbConfig {
            queue_capacity: std::num::NonZeroUsize::new(8).unwrap(),
            busy_timeout: Duration::from_millis(50),
            execution_timeout: Duration::from_secs(10),
        },
        Arc::new(crate::runtime_metrics::RuntimeMetrics::new(
            &crate::limits::RuntimeLimits::default(),
        )),
    )
    .unwrap();
    f.client = f.worker.client();
    let (client, task) = connect(&f, true).await;
    let replay = exchange(&client, &q).await;
    assert!(matches!(
        decode_job_result(replay.result.as_ref().unwrap()).unwrap(),
        GovernorJobResult::Traffic {
            result: Reply::Granted {
                permit_generation: 1,
                ..
            }
        }
    ));
    assert_eq!(counts(&f), (1, 1));
    assert_eq!(
        arc_storage::jobs::get(&ro(&f), &f.id(1))
            .unwrap()
            .unwrap()
            .classification,
        stored.classification
    );
    assert_eq!(diagnostic(&client, &f, 1).await.job_id, stored.job_id);
    action(
        &client,
        JobRequest::Cancel {
            job_id: f.id(1),
            observer: f.owner.clone(),
            permit_generation: None,
        },
    )
    .await;
    assert_eq!(counts(&f), (1, 0));
    close(client, task).await;
    f.shutdown().await;
}

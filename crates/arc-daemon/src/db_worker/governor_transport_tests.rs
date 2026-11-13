//! Deterministic framed IPC checks use production routing and the real writer.
use super::*;
use crate::{
    admission::Requests, connection, limits::RuntimeLimits, runtime_metrics::RuntimeMetrics,
};
use arc_ipc::{Request, Response};
async fn connect(f: &Fixture) -> (tokio::net::UnixStream, tokio::task::JoinHandle<()>) {
    let (server, client) = tokio::net::UnixStream::pair().unwrap();
    let limits = RuntimeLimits::default();
    let metrics = Arc::new(RuntimeMetrics::new(&limits));
    let requests = Requests::new(&limits, metrics.clone()).unwrap();
    let db = f.client.clone();
    let handler: connection::policy_dispatch::Handler = Arc::new(move |input, peer, end| {
        Box::pin(crate::db_worker::policy::handle(
            db.clone(),
            input,
            peer,
            end,
        ))
    });
    let task = tokio::spawn(connection::handle_with_policy(
        server,
        limits,
        requests,
        metrics,
        |_| async { panic!("diagnostics not requested") },
        |_, _| async { Err(arc_ipc::ERROR_UNSUPPORTED_CAPABILITY) },
        (
            |_, _, _| async { Err(arc_ipc::ERROR_UNSUPPORTED_CAPABILITY) },
            None,
            None,
        ),
        Some(handler),
    ));
    (client, task)
}
async fn request(
    client: &tokio::net::UnixStream,
    op: &str,
    payload: serde_json::Value,
) -> Response {
    let mut request = Request::new("governor-runtime", op);
    request.payload = payload;
    crate::transport_test_support::request(client, &request).await
}
async fn close(client: tokio::net::UnixStream, task: tokio::task::JoinHandle<()>) {
    drop(client);
    tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap()
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn governor_registered_daemon_routes_aliases_receipt_reads_and_strict_protocol_one() {
    let f = Fixture::new(None).await;
    let (client, server) = connect(&f).await;
    let caps = request(&client, OPERATION_CAPABILITIES, serde_json::Value::Null).await;
    assert_eq!(
        caps.result,
        Some(serde_json::json!(GovernorCapabilities::supported()))
    );
    let initial = request(
        &client,
        OPERATION_GET_SWITCH,
        serde_json::json!({"key":"governor.enabled"}),
    )
    .await;
    assert_eq!(
        decode_switch_read_result(
            initial.result.as_ref().unwrap(),
            &GetGovernorSwitchRequest::default()
        )
        .unwrap()
        .switch,
        GovernorSwitchState::default()
    );
    let q = intent(0, false, 1);
    let invalid = request(&client, OPERATION_RESUME, serde_json::json!(q)).await;
    assert_eq!(invalid.error.unwrap().code, arc_ipc::ERROR_INVALID_REQUEST);
    let changed = request(&client, OPERATION_PAUSE, serde_json::json!(q)).await;
    let committed = decode_mutation_result(changed.result.as_ref().unwrap(), &q).unwrap();
    assert_eq!(committed.switch().revision, 1);
    let replay = request(&client, OPERATION_SET_SWITCH, serde_json::json!(q)).await;
    assert!(matches!(
        decode_mutation_result(replay.result.as_ref().unwrap(), &q).unwrap(),
        GovernorMutationResult::Committed { replayed: true, .. }
    ));
    let read_query = GetGovernorSwitchRequest {
        key: GovernorSwitchKey::GovernorEnabled,
        mutation_id: Some(q.mutation_id),
    };
    let read = request(&client, OPERATION_GET_SWITCH, serde_json::json!(read_query)).await;
    let read = decode_switch_read_result(read.result.as_ref().unwrap(), &read_query).unwrap();
    assert_eq!(&read.switch, committed.switch());
    assert!(read.mutation_result.is_some());
    let mut bad = Request::new("bad-version", OPERATION_PAUSE);
    bad.protocol_version = 2;
    bad.payload = serde_json::json!(intent(1, true, 2));
    assert_eq!(
        crate::transport_test_support::request(&client, &bad)
            .await
            .error
            .unwrap()
            .code,
        arc_ipc::ERROR_PROTOCOL_INCOMPATIBLE
    );
    assert_eq!(state::get_switch(&ro(&f)).unwrap().revision, 1);
    close(client, server).await;
    f.shutdown().await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn governor_lost_launch_ack_disconnect_and_reconnect_never_issue_another_release() {
    let f = Fixture::new(None).await;
    set(&f, intent(0, false, 1)).await;
    f.governor(enqueue(&f, 1)).await;
    let (client, server) = connect(&f).await;
    let q = GovernorLaunchRequest::Unreserved {
        job_id: f.id(1),
        observer: f.owner.clone(),
    };
    let release = request(&client, OPERATION_LAUNCH, serde_json::json!(q)).await;
    let result = decode_launch_result(release.result.as_ref().unwrap()).unwrap();
    assert!(matches!(result, GovernorLaunchResult::Unreserved { .. }));
    close(client, server).await;
    let (client, server) = connect(&f).await;
    let retry = request(&client, OPERATION_LAUNCH, serde_json::json!(q)).await;
    assert_eq!(
        decode_launch_result(retry.result.as_ref().unwrap()).unwrap(),
        GovernorLaunchResult::Unconfirmed {}
    );
    assert_eq!(counts(&f), (0, 0));
    assert_eq!(
        ro(&f)
            .query_row(
                "SELECT count(*) FROM governor_jobs WHERE release_id IS NOT NULL",
                [],
                |r| r.get::<_, u32>(0)
            )
            .unwrap(),
        1
    );
    close(client, server).await;
    f.shutdown().await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn governor_unspawned_ack_after_expiry_drops_both_leases_before_writer_cleanup() {
    let f = Fixture::new(None).await;
    set(&f, intent(0, false, 1)).await;
    f.governor(enqueue(&f, 1)).await;
    let (client, server) = connect(&f).await;
    let q = GovernorLaunchRequest::Unreserved {
        job_id: f.id(1),
        observer: f.owner.clone(),
    };
    let begin = Instant::now();
    let release = request(&client, OPERATION_LAUNCH, serde_json::json!(q)).await;
    let result = decode_launch_result(release.result.as_ref().unwrap()).unwrap();
    let GovernorLaunchResult::Unreserved { release_id, .. } = &result else {
        panic!()
    };
    // Expiry is the production absolute std::Instant budget, not a timer guess
    // about whether a child has spawned. We send an explicit known-unspawned ACK.
    tokio::time::sleep_until(tokio::time::Instant::from_std(
        begin + Duration::from_millis(MAX_LAUNCH_VALID_FOR_MS + 10),
    ))
    .await;
    let ack = GovernorLaunchAck::Unreserved {
        ack: GovernorUnreservedAck::Unspawned {
            job_id: f.id(1),
            observer: f.owner.clone(),
            release_id: release_id.clone(),
        },
    };
    let response = tokio::time::timeout(
        Duration::from_secs(1),
        request(&client, OPERATION_LAUNCH_ACK, serde_json::json!(ack)),
    )
    .await
    .unwrap();
    assert_eq!(response.status, arc_ipc::STATUS_OK);
    assert!(execution::disposition(&ro(&f), &f.id(1))
        .unwrap()
        .unwrap()
        .release_id
        .is_none());
    assert!(matches!(
        set(&f, intent(1, true, 2)).await,
        GovernorMutationResult::Committed { .. }
    ));
    close(client, server).await;
    f.shutdown().await;
}

async fn late_unspawned_ack_after_cancel(resume: bool) {
    let f = Fixture::new(None).await;
    let Value::Job(GovernorJobResult::Traffic {
        result: arc_ipc::traffic::JobResult::Granted {
            permit_generation, ..
        },
    }) = f.governor(enqueue(&f, 1)).await
    else {
        panic!("initial reservation")
    };
    set(&f, intent(0, false, 1)).await;
    assert_eq!(counts(&f), (1, 0));
    let (client, server) = connect(&f).await;
    let q = GovernorLaunchRequest::Reserved {
        request: arc_ipc::policy::launch::LaunchRequest {
            job_id: f.id(1),
            observer: f.owner.clone(),
            permit_generation,
        },
    };
    let begin = Instant::now();
    let release = request(&client, OPERATION_LAUNCH, serde_json::json!(q)).await;
    let result = decode_launch_result(release.result.as_ref().unwrap()).unwrap();
    let GovernorLaunchResult::Unreserved { release_id, .. } = &result else {
        panic!("pause must convert the retired reservation to an unreserved release")
    };
    // Cross the actual absolute lease boundary while the original production
    // connection is still waiting for its matching ACK within the grace period.
    tokio::time::sleep_until(tokio::time::Instant::from_std(
        begin + Duration::from_millis(MAX_LAUNCH_VALID_FOR_MS + 10),
    ))
    .await;
    let (control, control_server) = connect(&f).await;
    let cancelled = request(
        &control,
        OPERATION_JOB,
        serde_json::json!(GovernorJobRequest::Traffic {
            request: JobRequest::Cancel {
                job_id: f.id(1),
                observer: f.owner.clone(),
                permit_generation: None,
            }
        }),
    )
    .await;
    assert_eq!(
        decode_job_result(cancelled.result.as_ref().unwrap()).unwrap(),
        GovernorJobResult::Traffic {
            result: arc_ipc::traffic::JobResult::Cancelled {}
        }
    );
    if resume {
        let q = intent(1, true, 2);
        let response = request(&control, OPERATION_RESUME, serde_json::json!(q)).await;
        assert_eq!(
            decode_mutation_result(response.result.as_ref().unwrap(), &q)
                .unwrap()
                .switch()
                .revision,
            2
        );
    }
    let before = arc_storage::jobs::get(&ro(&f), &f.id(1)).unwrap().unwrap();
    assert!(before.child.is_none() && before.outcome.is_none());
    let ack = GovernorLaunchAck::Unreserved {
        ack: GovernorUnreservedAck::Unspawned {
            job_id: f.id(1),
            observer: f.owner.clone(),
            release_id: release_id.clone(),
        },
    };
    let response = request(&client, OPERATION_LAUNCH_ACK, serde_json::json!(ack)).await;
    assert_eq!(response.status, arc_ipc::STATUS_OK);
    assert_eq!(
        repository::status(&ro(&f), &f.id(1)).unwrap(),
        arc_ipc::traffic::StatusResult::Cancelled {},
        "a matching late ACK must reconcile its claim without reviving cancelled work"
    );
    assert_eq!(
        arc_storage::jobs::get(&ro(&f), &f.id(1)).unwrap().unwrap(),
        before
    );
    close(client, server).await;
    close(control, control_server).await;

    // Reconnect through the real framed router: neither polling nor either
    // spelling of the old launch can create a fresh release or permit.
    let (client, server) = connect(&f).await;
    let poll = GovernorJobRequest::Traffic {
        request: JobRequest::Poll {
            job_id: f.id(1),
            observer: f.owner.clone(),
        },
    };
    for launch in [
        q,
        GovernorLaunchRequest::Unreserved {
            job_id: f.id(1),
            observer: f.owner.clone(),
        },
    ] {
        let response = request(&client, OPERATION_JOB, serde_json::json!(poll)).await;
        assert_eq!(
            decode_job_result(response.result.as_ref().unwrap()).unwrap(),
            GovernorJobResult::Traffic {
                result: arc_ipc::traffic::JobResult::Cancelled {}
            }
        );
        let response = request(&client, OPERATION_LAUNCH, serde_json::json!(launch)).await;
        assert_eq!(
            decode_launch_result(response.result.as_ref().unwrap()).unwrap(),
            GovernorLaunchResult::Cancelled {}
        );
    }
    let db = ro(&f);
    let switch = state::get_switch(&db).unwrap();
    assert_eq!(switch.enabled, resume);
    assert_eq!(switch.revision, if resume { 2 } else { 1 });
    assert_eq!(counts(&f), (1, 0));
    assert_eq!(
        repository::status(&db, &f.id(1)).unwrap(),
        arc_ipc::traffic::StatusResult::Cancelled {}
    );
    assert_eq!(
        arc_storage::jobs::get(&db, &f.id(1)).unwrap().unwrap(),
        before
    );
    let claims: u32 = db
        .query_row(
            "SELECT count(*) FROM governor_jobs WHERE release_id IS NOT NULL OR release_claimed_at_ms IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    let children: u32 = db
        .query_row(
            "SELECT count(*) FROM job_lifecycle WHERE child_pid IS NOT NULL",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!((claims, children), (0, 0));
    eprintln!(
        "governor-terminal-ack-observation={}",
        serde_json::json!({"case":"cancel-during-unreserved-ack-grace","interveningResume":resume,
            "switch":switch,"status":"CANCELLED","permits":counts(&f),
            "releaseClaims":claims,"sqliteChildren":children,"outcome":before.outcome,
            "scope":"production framed router and writer; injected pressure source; terminal retries authorize no child; none spawned in this case"})
    );
    drop(db);
    close(client, server).await;
    f.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn governor_late_unspawned_ack_preserves_cancelled_after_lease_expiry() {
    late_unspawned_ack_after_cancel(false).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn governor_late_unspawned_ack_preserves_cancelled_after_resume() {
    late_unspawned_ack_after_cancel(true).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn governor_backlogged_unknown_write_is_uncertain_and_reconnect_reads_authority() {
    let (hook, entered, release) = gate();
    let f = Fixture::new(Some(hook)).await;
    let block = f
        .client
        .try_doctor(Instant::now() + Duration::from_secs(10))
        .unwrap();
    entered.recv_timeout(Duration::from_secs(2)).unwrap();
    let (client, server) = connect(&f).await;
    let response = request(
        &client,
        OPERATION_PAUSE,
        serde_json::json!(intent(0, false, 1)),
    )
    .await;
    assert_eq!(response.error.unwrap().code, ERROR_WRITE_UNCERTAIN);
    release.send(()).unwrap();
    block.receive().await.unwrap();
    close(client, server).await;
    let (client, server) = connect(&f).await;
    let query = GetGovernorSwitchRequest {
        key: GovernorSwitchKey::GovernorEnabled,
        mutation_id: Some(intent(0, false, 1).mutation_id),
    };
    let read = request(&client, OPERATION_GET_SWITCH, serde_json::json!(query)).await;
    let read = decode_switch_read_result(read.result.as_ref().unwrap(), &query).unwrap();
    assert!(read.switch.enabled);
    assert_eq!(read.switch.revision, 0);
    assert!(read.mutation_result.is_none());
    close(client, server).await;
    f.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn governor_busy_job_route_preserves_retryable_db_busy_instead_of_read_unavailability() {
    let f = Fixture::new(None).await;
    set(&f, intent(0, false, 1)).await;
    f.governor(enqueue(&f, 1)).await;
    let launch = GovernorLaunchRequest::Unreserved {
        job_id: f.id(1),
        observer: f.owner.clone(),
    };
    let Value::Launch { result, fence } = f.governor(Input::Launch(launch.clone())).await else {
        panic!()
    };
    let (client, server) = connect(&f).await;
    let Input::Job(job) = enqueue(&f, 2) else {
        panic!()
    };
    let busy = request(&client, OPERATION_JOB, serde_json::json!(job)).await;
    assert_eq!(busy.error.unwrap().code, arc_ipc::ERROR_DB_BUSY);
    close(client, server).await;
    drop(fence);
    f.governor(Input::Unspawned {
        request: launch,
        result,
    })
    .await;
    f.shutdown().await;
}

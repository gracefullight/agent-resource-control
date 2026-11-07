use crate::{
    admission::{Handlers, Requests},
    connection::{self, DoctorOutcome},
    limits::RuntimeLimits,
    runtime_metrics::{Counter, Gauge, RuntimeMetrics},
    transport,
    transport_test_support::{request, response, send, setup, spawn_handler},
};
use arc_ipc::{FrameError, Request, Response, MAX_FRAME_BYTES, OPERATION_HELLO};
use std::{io, num::NonZeroUsize, os::fd::AsRawFd, sync::Arc, time::Duration};
use tokio::{
    net::UnixStream,
    time::{advance, timeout, Instant},
};

#[tokio::test(start_paused = true)]
async fn headers_and_trickled_bodies_have_absolute_deadlines() {
    let (server, client) = UnixStream::pair().unwrap();
    let deadline = Instant::now() + Duration::from_millis(100);
    let task = tokio::spawn(async move { transport::read_header(&server, deadline).await });
    for byte in [0, 0, 0] {
        send(&client, &[byte]).await;
        advance(Duration::from_millis(30)).await;
    }
    advance(Duration::from_millis(10)).await;
    assert_timeout(task.await.unwrap());

    let (server, client) = UnixStream::pair().unwrap();
    let deadline = Instant::now() + Duration::from_millis(100);
    let task = tokio::spawn(async move { transport::read_body(&server, 10, deadline).await });
    for _ in 0..3 {
        send(&client, b"x").await;
        advance(Duration::from_millis(30)).await;
    }
    advance(Duration::from_millis(10)).await;
    assert_timeout(task.await.unwrap());
}

fn assert_timeout<T: std::fmt::Debug>(result: Result<T, FrameError>) {
    assert!(
        matches!(result, Err(FrameError::Io(ref e)) if e.kind() == io::ErrorKind::TimedOut),
        "{result:?}"
    );
}

#[tokio::test(start_paused = true)]
async fn stalled_header_and_partial_body_release_request_capacity() {
    let (limits, metrics) = setup();
    let (client, task) = spawn_handler(limits.clone(), Arc::clone(&metrics));
    advance(limits.header_timeout).await;
    task.await.unwrap();
    assert_eq!(metrics.snapshot().counter(Counter::HeaderTimeouts), 1);
    drop(client);

    let (client, task) = spawn_handler(limits.clone(), Arc::clone(&metrics));
    send(&client, &100_u32.to_be_bytes()).await;
    send(&client, b"{").await;
    timeout(
        Duration::from_secs(2),
        metrics.wait_for(|s| s.gauge(Gauge::RequestBodyBytes).current == 100),
    )
    .await
    .unwrap();
    advance(limits.frame_timeout).await;
    task.await.unwrap();
    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.gauge(Gauge::Requests).current, 0);
    assert_eq!(snapshot.gauge(Gauge::RequestBodyBytes).current, 0);
    assert_eq!(snapshot.counter(Counter::FrameTimeouts), 1);
}

#[tokio::test]
async fn malformed_json_and_version_errors_continue_without_database() {
    let (limits, metrics) = setup();
    let (client, task) = spawn_handler(limits, Arc::clone(&metrics));
    send(&client, &1_u32.to_be_bytes()).await;
    send(&client, b"{").await;
    assert_eq!(
        response(&client).await.error.unwrap().code,
        arc_ipc::ERROR_INVALID_REQUEST
    );
    let mut wrong = Request::new("version", arc_ipc::OPERATION_DOCTOR);
    wrong.protocol_version = 99;
    assert_eq!(
        request(&client, &wrong).await.error.unwrap().code,
        arc_ipc::ERROR_PROTOCOL_INCOMPATIBLE
    );
    assert_eq!(
        request(&client, &Request::new("ok", OPERATION_HELLO))
            .await
            .status,
        arc_ipc::STATUS_OK
    );
    drop(client);
    task.await.unwrap();
    assert!(Gauge::ALL
        .iter()
        .all(|g| metrics.snapshot().gauge(*g).current == 0));
}

#[tokio::test]
async fn zero_and_oversized_frames_reject_without_body_allocation() {
    for (declared, code) in [
        (0, arc_ipc::ERROR_INVALID_REQUEST),
        (MAX_FRAME_BYTES + 1, arc_ipc::ERROR_FRAME_TOO_LARGE),
    ] {
        let (limits, metrics) = setup();
        let (client, task) = spawn_handler(limits, Arc::clone(&metrics));
        send(&client, &declared.to_be_bytes()).await;
        assert_eq!(response(&client).await.error.unwrap().code, code);
        task.await.unwrap();
        assert_eq!(
            metrics.snapshot().gauge(Gauge::RequestBodyBytes).high_water,
            0
        );
        assert_eq!(metrics.snapshot().gauge(Gauge::Requests).high_water, 1);
    }
    let (server, _client) = UnixStream::pair().unwrap();
    assert!(matches!(
        transport::read_body(&server, MAX_FRAME_BYTES + 1, Instant::now()).await,
        Err(FrameError::TooLarge { .. })
    ));
}

#[tokio::test]
async fn saturated_requests_close_before_allocating_even_for_invalid_prefixes() {
    for declared in [100, 0, MAX_FRAME_BYTES + 1] {
        let (limits, metrics) = setup();
        let requests = Requests::new(&limits, Arc::clone(&metrics)).unwrap();
        let guards: Vec<_> = (0..limits.requests.get())
            .map(|_| requests.try_admit().unwrap())
            .collect();
        let (server, client) = UnixStream::pair().unwrap();
        let task = tokio::spawn(connection::handle_connection(
            server,
            limits,
            requests,
            Arc::clone(&metrics),
            |_| async { unreachable!() },
        ));
        send(&client, &declared.to_be_bytes()).await;
        timeout(Duration::from_secs(1), task)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            metrics.snapshot().gauge(Gauge::RequestBodyBytes).high_water,
            0
        );
        assert_eq!(metrics.snapshot().counter(Counter::RequestRejected), 1);
        drop(guards);
    }
}

#[tokio::test(start_paused = true)]
async fn nonreading_peer_expires_under_write_backpressure() {
    let (server, _client) = UnixStream::pair().unwrap();
    let size: libc::c_int = 1024;
    assert_eq!(
        unsafe {
            libc::setsockopt(
                server.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_SNDBUF,
                &size as *const _ as *const libc::c_void,
                std::mem::size_of_val(&size) as libc::socklen_t,
            )
        },
        0
    );
    let deadline = Instant::now() + Duration::from_millis(100);
    let task = tokio::spawn(async move {
        transport::write_response(
            &server,
            &Response::error("x".repeat(512 * 1024), "TEST"),
            deadline,
        )
        .await
    });
    // A separate running task plus explicit yields prevent paused-time auto-advance.
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
    assert!(
        !task.is_finished(),
        "large response must encounter actual backpressure"
    );
    advance(Duration::from_millis(100)).await;
    assert_eq!(
        task.await.unwrap().unwrap_err().kind(),
        io::ErrorKind::TimedOut
    );
}

#[tokio::test]
async fn response_serialization_is_capped_before_any_prefix_is_written() {
    let (server, client) = UnixStream::pair().unwrap();
    let result = transport::write_response(
        &server,
        &Response::error("\0".repeat(MAX_FRAME_BYTES as usize / 2), "TEST"),
        Instant::now() + Duration::from_secs(2),
    )
    .await;
    assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidData);
    let mut byte = [0];
    assert_eq!(
        client.try_read(&mut byte).unwrap_err().kind(),
        io::ErrorKind::WouldBlock
    );
}

#[tokio::test]
async fn retained_tasks_reap_and_rejections_do_not_call_factory() {
    let mut limits = RuntimeLimits::test_profile();
    limits.connections = NonZeroUsize::new(4).unwrap();
    let metrics = Arc::new(RuntimeMetrics::new(&limits));
    let mut handlers = Handlers::new(&limits, Arc::clone(&metrics)).unwrap();
    let gate = Arc::new(tokio::sync::Semaphore::new(0));
    let mut clients = Vec::new();
    for _ in 0..4 {
        let (server, client) = UnixStream::pair().unwrap();
        clients.push(client);
        let gate = Arc::clone(&gate);
        assert!(handlers.try_spawn(server, |_| async move {
            let _permit = gate.acquire().await.unwrap();
        }));
    }
    for _ in 0..100 {
        let (server, _client) = UnixStream::pair().unwrap();
        assert!(
            !handlers.try_spawn(server, |_| -> std::future::Pending<()> {
                panic!("rejected factory invoked")
            })
        );
    }
    assert_eq!(handlers.len(), 4);
    gate.add_permits(4);
    timeout(
        Duration::from_secs(2),
        metrics.wait_for(|s| s.gauge(Gauge::Connections).current == 0),
    )
    .await
    .unwrap();
    assert_eq!(
        handlers.len(),
        4,
        "completed entries remain accounted until reaped"
    );
    assert_eq!(metrics.snapshot().tracked_tasks.current, 4);
    let (server, _client) = UnixStream::pair().unwrap();
    assert!(handlers.try_spawn(server, |_| async {}));
    assert_eq!(
        handlers.len(),
        1,
        "ready completions reaped before replacement"
    );
    handlers.join_next().await.unwrap().unwrap();
    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.tracked_tasks.high_water, 4);
    assert_eq!(snapshot.counter(Counter::HandlersSpawned), 5);
    assert_eq!(snapshot.counter(Counter::HandlersReaped), 5);
    assert_eq!(snapshot.counter(Counter::ConnectionRejected), 100);
    assert_eq!(snapshot.gauge(Gauge::RejectionWork).high_water, 1);
}

#[tokio::test]
async fn cancellation_reaps_unpolled_handlers_and_drops_all_guards() {
    let (limits, metrics) = setup();
    let mut handlers = Handlers::new(&limits, Arc::clone(&metrics)).unwrap();
    let (server, _client) = UnixStream::pair().unwrap();
    assert!(handlers.try_spawn(server, |_| std::future::pending::<()>()));
    handlers
        .shutdown(Instant::now() + Duration::from_secs(1))
        .await
        .unwrap();
    assert_eq!(handlers.len(), 0);
    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.gauge(Gauge::Connections).current, 0);
    assert_eq!(snapshot.counter(Counter::HandlersReaped), 1);
    let (server, _client) = UnixStream::pair().unwrap();
    assert!(
        !handlers.try_spawn(server, |_| -> std::future::Pending<()> {
            panic!("shutdown admits no work")
        })
    );
}

#[tokio::test(start_paused = true)]
async fn header_progress_does_not_renew_the_body_budget() {
    let (limits, metrics) = setup();
    let (client, task) = spawn_handler(limits.clone(), Arc::clone(&metrics));
    // Start the handler and deliver three bytes, then the fourth just before
    // the header deadline. Body admission supplies synchronization.
    send(&client, &[0, 0, 0]).await;
    tokio::task::yield_now().await;
    advance(limits.header_timeout - Duration::from_millis(1)).await;
    send(&client, &[100]).await;
    timeout(
        Duration::from_secs(2),
        metrics.wait_for(|s| s.gauge(Gauge::Requests).current == 1),
    )
    .await
    .unwrap();
    let remaining = limits.frame_timeout - limits.header_timeout + Duration::from_millis(1);
    advance(remaining - Duration::from_millis(1)).await;
    tokio::task::yield_now().await;
    assert!(!task.is_finished());
    advance(Duration::from_millis(1)).await;
    task.await.unwrap();
    assert_eq!(metrics.snapshot().counter(Counter::FrameTimeouts), 1);
}

#[tokio::test]
async fn oversized_echo_uses_one_small_fallback_then_closes() {
    let (mut limits, metrics) = setup();
    limits.write_timeout = Duration::from_millis(500);
    let (client, task) = spawn_handler(limits, Arc::clone(&metrics));
    let overhead = serde_json::to_vec(&Request::new("", OPERATION_HELLO))
        .unwrap()
        .len();
    let reply = request(
        &client,
        &Request::new(
            "x".repeat(MAX_FRAME_BYTES as usize - overhead),
            OPERATION_HELLO,
        ),
    )
    .await;
    assert!(
        reply.request_id.is_empty(),
        "overflow fallback must not echo the large ID"
    );
    assert_eq!(reply.error.unwrap().code, arc_ipc::ERROR_FRAME_TOO_LARGE);
    task.await.unwrap();
    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.counter(Counter::FramesCompleted), 1);
    assert_eq!(
        snapshot.gauge(Gauge::ResponseBytes).high_water,
        MAX_FRAME_BYTES as usize
    );
    assert!(Gauge::ALL.iter().all(|g| snapshot.gauge(*g).current == 0));
}

#[tokio::test(start_paused = true)]
async fn blocked_handler_write_releases_request_and_response_reservations() {
    let (limits, metrics) = setup();
    let requests = Requests::new(&limits, Arc::clone(&metrics)).unwrap();
    let (server, client) = UnixStream::pair().unwrap();
    let size: libc::c_int = 1024;
    assert_eq!(
        unsafe {
            libc::setsockopt(
                server.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_SNDBUF,
                &size as *const _ as *const libc::c_void,
                std::mem::size_of_val(&size) as libc::socklen_t,
            )
        },
        0
    );
    let write_timeout = limits.write_timeout;
    let task = tokio::spawn(connection::handle_connection(
        server,
        limits,
        requests,
        Arc::clone(&metrics),
        |_| async { unreachable!() },
    ));
    let mut bytes = Vec::new();
    arc_ipc::write_message(
        &mut bytes,
        &Request::new("x".repeat(512 * 1024), OPERATION_HELLO),
    )
    .unwrap();
    send(&client, &bytes).await;
    timeout(
        Duration::from_secs(2),
        metrics.wait_for(|s| s.gauge(Gauge::ResponseBytes).current > 0),
    )
    .await
    .unwrap();
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
    assert!(!task.is_finished());
    advance(write_timeout).await;
    task.await.unwrap();
    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.counter(Counter::WriteTimeouts), 1);
    assert!(Gauge::ALL.iter().all(|g| snapshot.gauge(*g).current == 0));
}

#[tokio::test]
async fn same_user_authentication_supports_async_sockets_and_rejects_non_sockets() {
    let (server, _client) = UnixStream::pair().unwrap();
    crate::platform::ensure_same_user(&server).unwrap();
    let file = std::fs::File::open("/dev/null").unwrap();
    assert!(crate::platform::ensure_same_user(&file).is_err());
}

#[test]
fn metrics_are_fixed_size_and_preserve_high_waters() {
    let (_, metrics) = setup();
    let guard = metrics.enter(Gauge::DbOutstanding, 1);
    metrics.increment(Counter::DbRejected);
    assert_eq!(metrics.snapshot().gauge(Gauge::DbOutstanding).current, 1);
    drop(guard);
    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.gauge(Gauge::DbOutstanding).current, 0);
    assert_eq!(snapshot.gauge(Gauge::DbOutstanding).high_water, 1);
    assert!(serde_json::to_vec(&snapshot.json()).unwrap().len() < 3_000);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn request_admission_races_and_unwind_never_exceed_cap() {
    let (limits, metrics) = setup();
    let requests = Requests::new(&limits, Arc::clone(&metrics)).unwrap();
    let mut tasks = tokio::task::JoinSet::new();
    for _ in 0..16 {
        let requests = requests.clone();
        tasks.spawn(async move {
            for _ in 0..1_000 {
                if let Some(guard) = requests.try_admit() {
                    tokio::task::yield_now().await;
                    drop(guard);
                }
            }
        });
    }
    while let Some(task) = tasks.join_next().await {
        task.unwrap();
    }
    assert!(metrics.snapshot().gauge(Gauge::Requests).high_water <= limits.requests.get());
    assert_eq!(metrics.snapshot().gauge(Gauge::Requests).current, 0);
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = requests.try_admit().unwrap();
        panic!("test unwind");
    }));
    assert_eq!(metrics.snapshot().gauge(Gauge::Requests).current, 0);
}

#[tokio::test]
async fn typed_worker_outcomes_keep_busy_distinct_from_integrity() {
    for busy in [true, false] {
        let (limits, metrics) = setup();
        let requests = Requests::new(&limits, Arc::clone(&metrics)).unwrap();
        let (server, client) = UnixStream::pair().unwrap();
        let task = tokio::spawn(connection::handle_connection(
            server,
            limits,
            requests,
            metrics,
            move |_| async move {
                if busy {
                    DoctorOutcome::Busy
                } else {
                    DoctorOutcome::IntegrityFailed
                }
            },
        ));
        let reply = request(&client, &Request::new("db", arc_ipc::OPERATION_DOCTOR)).await;
        assert_eq!(
            reply.error.unwrap().code,
            if busy {
                connection::ERROR_DB_BUSY
            } else {
                arc_ipc::ERROR_INTEGRITY_FAILED
            }
        );
        drop(client);
        task.await.unwrap();
    }
}

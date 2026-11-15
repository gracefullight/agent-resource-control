//! Controlled subprocesses reuse the production supervisor and test-only worker gate.
#![allow(dead_code)] // Imported production modules expose more APIs than this target needs.
#[path = "../src/accept_recovery.rs"]
mod accept_recovery;
#[path = "../src/admission.rs"]
mod admission;
#[path = "support/admission_load.rs"]
mod admission_load;
#[path = "../src/connection.rs"]
mod connection;
#[path = "../src/db_worker.rs"]
mod db_worker;
#[path = "support/deadline_recovery.rs"]
mod deadline_recovery;
#[path = "../src/instance.rs"]
mod instance;
#[path = "../src/job_meter_dispatch.rs"]
mod job_meter_dispatch;
#[path = "../src/lifecycle.rs"]
mod lifecycle;
#[path = "../src/limits.rs"]
mod limits;
#[path = "../src/meter.rs"]
mod meter;
#[cfg(target_os = "macos")]
#[path = "support/meter_partial.rs"]
mod meter_partial;
#[path = "../src/platform.rs"]
mod platform;
#[path = "support/runtime.rs"]
mod runtime;
#[path = "../src/runtime_metrics.rs"]
mod runtime_metrics;
#[path = "../src/runtime_report.rs"]
mod runtime_report;
#[path = "../src/server.rs"]
mod server;
mod support;
#[path = "../src/transport.rs"]
mod transport;
#[path = "../src/transport_test_support.rs"]
mod transport_test_support;

use runtime as native_fixture;
use runtime::{receipt, send, Fixture, OwnedChild};
use runtime_metrics::{Gauge, RuntimeMetrics};
use std::{
    fs,
    io::Write,
    num::NonZeroUsize,
    sync::{mpsc, Arc},
    time::{Duration, Instant},
};

#[test]
fn controlled_daemon_child() {
    let Some(mode) = std::env::var_os("ARC_BOUNDED_CHILD") else {
        return;
    };
    let mut builder = if mode == "deadlines" {
        let mut builder = tokio::runtime::Builder::new_current_thread();
        builder.start_paused(true);
        builder
    } else {
        let mut builder = tokio::runtime::Builder::new_multi_thread();
        builder.worker_threads(2);
        builder
    };
    builder.enable_all().build().unwrap().block_on(async {
        // A runnable task prevents Tokio's paused clock from auto-advancing
        // while the external parent performs the two rejection handshakes.
        let clock_hold = (mode == "deadlines").then(|| tokio::spawn(async {
            loop { tokio::task::yield_now().await; }
        }));
        let socket = arc_core::socket_path().unwrap();
        let anchor = arc_core::instance_anchor().unwrap();
        server::validate_socket_path(&socket).unwrap();
        server::prepare_socket_directory(&anchor).unwrap();
        let _instance = instance::InstanceLock::acquire(&anchor).unwrap();
        server::prepare_socket_directory(&socket).unwrap();
        let mut database = arc_storage::open_and_migrate(arc_core::database_path().unwrap()).unwrap();
        let limits = if mode == "deadlines" { limits::RuntimeLimits::test_profile() } else { limits::RuntimeLimits { connections:NonZeroUsize::new(8).unwrap(),
            requests:NonZeroUsize::new(4).unwrap(),db_queue:NonZeroUsize::new(2).unwrap(),
            ..if mode == "meter-partial" { limits::RuntimeLimits::default() } else { limits::RuntimeLimits::holding_fixture() } } };
        let metrics = Arc::new(RuntimeMetrics::new(&limits));
        let (started, _started) = mpsc::sync_channel(1);
        let (release, receiver) = mpsc::sync_channel(1);
        let config = db_worker::DbConfig {
            queue_capacity:limits.db_queue,busy_timeout:limits.db_busy_timeout,execution_timeout:limits.db_execution_timeout,
        };
        let worker = if mode == "sql" || mode == "schema" {
            let paused_pragma = if mode == "schema" { "user_version" } else { "integrity_check" };
            let mut gate = Some(receiver);
            database.authorizer(Some(move |context: rusqlite::hooks::AuthContext<'_>| {
                if matches!(context.action, rusqlite::hooks::AuthAction::Pragma { pragma_name, .. } if pragma_name == paused_pragma) {
                    if let Some(gate) = gate.take() {
                        // Deliberately pause inside SQLite's preparation of the
                        // real fixed diagnostic, without replacing its result.
                        println!("bounded-child:sql-active");
                        std::io::stdout().flush().unwrap();
                        gate.recv_timeout(Duration::from_secs(60)).expect("parent must release SQL gate");
                    }
                }
                rusqlite::hooks::Authorization::Allow
            })).unwrap();
            db_worker::DbWorker::start(database, config, Arc::clone(&metrics)).unwrap()
        } else if mode == "deadlines" || mode == "meter-partial" {
            db_worker::DbWorker::start(database, config, Arc::clone(&metrics)).unwrap()
        } else {
            db_worker::start_with_test_hooks(database, config, Arc::clone(&metrics), db_worker::WorkerTestHooks {started,release:receiver}).unwrap()
        };
        let serving = lifecycle::supervise(socket.clone(), limits, Arc::clone(&metrics), worker);
        tokio::pin!(serving);
        let observe = async {
            if mode == "meter-partial" { std::future::pending::<()>().await; }
            if mode == "deadlines" {
                for (gauge, count, marker) in [(Gauge::Requests,4,"requests-full"),(Gauge::Connections,8,"connections-full")] {
                    metrics.wait_for(|s|s.gauge(gauge).current==count).await;
                    println!("bounded-child:{marker}");
                    std::io::stdout().flush().unwrap();
                }
                while !socket.parent().unwrap().join("expire-frames").exists() { tokio::task::yield_now().await; }
                let snapshot = metrics.snapshot();
                assert_eq!(snapshot.gauge(Gauge::Requests).current, 4);
                assert_eq!(snapshot.gauge(Gauge::Connections).current, 8);
                assert_eq!(snapshot.counter(runtime_metrics::Counter::RequestRejected), 1);
                assert_eq!(snapshot.counter(runtime_metrics::Counter::ConnectionRejected), 1);
                assert_eq!(snapshot.counter(runtime_metrics::Counter::HeaderTimeouts), 0);
                assert_eq!(snapshot.counter(runtime_metrics::Counter::FrameTimeouts), 0);
                tokio::time::advance(Duration::from_millis(301)).await;
                metrics.wait_for(|s|s.gauge(Gauge::Connections).current==0).await;
                println!("bounded-child:deadline-released");
                std::io::stdout().flush().unwrap();
                tokio::time::resume();
                clock_hold.as_ref().unwrap().abort();
                std::future::pending::<()>().await;
            }
            for (gauge, count, marker) in [(Gauge::DbExecuting,1,"active"),(Gauge::DbQueued,2,"queue-full")] {
                metrics.wait_for(|s|s.gauge(gauge).current==count).await;
                println!("bounded-child:{marker}");
                std::io::stdout().flush().unwrap();
            }
            if mode == "release" || mode == "sql" || mode == "schema" {
                while !socket.parent().unwrap().join("release").exists() { tokio::time::sleep(Duration::from_millis(1)).await; }
                let snapshot = metrics.snapshot();
                assert_eq!(snapshot.gauge(Gauge::DbExecuting).current, 1);
                assert_eq!(snapshot.gauge(Gauge::DbQueued).current, 2);
                assert_eq!(snapshot.gauge(Gauge::DbOutstanding).current, 3);
                assert_eq!(snapshot.counter(runtime_metrics::Counter::DbTimeouts), 0);
                release.send(()).unwrap();
                metrics.wait_for(|s|s.gauge(Gauge::DbOutstanding).current==0).await;
                println!("bounded-child:released");
                std::io::stdout().flush().unwrap();
            } else {
                metrics.wait_for(|s|s.gauge(Gauge::RequestBodyBytes).current>=1024).await;
                println!("bounded-child:body");
                std::io::stdout().flush().unwrap();
                metrics.wait_for(|s|s.gauge(Gauge::Connections).current==8).await;
                println!("bounded-child:connections-full");
                std::io::stdout().flush().unwrap();
                let trigger = socket.parent().unwrap().join("signal");
                while !trigger.exists() { tokio::time::sleep(Duration::from_millis(1)).await; }
                // Earlier markers observe individual gauges at different
                // instants. Require the complete saturation state together
                // before self-signalling, including retirement of short-lived
                // health/rejection handlers from the parent's setup phase.
                metrics.wait_for(|s| [(Gauge::Connections,8),(Gauge::Requests,4),(Gauge::DbExecuting,1),(Gauge::DbQueued,2),(Gauge::DbOutstanding,3)]
                    .iter().all(|(gauge,count)| s.gauge(*gauge).current==*count)).await;
                let snapshot = metrics.snapshot();
                for (gauge, count) in [(Gauge::Connections,8),(Gauge::Requests,4),(Gauge::DbExecuting,1),(Gauge::DbQueued,2),(Gauge::DbOutstanding,3)] {
                    assert_eq!(snapshot.gauge(gauge).current, count, "{gauge:?} at signal trigger");
                }
                assert_eq!(snapshot.counter(runtime_metrics::Counter::DbTimeouts), 0);
                let signal: i32 = fs::read_to_string(trigger).unwrap().parse().unwrap();
                assert!([libc::SIGTERM,libc::SIGINT].contains(&signal));
                assert_eq!(unsafe { libc::kill(std::process::id() as i32, signal) }, 0);
            }
            std::future::pending::<()>().await;
        };
        tokio::select! { result = &mut serving => result.unwrap(), _ = observe => unreachable!() }
    });
}

fn spawn(fixture: &Fixture, mode: &str) -> OwnedChild {
    let mut command = std::process::Command::new(std::env::current_exe().unwrap());
    command.args([
        "--exact",
        "controlled_daemon_child",
        "--nocapture",
        "--test-threads=1",
    ]);
    fixture.configure(&mut command);
    command.env("ARC_BOUNDED_CHILD", mode);
    if mode == "deadlines" {
        command.env("ARC_IPC_PROFILE", "test");
    }
    let mut child = OwnedChild::new(command.spawn().unwrap());
    fixture.wait_for(&mut child, "arc-daemon ready.");
    child
}

fn saturate_db(fixture: &Fixture, child: &mut OwnedChild) -> Vec<std::os::unix::net::UnixStream> {
    let mut streams = vec![fixture.connect()];
    send(
        &mut streams[0],
        &arc_ipc::Request::new("active", arc_ipc::OPERATION_DOCTOR),
    );
    fixture.wait_for(child, "bounded-child:active");
    for _ in 0..2 {
        let mut stream = fixture.connect();
        send(
            &mut stream,
            &arc_ipc::Request::new("queued", arc_ipc::OPERATION_DOCTOR),
        );
        streams.push(stream);
    }
    fixture.wait_for(child, "bounded-child:queue-full");
    let busy = fixture.cli("doctor");
    assert!(!busy.status.success(), "{busy:?}");
    assert!(busy.stdout.is_empty());
    assert_eq!(busy.stderr, b"IPC_ERROR\n");
    let status = fixture.cli("status");
    assert!(status.status.success(), "{status:?}");

    streams
}

#[test]
fn controlled_slow_database_does_not_block_health_and_cli_recovers() {
    let _serial = support::serial();
    let fixture = Fixture::new();
    let mut child = spawn(&fixture, "release");
    let mut streams = saturate_db(&fixture, &mut child);

    fs::write(fixture.root.join("release"), b"go").unwrap();
    fixture.wait_for(&mut child, "bounded-child:released");
    for stream in &mut streams {
        let response: arc_ipc::Response = arc_ipc::read_message(stream).unwrap();
        assert_eq!(response.status, arc_ipc::STATUS_OK);
    }
    drop(streams);
    fixture.assert_cli();
    let (exit_us, _) = child.stop(libc::SIGTERM);
    let report = fixture.report();
    assert_eq!(report["metrics"]["db_queued"]["high_water"], 2);
    assert_eq!(report["metrics"]["db_executing"]["high_water"], 1);
    assert_eq!(report["metrics"]["db_outstanding"]["high_water"], 3);
    assert_eq!(report["metrics"]["db_rejected"], 1);
    receipt(
        "slow-db-release",
        &serde_json::json!({"report":report,"signal_to_exit_us":exit_us,"reopen_integrity":"ok"}),
    );
}

#[test]
fn health_completes_while_native_sql_preparation_is_deliberately_stalled() {
    health_during_preparation("sql");
}

#[test]
fn health_completes_while_schema_assessment_is_deliberately_stalled() {
    health_during_preparation("schema");
}

fn health_during_preparation(mode: &str) {
    let _serial = support::serial();
    let fixture = Fixture::new();
    let mut child = spawn(&fixture, mode);
    let mut streams = saturate_db(&fixture, &mut child);
    fixture.wait_for(&mut child, "bounded-child:sql-active");
    let start = Instant::now();
    let status = fixture.cli("status");
    assert!(status.status.success(), "{status:?}");

    let health_us = start.elapsed().as_micros();
    fs::write(fixture.root.join("release"), b"go").unwrap();
    fixture.wait_for(&mut child, "bounded-child:released");
    for stream in &mut streams {
        let response: arc_ipc::Response = arc_ipc::read_message(stream).unwrap();
        assert_eq!(response.status, arc_ipc::STATUS_OK);
    }
    drop(streams);
    fixture.assert_cli();
    child.stop(libc::SIGTERM);
    receipt(
        mode,
        &serde_json::json!({"report":fixture.report(),"health_cli_us":health_us,
        "controlled_delay":"one bounded test-only SQLite authorizer callback; original diagnostic SQL executes after release", "mode":mode}),
    );
}

fn signal_case(signal: i32, name: &str) {
    let _serial = support::serial();
    let fixture = Fixture::new();
    let mut child = spawn(&fixture, "signal");
    let _database_clients = saturate_db(&fixture, &mut child);
    let mut body = fixture.connect();
    body.write_all(&1024u32.to_be_bytes()).unwrap();
    body.write_all(b"{").unwrap();
    fixture.wait_for(&mut child, "bounded-child:body");
    let mut rejected = fixture.connect();
    send(
        &mut rejected,
        &arc_ipc::Request::new("reject", arc_ipc::OPERATION_HELLO),
    );
    runtime::assert_closed(rejected);
    let mut headers = Vec::new();
    for _ in 0..4 {
        let mut stream = fixture.connect();
        stream.write_all(&[0]).unwrap();
        headers.push(stream);
    }
    fixture.wait_for(&mut child, "bounded-child:connections-full");
    let rejected = fixture.connect();
    runtime::assert_closed(rejected);

    // Publish only the complete value: the child observes existence before
    // reading, and fs::write creates the destination before writing its bytes.
    let pending_signal = fixture.root.join("signal.pending");
    fs::write(&pending_signal, signal.to_string()).unwrap();
    fs::rename(pending_signal, fixture.root.join("signal")).unwrap();
    let (exit_us, peak) = child.wait_for_exit();
    let report = fixture.report();
    assert_eq!(report["reason"], name);
    assert_eq!(report["metrics"]["connections"]["high_water"], 8);
    assert_eq!(report["metrics"]["tracked_tasks"]["high_water"], 8);
    assert_eq!(report["metrics"]["requests"]["high_water"], 4);
    assert_eq!(report["metrics"]["db_outstanding"]["high_water"], 3);
    assert_eq!(report["metrics"]["db_timeouts"], 0);
    assert_eq!(report["metrics"]["request_rejected"], 1);
    assert_eq!(report["metrics"]["connection_rejected"], 1);
    drop(body);
    drop(headers);
    // A new real daemon must acquire admission and reopen this same database.
    let mut restart = fixture.command();
    restart.env("ARC_RUNTIME_REPORT", fixture.root.join("restart.json"));
    let mut restart = OwnedChild::new(restart.spawn().unwrap());
    fixture.wait_for(&mut restart, "arc-daemon ready.");
    fixture.assert_cli();
    restart.stop(libc::SIGTERM);
    let restart_report: serde_json::Value =
        serde_json::from_slice(&fs::read(fixture.root.join("restart.json")).unwrap()).unwrap();
    runtime::assert_report(&restart_report);
    receipt(
        name,
        &serde_json::json!({"signal_to_exit_us":exit_us,"native_peak_rss_bytes":peak,
        "report":report,"restart_report":restart_report,"reopen_integrity":"ok",
        "shutdown_cancelled_requests":4,"shutdown_cancelled_headers":4}),
    );
}

#[test]
fn saturated_sigterm_finishes_within_five_seconds() {
    signal_case(libc::SIGTERM, "SIGTERM");
}
#[test]
fn saturated_sigint_finishes_within_five_seconds() {
    signal_case(libc::SIGINT, "SIGINT");
}

//! Owned subprocess checks for the shared supervisor and actual CLI binary.

use crate::{
    db_worker,
    limits::RuntimeLimits,
    runtime_metrics::{Gauge, RuntimeMetrics},
    server,
};
use std::{
    fs,
    io::{Read, Write},
    num::NonZeroUsize,
    os::unix::{fs::DirBuilderExt, net::UnixStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Arc,
    },
    thread,
    time::{Duration, Instant},
};

#[path = "../tests/support/mod.rs"]
mod support;

#[path = "accept_recovery_tests.rs"]
mod accept_recovery_tests;

#[path = "report_lifecycle_tests.rs"]
mod report_lifecycle_tests;

#[test]
fn controlled_worker_child() {
    if std::env::var_os("ARC_LIFECYCLE_CHILD").is_none() {
        return;
    }
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let (socket, instance) = crate::prepare_instance().unwrap();
            let connection =
                arc_storage::open_and_migrate(arc_core::database_path().unwrap()).unwrap();
            let limits = RuntimeLimits {
                db_queue: NonZeroUsize::new(2).unwrap(),
                ..RuntimeLimits::default()
            };
            let metrics = Arc::new(RuntimeMetrics::new(&limits));
            let (started, _started_receiver) = mpsc::sync_channel(1);
            let (_release_sender, release) = mpsc::sync_channel(1);
            let worker = db_worker::start_with_test_hooks(
                connection,
                db_worker::DbConfig {
                    queue_capacity: limits.db_queue,
                    busy_timeout: limits.db_busy_timeout,
                    execution_timeout: limits.db_execution_timeout,
                },
                Arc::clone(&metrics),
                db_worker::WorkerTestHooks { started, release },
            )
            .unwrap();
            let serving = server::serve_with_worker(socket, limits, Arc::clone(&metrics), worker);
            tokio::pin!(serving);
            tokio::select! {
                result = &mut serving => panic!("supervisor ended before active job: {result:?}"),
                _ = metrics.wait_for(|s| s.gauge(Gauge::DbExecuting).current == 1) => {}
            }
            println!("lifecycle-fixture: worker-active");
            tokio::select! {
                result = &mut serving => panic!("supervisor ended before full queue: {result:?}"),
                _ = metrics.wait_for(|s| s.gauge(Gauge::DbQueued).current == 2) => {}
            }
            println!("lifecycle-fixture: queue-full");
            tokio::select! {
                result = &mut serving => panic!("supervisor ended before stalled body: {result:?}"),
                _ = metrics.wait_for(|s| s.gauge(Gauge::RequestBodyBytes).current >= 1024) => {}
            }
            println!("lifecycle-fixture: stalled-body");
            serving.await.unwrap();
            let anchor = arc_core::instance_anchor().unwrap();
            assert!(matches!(
                crate::instance::InstanceLock::acquire(&anchor),
                Err(crate::instance::InstanceError::Duplicate { .. })
            ));
            println!("lifecycle-fixture: instance-held-through-join");
            drop(instance);
            let _reacquired = crate::instance::InstanceLock::acquire(&anchor).unwrap();
        });
}

struct Fixture {
    root: PathBuf,
    socket: PathBuf,
    database: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = arc_core::runtime_root().unwrap().join(format!(
            "l{:x}-{:x}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        Self {
            socket: root.join("arc.sock"),
            database: root.join("arc.sqlite3"),
            root,
        }
    }
    fn spawn(&self) -> OwnedChild {
        self.spawn_test(
            "lifecycle_tests::controlled_worker_child",
            "ARC_LIFECYCLE_CHILD",
            "1",
        )
    }
    fn spawn_test(&self, test: &str, setting: &str, value: &str) -> OwnedChild {
        let child = self.command(test).env(setting, value).spawn().unwrap();
        OwnedChild(child)
    }
    fn command(&self, test: &str) -> Command {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", test, "--nocapture", "--test-threads=1"])
            .env_remove("ARC_IPC_PROFILE")
            .env("ARC_RUNTIME_REPORT", self.root.join("runtime.json"))
            .env(arc_core::ARC_SOCKET_PATH, &self.socket)
            .env(arc_core::ARC_DB_PATH, &self.database)
            .stdout(fs::File::create(self.root.join("stdout")).unwrap())
            .stderr(fs::File::create(self.root.join("stderr")).unwrap())
            .stdin(Stdio::null());
        command
    }
    fn stdout(&self) -> String {
        read_bounded(&self.root.join("stdout"))
    }
    fn wait_for(&self, child: &mut OwnedChild, text: &str) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if self.stdout().contains(text) {
                return;
            }
            assert!(
                child.0.try_wait().unwrap().is_none(),
                "child exited: {} {}",
                self.stdout(),
                read_bounded(&self.root.join("stderr"))
            );
            assert!(
                Instant::now() < deadline,
                "missing {text}: {}",
                self.stdout()
            );
            thread::sleep(Duration::from_millis(1));
        }
    }
    fn report(&self) -> serde_json::Value {
        let bytes = fs::read(self.root.join("runtime.json")).unwrap();
        assert!(bytes.len() <= crate::runtime_report::MAX_REPORT_BYTES);
        let report = serde_json::from_slice(&bytes).unwrap();
        assert_final_report(&report);
        report
    }
    fn connect(&self) -> UnixStream {
        let stream = UnixStream::connect(&self.socket).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        stream
            .set_write_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        stream
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}
struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
}
fn read_bounded(path: &Path) -> String {
    let mut result = String::new();
    if let Ok(file) = fs::File::open(path) {
        file.take(8192).read_to_string(&mut result).unwrap();
    }
    result
}

fn signal_case(signal: libc::c_int, name: &str) {
    let _serial = support::serial();
    let fixture = Fixture::new();
    let mut child = fixture.spawn();
    fixture.wait_for(&mut child, "arc-daemon ready.");
    let mut doctor = fixture.connect();
    let work_started = Instant::now();
    arc_ipc::write_message(
        &mut doctor,
        &arc_ipc::Request::new("slow", arc_ipc::OPERATION_DOCTOR),
    )
    .unwrap();
    fixture.wait_for(&mut child, "lifecycle-fixture: worker-active");
    let mut queued = [fixture.connect(), fixture.connect()];
    for stream in &mut queued {
        arc_ipc::write_message(
            stream,
            &arc_ipc::Request::new("queued", arc_ipc::OPERATION_DOCTOR),
        )
        .unwrap();
    }
    fixture.wait_for(&mut child, "lifecycle-fixture: queue-full");
    // Real CLI error-first mapping must distinguish overload from corruption/version failure.
    let cli = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("arcctl");
    assert!(
        cli.is_file(),
        "workspace CLI prerequisite missing: {}",
        cli.display()
    );
    let busy = Command::new(&cli)
        .arg("doctor")
        .env(arc_core::ARC_SOCKET_PATH, &fixture.socket)
        .output()
        .unwrap();
    assert!(!busy.status.success(), "{busy:?}");
    assert!(busy.stdout.is_empty(), "{busy:?}");
    assert_eq!(busy.stderr, b"IPC_ERROR\n");
    let health = arc_ipc::exchange(
        &mut fixture.connect(),
        &arc_ipc::Request::new("independent", arc_ipc::OPERATION_HEALTH),
    )
    .unwrap();
    assert_eq!(health.status, arc_ipc::STATUS_OK);
    assert!(
        work_started.elapsed() < Duration::from_millis(900),
        "slow-job budget elapsed before signal"
    );
    let mut header = fixture.connect();
    header.write_all(&[0]).unwrap();
    let mut body = fixture.connect();
    body.write_all(&1024u32.to_be_bytes()).unwrap();
    body.write_all(b"{").unwrap();
    fixture.wait_for(&mut child, "lifecycle-fixture: stalled-body");
    // Ordered hello is an accept-loop barrier: both preceding sockets were admitted.
    assert_eq!(
        arc_ipc::exchange(
            &mut fixture.connect(),
            &arc_ipc::Request::new("barrier", arc_ipc::OPERATION_HELLO)
        )
        .unwrap()
        .status,
        arc_ipc::STATUS_OK
    );
    assert!(
        work_started.elapsed() < Duration::from_millis(900),
        "slow-job budget elapsed before signal"
    );
    let started = Instant::now();
    assert_eq!(
        unsafe { libc::kill(child.0.id() as libc::pid_t, signal) },
        0
    );
    let status = loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            break status;
        }
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "signal exit exceeded five seconds"
        );
        thread::sleep(Duration::from_millis(1));
    };
    let signal_to_exit_us = started.elapsed().as_micros();
    assert!(started.elapsed() < Duration::from_secs(5));
    assert!(
        status.success(),
        "{} {}",
        fixture.stdout(),
        read_bounded(&fixture.root.join("stderr"))
    );
    let stdout = fixture.stdout();
    assert!(!stdout.contains("arc-daemon runtime:"), "{stdout}");
    let report = fixture.report();
    assert_eq!(report["handlers_remaining"], 0);
    assert_eq!(report["profiles"]["ipc"], "default");
    assert_eq!(report["reason"], name);
    for key in [
        "accounted",
        "handlers_reaped",
        "worker_joined",
        "connection_closed",
        "gauges_zero",
        "socket_removed",
        "deadline_met",
    ] {
        assert_eq!(report[key], true, "{key}: {report}");
    }
    assert_eq!(report["metrics"]["db_timeouts"], 0);
    assert_eq!(report["metrics"]["frames_completed"], 3);
    assert!(
        report["metrics"]["request_body_bytes"]["high_water"]
            .as_u64()
            .unwrap()
            >= 1024
    );
    assert_eq!(report["metrics"]["worker_started"], 1);
    assert_eq!(report["metrics"]["worker_joined"], 1);
    assert_eq!(report["metrics"]["db_queued"]["high_water"], 2);
    assert_eq!(report["metrics"]["db_executing"]["high_water"], 1);
    assert_eq!(report["metrics"]["db_outstanding"]["high_water"], 3);
    assert_eq!(
        report["metrics"]["handlers_spawned"],
        report["metrics"]["handlers_reaped"]
    );
    assert!(stdout.contains("lifecycle-fixture: instance-held-through-join"));
    assert!(!fixture.socket.exists());
    let reopened = arc_storage::open_and_migrate(&fixture.database).unwrap();
    assert_eq!(
        reopened
            .query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
            .unwrap(),
        "ok"
    );
    reopened.close().unwrap();
    eprintln!(
        "integration-signal-receipt: {}",
        serde_json::json!({"signal":name,"signal_to_exit_us":signal_to_exit_us,"reopen_integrity":"ok","report":report})
    );
}

#[test]
fn sigterm_reaps_stalled_handlers_and_joins_worker() {
    signal_case(libc::SIGTERM, "SIGTERM");
}
#[test]
fn sigint_reaps_stalled_handlers_and_joins_worker() {
    signal_case(libc::SIGINT, "SIGINT");
}

fn assert_final_report(report: &serde_json::Value) {
    assert_eq!(report["handlers_remaining"], 0);
    for key in [
        "accounted",
        "handlers_reaped",
        "worker_joined",
        "connection_closed",
        "gauges_zero",
        "socket_removed",
        "deadline_met",
    ] {
        assert_eq!(report[key], true, "{key}: {report}");
    }
    for gauge in Gauge::ALL {
        assert_eq!(report["metrics"][gauge.name()]["current"], 0, "{report}");
        assert!(report["metrics"][gauge.name()]["high_water"].is_u64());
    }
    for counter in crate::runtime_metrics::Counter::ALL {
        assert!(report["metrics"][counter.name()].is_u64(), "{report}");
    }
    assert_eq!(report["metrics"]["tracked_tasks"]["current"], 0);
    assert!(
        report["metrics"]["tracked_tasks"]["high_water"]
            .as_u64()
            .unwrap()
            <= report["limits"]["connections"].as_u64().unwrap()
    );
    assert!(
        report["metrics"]["connections"]["high_water"]
            .as_u64()
            .unwrap()
            <= report["limits"]["connections"].as_u64().unwrap()
    );
    assert!(
        report["metrics"]["requests"]["high_water"]
            .as_u64()
            .unwrap()
            <= report["limits"]["requests"].as_u64().unwrap()
    );
    assert!(
        report["metrics"]["db_queued"]["high_water"]
            .as_u64()
            .unwrap()
            <= report["limits"]["db_queue"].as_u64().unwrap()
    );
    assert_eq!(report["metrics"]["worker_started"], 1);
    assert_eq!(report["metrics"]["worker_joined"], 1);
    assert_eq!(
        report["metrics"]["handlers_spawned"],
        report["metrics"]["handlers_reaped"]
    );
    assert_eq!(report["supervisor"]["retry_tasks"], 0);
    assert_eq!(report["supervisor"]["supervisors"]["current"], 1);
    assert_eq!(report["supervisor"]["supervisors"]["high_water"], 1);
}

fn stop(child: &mut OwnedChild, signal: i32) {
    assert_eq!(
        unsafe { libc::kill(child.0.id() as libc::pid_t, signal) },
        0
    );
    wait_exit(child, true);
}

fn wait_exit(child: &mut OwnedChild, success: bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            assert_eq!(status.success(), success, "{status}");
            return;
        }
        assert!(
            Instant::now() < deadline,
            "child failed to exit within five seconds"
        );
        thread::sleep(Duration::from_millis(1));
    }
}

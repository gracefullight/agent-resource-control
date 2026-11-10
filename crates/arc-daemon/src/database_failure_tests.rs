//! Typed worker failures through the production adapter, transport and real CLI.

use super::*;
use crate::{admission::Requests, transport_test_support::request};
use arc_ipc::{DoctorReport, Request, ERROR_DB_BUSY, ERROR_INTEGRITY_FAILED, OPERATION_DOCTOR};
use std::{
    os::unix::fs::DirBuilderExt,
    process::{Child, Command, Output, Stdio},
    sync::atomic::{AtomicI32, AtomicU64, Ordering},
    time::Duration,
};
use tokio::{net::UnixStream, sync::oneshot, task::JoinHandle};

fn sql_error(code: i32) -> DbError {
    DbError::Sql(rusqlite::Error::SqliteFailure(
        rusqlite::ffi::Error::new(code),
        Some("private diagnostic must not reach the protocol".into()),
    ))
}

#[test]
fn operational_resource_and_unexpected_sql_errors_are_unavailable() {
    use rusqlite::ffi::*;
    for code in [
        SQLITE_IOERR,
        SQLITE_IOERR_READ,
        SQLITE_NOMEM,
        SQLITE_FULL,
        SQLITE_CANTOPEN,
        SQLITE_READONLY,
        SQLITE_PERM,
        SQLITE_AUTH,
        SQLITE_BUSY,
        SQLITE_LOCKED,
        SQLITE_INTERRUPT,
        SQLITE_ERROR,
        SQLITE_INTERNAL,
        SQLITE_CONSTRAINT,
    ] {
        assert!(
            matches!(database_failure(sql_error(code)), DoctorOutcome::Busy),
            "SQLite code {code} must be unavailable"
        );
    }
}

#[test]
fn non_sqlite_failures_default_to_unavailable() {
    for error in [
        rusqlite::Error::QueryReturnedNoRows,
        rusqlite::Error::InvalidQuery,
        rusqlite::Error::InvalidParameterName("private parameter".into()),
        rusqlite::Error::ExecuteReturnedResults,
    ] {
        assert!(matches!(
            database_failure(DbError::Sql(error)),
            DoctorOutcome::Busy
        ));
    }
}

#[test]
fn only_positive_sqlite_corruption_evidence_is_integrity_failure() {
    for code in [rusqlite::ffi::SQLITE_CORRUPT, rusqlite::ffi::SQLITE_NOTADB] {
        assert!(matches!(
            database_failure(sql_error(code)),
            DoctorOutcome::IntegrityFailed
        ));
    }
}

#[tokio::test]
async fn failed_integrity_and_schema_reports_keep_dispatch_validation() {
    for report in [
        DoctorReport {
            integrity_check: "failed integrity check".into(),
            schema_migrations: 1,
            schema: None,
        },
        DoctorReport {
            integrity_check: "ok".into(),
            schema_migrations: 0,
            schema: None,
        },
    ] {
        let expected = if report.integrity_check == "ok" {
            arc_ipc::ERROR_SCHEMA_DETAILS_UNAVAILABLE
        } else {
            ERROR_INTEGRITY_FAILED
        };
        let (limits, metrics) = crate::transport_test_support::setup();
        let requests = Requests::new(&limits, Arc::clone(&metrics)).unwrap();
        let (server, client) = UnixStream::pair().unwrap();
        let handler = tokio::spawn(connection::handle_connection(
            server,
            limits,
            requests,
            metrics,
            move |_| {
                let report = report.clone();
                async move { DoctorOutcome::Ready(report) }
            },
        ));
        let response = request(&client, &Request::new("invalid-report", OPERATION_DOCTOR)).await;
        assert_eq!(response.protocol_version, 1);
        assert_eq!(response.error.unwrap().code, expected);
        drop(client);
        tokio::time::timeout(Duration::from_secs(2), handler)
            .await
            .unwrap()
            .unwrap();
    }
}

struct FailureFixture {
    root: PathBuf,
    socket: PathBuf,
    fault: Arc<AtomicI32>,
    stop: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
}

impl FailureFixture {
    fn start() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = arc_core::runtime_root().unwrap().join(format!(
            "e{:x}-{:x}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let socket = root.join("arc.sock");
        let listener = server::bind_socket(&socket).unwrap();
        let connection = arc_storage::open_and_migrate(root.join("arc.sqlite3")).unwrap();
        let limits = RuntimeLimits::default();
        let metrics = Arc::new(RuntimeMetrics::new(&limits));
        let mut worker = DbWorker::start(
            connection,
            crate::db_worker::DbConfig {
                queue_capacity: limits.db_queue,
                busy_timeout: limits.db_busy_timeout,
                execution_timeout: limits.db_execution_timeout,
            },
            Arc::clone(&metrics),
        )
        .unwrap();
        let fault = Arc::new(AtomicI32::new(0));
        let injected = Arc::clone(&fault);
        let (stop, mut stopped) = oneshot::channel();
        // Binding completes before returning, so clients need no readiness sleep.
        let task = tokio::spawn(async move {
            let mut handlers = Handlers::new(&limits, Arc::clone(&metrics)).unwrap();
            let requests = Requests::new(&limits, Arc::clone(&metrics)).unwrap();
            loop {
                tokio::select! {
                    _ = &mut stopped => break,
                    result = handlers.join_next(), if handlers.len() > 0 => {
                        result.unwrap().unwrap();
                    }
                    accepted = listener.accept() => {
                        let (stream, _) = accepted.unwrap();
                        let fault = Arc::clone(&injected);
                        let client = worker.client();
                        let requests = requests.clone();
                        let metrics = Arc::clone(&metrics);
                        let limits = limits.clone();
                        assert!(handlers.try_spawn(stream, move |stream| {
                            connection::handle_connection(stream, limits, requests, metrics, move |deadline| {
                                let code = fault.load(Ordering::SeqCst);
                                let client = client.clone();
                                async move {
                                    if code == 0 {
                                        doctor(client, deadline).await
                                    } else {
                                        database_failure(sql_error(code))
                                    }
                                }
                            })
                        }));
                    }
                }
            }
            drop(listener);
            let deadline = Instant::now() + Duration::from_secs(2);
            worker.begin_shutdown();
            handlers.shutdown(deadline).await.unwrap();
            let exit = worker.shutdown(deadline).await.unwrap();
            assert!(exit.worker_joined && exit.connection_closed);
            let snapshot = metrics.snapshot();
            for gauge in Gauge::ALL {
                assert_eq!(snapshot.gauge(gauge).current, 0);
            }
            let counters = snapshot.json();
            assert_eq!(counters["handlers_spawned"], counters["handlers_reaped"]);
            assert_eq!(counters["worker_started"], 1);
            assert_eq!(counters["worker_joined"], 1);
        });
        Self {
            root,
            socket,
            fault,
            stop: Some(stop),
            task,
        }
    }

    async fn cli(&self, command: &str) -> Output {
        let cli = std::env::current_exe()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("arcctl");
        assert!(
            cli.is_file(),
            "offline CLI prerequisite missing: {}",
            cli.display()
        );
        let mut child = OwnedCli(
            Command::new(cli)
                .arg(command)
                .env(arc_core::ARC_SOCKET_PATH, &self.socket)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .stdin(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let deadline = Instant::now() + Duration::from_secs(3);
        while child.0.try_wait().unwrap().is_none() {
            assert!(Instant::now() < deadline, "owned CLI exceeded deadline");
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
        // Reaped above; read only the CLI's fixed, small output.
        use std::io::Read;
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        child
            .0
            .stdout
            .take()
            .unwrap()
            .take(4096)
            .read_to_end(&mut stdout)
            .unwrap();
        child
            .0
            .stderr
            .take()
            .unwrap()
            .take(4096)
            .read_to_end(&mut stderr)
            .unwrap();
        Output {
            status: child.0.wait().unwrap(),
            stdout,
            stderr,
        }
    }

    async fn finish(mut self) {
        self.stop.take().unwrap().send(()).unwrap();
        tokio::time::timeout(Duration::from_secs(3), &mut self.task)
            .await
            .unwrap()
            .unwrap();
        let reopened = arc_storage::open_and_migrate(self.root.join("arc.sqlite3")).unwrap();
        assert_eq!(
            reopened
                .query_row("PRAGMA integrity_check", [], |row| row.get::<_, String>(0))
                .unwrap(),
            "ok"
        );
        reopened.close().unwrap();
        eprintln!("classification-cleanup: handlers reaped; worker joined; gauges zero; database reopened; integrity ok");
    }
}

impl Drop for FailureFixture {
    fn drop(&mut self) {
        self.task.abort();
        let _ = fs::remove_dir_all(&self.root);
    }
}

struct OwnedCli(Child);
impl Drop for OwnedCli {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn typed_worker_errors_reach_protocol_and_actual_cli_then_recover() {
    let fixture = FailureFixture::start();
    for (code, wire, cli_error) in [
        (rusqlite::ffi::SQLITE_IOERR, ERROR_DB_BUSY, "IPC_ERROR\n"),
        (
            rusqlite::ffi::SQLITE_IOERR_READ,
            ERROR_DB_BUSY,
            "IPC_ERROR\n",
        ),
        (rusqlite::ffi::SQLITE_NOMEM, ERROR_DB_BUSY, "IPC_ERROR\n"),
        (
            rusqlite::ffi::SQLITE_CORRUPT,
            ERROR_INTEGRITY_FAILED,
            "INTEGRITY_FAILED\n",
        ),
        (
            rusqlite::ffi::SQLITE_NOTADB,
            ERROR_INTEGRITY_FAILED,
            "INTEGRITY_FAILED\n",
        ),
    ] {
        fixture.fault.store(code, Ordering::SeqCst);
        let stream = UnixStream::connect(&fixture.socket).await.unwrap();
        let response = request(&stream, &Request::new("typed-failure", OPERATION_DOCTOR)).await;
        assert_eq!(response.protocol_version, 1);
        assert_eq!(response.status, arc_ipc::STATUS_ERROR);
        assert_eq!(response.request_id, "typed-failure");
        let actual_wire = response.error.unwrap().code;
        assert!(response.result.is_none());
        drop(stream);
        let output = fixture.cli("doctor").await;
        assert!(!output.status.success(), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        assert_eq!(output.stderr, cli_error.as_bytes());
        assert_eq!(actual_wire, wire, "SQLite code {code}");
        fixture.fault.store(0, Ordering::SeqCst);
        let status = fixture.cli("status").await;
        assert!(
            status.status.success() && status.stderr.is_empty(),
            "{status:?}"
        );
        assert_eq!(
            String::from_utf8(status.stdout).unwrap(),
            format!(
                "running. Socket: {}. Protocol: 1. Sweeper off.\n",
                fixture.socket.display()
            )
        );
        let doctor = fixture.cli("doctor").await;
        assert!(
            doctor.status.success() && doctor.stderr.is_empty(),
            "{doctor:?}"
        );
        assert_eq!(doctor.stdout, b"ok. IPC owner-only. Tray not connected. Vendors not detected. Command classification not checked. Sweeper off. Schema compatible. Version 3. Migration history verified.\n");
        eprintln!(
            "classification-receipt: {}",
            serde_json::json!({"sqlite_code":code,"protocol":1,"wire":wire,"cli_stderr":cli_error,"status_recovered":true,"doctor_recovered":true})
        );
    }
    fixture.finish().await;
}

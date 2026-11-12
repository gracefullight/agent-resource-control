//! One supervisor owns admission, handler retention, SQLite and socket cleanup.

use crate::{
    accept_recovery::{Acceptor, Recovery},
    admission::{Handlers, Requests},
    connection::{self, DoctorOutcome},
    db_worker::{DbClient, DbError, DbWorker},
    limits::RuntimeLimits,
    runtime_metrics::{Counter, Gauge, RuntimeMetrics},
    runtime_report::{self, RuntimeReport},
    server,
};
use std::{
    error::Error,
    fs,
    io::{self, Write},
    os::unix::fs::MetadataExt,
    path::PathBuf,
    sync::Arc,
};
use tokio::{
    signal::unix::{signal, SignalKind},
    time::Instant,
};

pub(crate) async fn supervise(
    socket: PathBuf,
    limits: RuntimeLimits,
    metrics: Arc<RuntimeMetrics>,
    worker: DbWorker,
) -> Result<(), Box<dyn Error>> {
    supervise_with_acceptor(socket, limits, metrics, worker, Acceptor::default()).await
}

pub(crate) async fn supervise_with_acceptor(
    socket: PathBuf,
    limits: RuntimeLimits,
    metrics: Arc<RuntimeMetrics>,
    worker: DbWorker,
    acceptor: Acceptor,
) -> Result<(), Box<dyn Error>> {
    supervise_with_report(
        socket,
        limits,
        metrics,
        worker,
        acceptor,
        RuntimeReport::from_env(),
    )
    .await
}

pub(crate) async fn supervise_with_report(
    socket: PathBuf,
    limits: RuntimeLimits,
    metrics: Arc<RuntimeMetrics>,
    mut worker: DbWorker,
    mut acceptor: Acceptor,
    report: io::Result<RuntimeReport>,
) -> Result<(), Box<dyn Error>> {
    // Startup failures still close and join the worker before returning.
    let startup = (|| -> Result<_, Box<dyn Error>> {
        limits.validate()?;
        let handlers = Handlers::new(&limits, Arc::clone(&metrics))?;
        let requests = Requests::new(&limits, Arc::clone(&metrics))?;
        let terminate = signal(SignalKind::terminate())?;
        let interrupt = signal(SignalKind::interrupt())?;
        let report = report?;
        let listener = server::bind_socket(&socket)?;
        let owned_socket = OwnedSocket::new(socket)?;
        println!(
            "arc-daemon ready. Socket: {}. Protocol: 1. Sweeper off.",
            owned_socket.path.display()
        );
        io::stdout().flush()?;
        Ok((
            handlers,
            requests,
            terminate,
            interrupt,
            report,
            listener,
            owned_socket,
        ))
    })();
    let (mut handlers, requests, mut terminate, mut interrupt, report, listener, mut owned_socket) =
        match startup {
            Ok(startup) => startup,
            Err(error) => {
                worker.begin_shutdown();
                worker
                    .shutdown(Instant::now() + limits.shutdown_timeout)
                    .await?;
                return Err(error);
            }
        };
    let profile = match std::env::var_os("ARC_IPC_PROFILE").as_deref() {
        Some(value) if value == "test" => "test",
        _ => "default",
    };
    let mut recovery = Recovery::default();
    let client = worker.client();
    let mut meter = crate::meter::start();
    let meter_started = meter.is_some();
    if let Some(meter) = &meter {
        client.bind_meter(meter.activity());
    }
    let meter_reader = meter.as_ref().map(arc_meter::MeterWorker::reader);
    let (reason, accept_error) = loop {
        handlers.reap_ready();
        tokio::select! {
            biased;
            _ = terminate.recv() => break ("SIGTERM", None),
            _ = interrupt.recv() => break ("SIGINT", None),
            _ = handlers.join_next(), if handlers.len() != 0 => {},
            accepted = async {
                if let Some(at) = recovery.retry_at {
                    tokio::time::sleep_until(at).await;
                }
                acceptor.accept(&listener).await
            } => {
                match accepted {
                    Ok(stream) => {
                        recovery.accepted();
                        let limits = limits.clone();
                        let requests = requests.clone();
                        let metrics = Arc::clone(&metrics);
                        let client = client.clone();
                        let jobs_client = client.clone();
                        let traffic_client = client.clone();
                        let meter_client = client.clone();
                        let meter_reader = meter_reader.clone();
                        let job_meter: crate::job_meter_dispatch::Handler = Arc::new(move |input, peer, deadline| {
                            Box::pin(crate::db_worker::job_meter::handle(meter_client.clone(), input, peer, deadline))
                        });
                        handlers.try_spawn(stream, move |stream| {
                            connection::handle_with_job_meter(
                                stream, limits, requests, metrics,
                                move |deadline| doctor(client.clone(), deadline),
                                move |input, deadline| crate::db_worker::jobs::handle(jobs_client.clone(), input, deadline),
                                (move |input, peer, deadline| crate::db_worker::traffic::handle(traffic_client.clone(), input, peer, deadline), meter_reader, Some(job_meter)),
                            )
                        });
                    }
                    Err(error) => {
                        if !recovery.failed(&error) {
                            break ("accept_error", Some(error));
                        }
                        acceptor.retrying(&recovery);
                    }
                }
            }
        }
    };
    let shutdown_during_backoff = recovery.retry_at.is_some_and(|at| Instant::now() < at);
    let started = Instant::now();
    let deadline = started + limits.shutdown_timeout;
    drop(listener);
    if let Some(meter) = &meter {
        meter.begin_shutdown();
    }
    worker.begin_shutdown();
    // Both resources use the same absolute deadline; no stage renews it.
    let handler_result = handlers.shutdown(deadline).await;
    let worker_result = worker.shutdown(deadline).await;
    let meter_result = match &mut meter {
        Some(meter) => meter.shutdown(deadline).await,
        None => Ok(()),
    };
    let cleanup_result = owned_socket.remove();
    let snapshot = metrics.snapshot();
    let handlers_reaped = handlers.len() == 0
        && snapshot.counter(Counter::HandlersSpawned) == snapshot.counter(Counter::HandlersReaped);
    let worker_joined = worker_result.as_ref().is_ok_and(|exit| exit.worker_joined);
    let connection_closed = worker_result
        .as_ref()
        .is_ok_and(|exit| exit.connection_closed);
    let gauges_zero = Gauge::ALL
        .iter()
        .all(|gauge| snapshot.gauge(*gauge).current == 0)
        && snapshot.tracked_tasks.current == 0;
    let accounted = handlers_reaped
        && worker_joined
        && connection_closed
        && gauges_zero
        && snapshot.counter(Counter::WorkerStarted) == 1
        && snapshot.counter(Counter::WorkerJoined) == 1;
    let mut supervisor = recovery.json();
    supervisor["shutdown_during_backoff"] = shutdown_during_backoff.into();
    let final_report = serde_json::json!({
        "event": "ipc_shutdown", "reason": reason,
        "profiles": {"ipc": profile}, "limits": runtime_report::configuration(&limits),
        "shutdown_elapsed_ms": started.elapsed().as_millis(),
        "deadline_met": Instant::now() <= deadline,
        "handlers_remaining": handlers.len(),
        "handlers_reaped": handlers_reaped, "worker_joined": worker_joined,
        "connection_closed": connection_closed, "gauges_zero": gauges_zero,
        "socket_removed": cleanup_result.is_ok(), "accounted": accounted,
        "accept_error_os_code": accept_error.as_ref().and_then(io::Error::raw_os_error),
        "meter": { "started": meter_started, "shutdown_ok": meter_result.is_ok() },
        "metrics": snapshot.json(), "supervisor": supervisor,
    });
    // Serialization and I/O failures cannot bypass cancellation, join or cleanup.
    let report_result = report.finish(&final_report, deadline);
    handler_result?;
    worker_result?;
    cleanup_result?;
    report_result?;
    if !accounted || Instant::now() > deadline {
        return Err(io::Error::other("IPC shutdown accounting or deadline failed").into());
    }
    if let Some(error) = accept_error {
        return Err(error.into());
    }
    Ok(())
}

pub(crate) async fn doctor(client: DbClient, deadline: std::time::Instant) -> DoctorOutcome {
    let result = match client.try_doctor(deadline) {
        Ok(ticket) => ticket.receive().await,
        Err(error) => Err(error),
    };
    match result {
        Ok(report) => DoctorOutcome::Ready(report),
        Err(error) => database_failure(error),
    }
}

fn database_failure(error: DbError) -> DoctorOutcome {
    match error {
        DbError::Sql(rusqlite::Error::SqliteFailure(error, _))
            if matches!(
                error.code,
                rusqlite::ErrorCode::DatabaseCorrupt | rusqlite::ErrorCode::NotADatabase
            ) =>
        {
            DoctorOutcome::IntegrityFailed
        }
        // Operational, resource and unexpected errors establish unavailability,
        // not corruption. Failed integrity/schema reports are checked by dispatch.
        _ => DoctorOutcome::Busy,
    }
}

/// Remove only the socket inode this supervisor bound, including startup errors.
struct OwnedSocket {
    path: PathBuf,
    identity: (u64, u64),
    removed: bool,
}

impl OwnedSocket {
    fn new(path: PathBuf) -> io::Result<Self> {
        let metadata = fs::symlink_metadata(&path)?;
        Ok(Self {
            path,
            identity: (metadata.dev(), metadata.ino()),
            removed: false,
        })
    }

    fn remove(&mut self) -> io::Result<()> {
        match fs::symlink_metadata(&self.path) {
            Ok(metadata) if (metadata.dev(), metadata.ino()) == self.identity => {
                fs::remove_file(&self.path)?;
            }
            Ok(_) => return Err(io::Error::other("IPC socket changed during service")),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        self.removed = true;
        Ok(())
    }
}

impl Drop for OwnedSocket {
    fn drop(&mut self) {
        if !self.removed {
            let _ = self.remove();
        }
    }
}

#[cfg(test)]
#[path = "database_failure_tests.rs"]
mod failure_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_contention_and_unavailability_are_not_integrity_failures() {
        for code in [
            rusqlite::ffi::SQLITE_BUSY,
            rusqlite::ffi::SQLITE_LOCKED,
            rusqlite::ffi::SQLITE_INTERRUPT,
        ] {
            assert!(matches!(
                database_failure(DbError::Sql(rusqlite::Error::SqliteFailure(
                    rusqlite::ffi::Error::new(code),
                    None
                ))),
                DoctorOutcome::Busy
            ));
        }
        for error in [
            DbError::Busy,
            DbError::Stopping,
            DbError::Cancelled,
            DbError::Deadline,
            DbError::WorkerLost,
            DbError::ShutdownDeadline,
            DbError::Io(io::Error::other("worker failed")),
        ] {
            assert!(matches!(database_failure(error), DoctorOutcome::Busy));
        }
        assert!(matches!(
            database_failure(DbError::Sql(rusqlite::Error::SqliteFailure(
                rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CORRUPT),
                None
            ))),
            DoctorOutcome::IntegrityFailed
        ));
    }
}

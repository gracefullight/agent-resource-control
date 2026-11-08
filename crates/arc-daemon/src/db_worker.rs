//! One owned SQLite thread, a finite queue, and cancellation tied to tickets.

#[path = "db_worker/execution.rs"]
mod execution;
#[path = "db_worker/jobs.rs"]
pub(crate) mod jobs;
#[cfg(test)]
#[path = "db_worker/tests.rs"]
mod tests;

use arc_ipc::DoctorReport;
use rusqlite::{Connection, InterruptHandle};
use std::{
    fmt, io,
    num::NonZeroUsize,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle, Thread},
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, oneshot, OwnedSemaphorePermit, Semaphore};

use crate::runtime_metrics::{Counter, Gauge, GaugeGuard, RuntimeMetrics};

pub(crate) struct DbConfig {
    pub queue_capacity: NonZeroUsize,
    pub busy_timeout: Duration,
    pub execution_timeout: Duration,
}

#[derive(Debug)]
pub(crate) enum DbError {
    Busy,
    Stopping,
    Cancelled,
    Deadline,
    WorkerLost,
    ShutdownDeadline,
    Sql(rusqlite::Error),
    Migration(arc_storage::MigrationError),
    Io(io::Error),
}

impl fmt::Display for DbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Busy => "database queue or lock is busy",
            Self::Stopping => "database worker is stopping",
            Self::Cancelled => "database request cancelled",
            Self::Deadline => "database request deadline expired",
            Self::WorkerLost => "database worker terminated unexpectedly",
            Self::ShutdownDeadline => "database worker shutdown deadline expired",
            Self::Migration(_) => "database assessment failed",
            Self::Sql(_) => "database operation failed",
            Self::Io(_) => "database worker startup failed",
        })
    }
}

impl std::error::Error for DbError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Sql(error) => Some(error),
            Self::Migration(error) => Some(error),
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

pub(crate) struct DbWorker {
    client: DbClient,
    interrupt: InterruptHandle,
    thread: Option<JoinHandle<Result<(), DbError>>>,
    closed: bool,
}

#[derive(Clone)]
pub(crate) struct DbClient {
    sender: mpsc::Sender<Job>,
    queue_slots: Arc<Semaphore>,
    shared: Arc<Shared>,
    thread: Thread,
    metrics: Arc<RuntimeMetrics>,
}

pub(crate) struct DbTicket {
    reply: oneshot::Receiver<Result<DbValue, DbError>>,
    cancelled: Arc<AtomicBool>,
    deadline: Instant,
    metrics: Arc<RuntimeMetrics>,
}

pub(crate) struct DbExit {
    pub connection_closed: bool,
    pub worker_joined: bool,
}

struct Shared {
    stopping: AtomicBool,
    active: Mutex<Option<execution::Active>>,
}

pub(crate) enum DbValue {
    Doctor(DoctorReport),
    Lifecycle(Box<jobs::Value>),
}

struct Job {
    operation: Option<jobs::Operation>,
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
    reply: oneshot::Sender<Result<DbValue, DbError>>,
    queued: Option<GaugeGuard>,
    queue_slot: Option<OwnedSemaphorePermit>,
    _outstanding: GaugeGuard,
}

impl DbWorker {
    pub(crate) fn start(
        connection: Connection,
        config: DbConfig,
        metrics: Arc<RuntimeMetrics>,
    ) -> Result<Self, DbError> {
        Self::start_inner(connection, config, metrics, execution::Hooks::default())
    }

    fn start_inner(
        mut connection: Connection,
        config: DbConfig,
        metrics: Arc<RuntimeMetrics>,
        hooks: execution::Hooks,
    ) -> Result<Self, DbError> {
        if config.queue_capacity.get() > tokio::sync::Semaphore::MAX_PERMITS
            || config.busy_timeout.is_zero()
            || config.busy_timeout.as_millis() > i32::MAX as u128
            || config.execution_timeout < config.busy_timeout
            || Instant::now()
                .checked_add(config.execution_timeout)
                .is_none()
        {
            return Err(DbError::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid database limits",
            )));
        }
        let shared = Arc::new(Shared {
            stopping: AtomicBool::new(false),
            active: Mutex::new(None),
        });
        // Replace storage startup's five-second wait before announcing readiness.
        connection
            .busy_timeout(config.busy_timeout)
            .map_err(DbError::Sql)?;
        execution::install_progress(&mut connection, Arc::clone(&shared))?;
        execution::install_busy(
            &mut connection,
            Arc::clone(&shared),
            config.busy_timeout,
            &hooks,
        )?;
        let interrupt = connection.get_interrupt_handle();
        let capacity = config.queue_capacity.get();
        let (sender, receiver) = mpsc::channel(capacity);
        let worker_shared = Arc::clone(&shared);
        let worker_metrics = Arc::clone(&metrics);
        let thread = thread::Builder::new()
            .name("arc-sqlite".into())
            .spawn(move || {
                worker_metrics.increment(Counter::WorkerStarted);
                execution::run(
                    connection,
                    receiver,
                    worker_shared,
                    worker_metrics,
                    config,
                    hooks,
                )
            })
            .map_err(DbError::Io)?;
        let client = DbClient {
            sender,
            queue_slots: Arc::new(Semaphore::new(capacity)),
            shared,
            thread: thread.thread().clone(),
            metrics,
        };
        Ok(Self {
            client,
            interrupt,
            thread: Some(thread),
            closed: false,
        })
    }

    pub(crate) fn client(&self) -> DbClient {
        self.client.clone()
    }

    pub(crate) fn begin_shutdown(&mut self) {
        self.client.shared.stopping.store(true, Ordering::Release);
        self.interrupt.interrupt();
        self.client.thread.unpark();
    }

    pub(crate) async fn shutdown(
        &mut self,
        deadline: tokio::time::Instant,
    ) -> Result<DbExit, DbError> {
        self.begin_shutdown();
        while let Some(handle) = self.thread.as_ref() {
            if handle.is_finished() {
                let result = self.thread.take().expect("observed worker handle").join();
                self.client.metrics.increment(Counter::WorkerJoined);
                result.map_err(|_| DbError::WorkerLost)??;
                self.closed = true;
                break;
            }
            if tokio::time::Instant::now() >= deadline {
                // Keep ownership: a caller can retry shutdown and inspect failure.
                return Err(DbError::ShutdownDeadline);
            }
            tokio::time::sleep_until(
                deadline.min(tokio::time::Instant::now() + Duration::from_millis(1)),
            )
            .await;
        }
        if !self.closed {
            return Err(DbError::WorkerLost);
        }
        Ok(DbExit {
            connection_closed: self.closed,
            worker_joined: self.thread.is_none(),
        })
    }
}

impl Drop for DbWorker {
    fn drop(&mut self) {
        // Last-resort cleanup for startup/supervisor errors. Normal shutdown uses
        // the deadline-aware method and must report any failure before dropping.
        self.begin_shutdown();
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
            self.client.metrics.increment(Counter::WorkerJoined);
        }
    }
}

impl DbClient {
    pub(crate) fn try_doctor(&self, deadline: Instant) -> Result<DbTicket, DbError> {
        let result = self.admit(deadline, None);
        if result.is_err() {
            self.metrics.increment(Counter::DbRejected);
        }
        result
    }

    pub(crate) fn try_job(
        &self,
        deadline: Instant,
        operation: jobs::Operation,
    ) -> Result<DbTicket, DbError> {
        let result = self.admit(deadline, Some(operation));
        if result.is_err() {
            self.metrics.increment(Counter::DbRejected);
        }
        result
    }

    fn admit(
        &self,
        deadline: Instant,
        operation: Option<jobs::Operation>,
    ) -> Result<DbTicket, DbError> {
        if self.shared.stopping.load(Ordering::Acquire) {
            return Err(DbError::Stopping);
        }
        if Instant::now() >= deadline {
            return Err(DbError::Deadline);
        }
        // Keep queue ownership through dequeue accounting: mpsc releases its
        // slot before the received job's queue gauge can be dropped.
        let queue_slot = Arc::clone(&self.queue_slots)
            .try_acquire_owned()
            .map_err(|_| DbError::Busy)?;
        if self.shared.stopping.load(Ordering::Acquire) {
            return Err(DbError::Stopping);
        }
        let permit = self.sender.try_reserve().map_err(|error| match error {
            mpsc::error::TrySendError::Full(_) => DbError::Busy,
            mpsc::error::TrySendError::Closed(_) => DbError::WorkerLost,
        })?;
        let (reply, receiver) = oneshot::channel();
        let cancelled = Arc::new(AtomicBool::new(false));
        permit.send(Job {
            operation,
            deadline,
            cancelled: Arc::clone(&cancelled),
            reply,
            queued: Some(self.metrics.enter(Gauge::DbQueued, 1)),
            queue_slot: Some(queue_slot),
            _outstanding: self.metrics.enter(Gauge::DbOutstanding, 1),
        });
        self.thread.unpark();
        Ok(DbTicket {
            reply: receiver,
            cancelled,
            deadline,
            metrics: Arc::clone(&self.metrics),
        })
    }
}

impl DbTicket {
    pub(crate) async fn receive(self) -> Result<DoctorReport, DbError> {
        match self.receive_value().await? {
            DbValue::Doctor(report) => Ok(report),
            _ => Err(DbError::WorkerLost),
        }
    }

    pub(crate) async fn receive_value(mut self) -> Result<DbValue, DbError> {
        match tokio::time::timeout_at(
            tokio::time::Instant::from_std(self.deadline),
            &mut self.reply,
        )
        .await
        {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(DbError::WorkerLost),
            Err(_) => {
                self.metrics.increment(Counter::DbTimeouts);
                Err(DbError::Deadline)
            }
        }
    }
}

impl Drop for DbTicket {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }
}

#[cfg(test)]
pub(crate) struct WorkerTestHooks {
    pub started: std::sync::mpsc::SyncSender<()>,
    pub release: std::sync::mpsc::Receiver<()>,
}

#[cfg(test)]
pub(crate) fn start_with_test_hooks(
    connection: Connection,
    config: DbConfig,
    metrics: Arc<RuntimeMetrics>,
    hooks: WorkerTestHooks,
) -> Result<DbWorker, DbError> {
    DbWorker::start_inner(connection, config, metrics, execution::Hooks::gate(hooks))
}

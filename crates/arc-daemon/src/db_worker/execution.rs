use super::*;

pub(super) struct Active {
    cancelled: Arc<AtomicBool>,
    deadline: Instant,
    #[cfg(test)]
    progress: Option<std::sync::mpsc::SyncSender<()>>,
}

impl Active {
    fn check(&self, shared: &Shared) -> Result<(), DbError> {
        if shared.stopping.load(Ordering::Acquire) {
            return Err(DbError::Stopping);
        }
        if self.cancelled.load(Ordering::Acquire) {
            return Err(DbError::Cancelled);
        }
        if Instant::now() >= self.deadline {
            return Err(DbError::Deadline);
        }
        Ok(())
    }
}

pub(super) fn install_progress(
    connection: &mut Connection,
    shared: Arc<Shared>,
) -> Result<(), DbError> {
    connection
        .progress_handler(
            1_000,
            Some(move || {
                if shared.stopping.load(Ordering::Acquire) {
                    return true;
                }
                shared
                    .active
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .as_ref()
                    .is_some_and(|active| {
                        #[cfg(test)]
                        if let Some(sender) = active.progress.as_ref() {
                            let _ = sender.try_send(());
                        }
                        active.check(&shared).is_err()
                    })
            }),
        )
        .map_err(DbError::Sql)
}

pub(super) fn install_busy(
    connection: &mut Connection,
    shared: Arc<Shared>,
    budget: Duration,
    _hooks: &Hooks,
) -> Result<(), DbError> {
    #[cfg(test)]
    let observed = _hooks.lock_probe.clone();
    let mut end = Instant::now();
    connection
        .busy_handler(Some(move |attempt| {
            #[cfg(test)]
            if let Some(sender) = observed.as_ref() {
                let _ = sender.try_send(());
            }
            if attempt == 0 {
                end = Instant::now() + budget;
            }
            if check(&shared).is_err() || Instant::now() >= end {
                return false;
            }
            // SQLite's callback runs on its sole owning thread; cancellation and
            // deadlines are checked between bounded native lock retry slices.
            thread::sleep(
                Duration::from_millis(1).min(end.saturating_duration_since(Instant::now())),
            );
            check(&shared).is_ok() && Instant::now() < end
        }))
        .map_err(DbError::Sql)
}

pub(super) fn run(
    mut connection: Connection,
    mut receiver: mpsc::Receiver<Job>,
    shared: Arc<Shared>,
    metrics: Arc<RuntimeMetrics>,
    config: DbConfig,
    mut hooks: Hooks,
) -> Result<(), DbError> {
    loop {
        let job = {
            // The extra admission permit is retained until after the queue
            // gauge drops, even though mpsc has already freed its own slot.
            if shared.stopping.load(Ordering::Acquire) {
                receiver.close();
            }
            receiver.try_recv().map(|mut job| {
                job.queued.take();
                job.queue_slot.take();
                job
            })
        };
        match job {
            Ok(job) => {
                let executing = metrics.enter(Gauge::DbExecuting, 1);
                let active = Active {
                    cancelled: Arc::clone(&job.cancelled),
                    deadline: job.deadline.min(Instant::now() + config.execution_timeout),
                    #[cfg(test)]
                    progress: hooks.expensive.clone(),
                };
                *shared.active.lock().unwrap_or_else(|e| e.into_inner()) = Some(active);
                let result = if let Some(operation) = job.operation {
                    check(&shared)
                        .and_then(|()| hooks.before_sql(&shared))
                        .and_then(|()| {
                            check(&shared)?;
                            let value = jobs::execute(&mut connection, operation);
                            // A deadline here can follow COMMIT. The stable key resolves ambiguity.
                            check(&shared)?;
                            Ok(DbValue::Lifecycle(Box::new(value)))
                        })
                } else {
                    execute(&connection, &shared, &mut hooks).map(DbValue::Doctor)
                };
                if matches!(result, Err(DbError::Deadline)) {
                    metrics.increment(Counter::DbTimeouts);
                }
                // Ensure all job ownership is gone before receiving another job.
                *shared.active.lock().unwrap_or_else(|e| e.into_inner()) = None;
                let _ = job.reply.send(result);
                drop(executing);
                drop(job._outstanding);
            }
            Err(mpsc::error::TryRecvError::Disconnected) => break,
            Err(mpsc::error::TryRecvError::Empty) => {
                if shared.stopping.load(Ordering::Acquire) {
                    continue;
                }
                hooks.before_park(&shared);
                // unpark retains its token across the empty-check/park race.
                thread::park();
            }
        }
    }
    connection
        .close()
        .map_err(|(_, error)| DbError::Sql(error))?;
    #[cfg(test)]
    if let Some(exit) = hooks.exit.take() {
        let _ = exit.started.try_send(());
        let end = Instant::now() + Duration::from_secs(1);
        loop {
            match exit.release.recv_timeout(Duration::from_millis(5)) {
                Ok(()) | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) if Instant::now() < end => (),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => break,
            }
        }
    }
    Ok(())
}

fn check(shared: &Shared) -> Result<(), DbError> {
    shared
        .active
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .ok_or(DbError::WorkerLost)?
        .check(shared)
}

fn execute(
    connection: &Connection,
    shared: &Shared,
    hooks: &mut Hooks,
) -> Result<DoctorReport, DbError> {
    #[cfg(test)]
    let expensive = hooks.expensive.take().is_some();
    #[cfg(test)]
    let lock_probe = hooks.lock_probe.take().is_some();
    check(shared)?;
    hooks.before_sql(shared)?;
    check(shared)?;
    #[cfg(test)]
    if expensive {
        query(connection, shared, || {
            connection.query_row(
            "WITH RECURSIVE work(n) AS (VALUES(0) UNION ALL SELECT n+1 FROM work WHERE n<1000000000) SELECT sum(n) FROM work",
            [], |row| row.get::<_, i64>(0),
        )
        })?;
    }
    #[cfg(test)]
    if lock_probe {
        // Fixed write fixture to provoke a WAL writer lock without exposing SQL.
        query(connection, shared, || {
            connection.execute_batch("BEGIN IMMEDIATE; ROLLBACK;")
        })?;
    }
    // Fixed read-only diagnostics use this worker and its cancellation budget.
    let integrity_check = query(connection, shared, || {
        connection.query_row("PRAGMA integrity_check", [], |row| row.get(0))
    })?;
    check(shared)?;
    let assessment = arc_storage::assess_schema(connection);
    check(shared)?;
    let assessment = match assessment {
        Ok(assessment) => assessment,
        Err(arc_storage::MigrationError::Sql(error)) => {
            return query(connection, shared, || Err(error));
        }
        Err(error) => return Err(DbError::Migration(error)),
    };
    use arc_ipc::{SchemaReport, SchemaStatus as Wire};
    use arc_storage::SchemaStatus as Storage;
    let status = match assessment.status {
        Storage::Compatible => Wire::Compatible,
        Storage::UpgradeRequired => Wire::UpgradeRequired,
        Storage::MissingHistory => Wire::MissingHistory,
        Storage::ChecksumMismatch => Wire::ChecksumMismatch,
        Storage::InvalidHistory => Wire::InvalidHistory,
        Storage::UnsupportedNewer => Wire::UnsupportedNewer,
    };
    Ok(DoctorReport {
        integrity_check,
        schema_migrations: assessment.schema_migrations,
        schema: Some(SchemaReport {
            status,
            applied_version: assessment.applied_version,
            supported_version: assessment.supported_version,
            history_verified: assessment.history_verified,
        }),
    })
}

pub(super) fn query<T>(
    _: &Connection,
    shared: &Shared,
    sql: impl FnOnce() -> rusqlite::Result<T>,
) -> Result<T, DbError> {
    check(shared)?;
    let result = sql();
    // Sticky checks on both sides also cover an interrupt before SQL starts,
    // including statements too small to invoke a progress callback.
    check(shared)?;
    result.map_err(|error| match error.sqlite_error_code() {
        Some(rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked) => {
            DbError::Busy
        }
        Some(rusqlite::ErrorCode::OperationInterrupted) => DbError::Cancelled,
        _ => DbError::Sql(error),
    })
}

#[derive(Default)]
pub(super) struct Hooks {
    #[cfg(test)]
    pub(super) gate: Option<WorkerTestHooks>,
    #[cfg(test)]
    pub(super) expensive: Option<std::sync::mpsc::SyncSender<()>>,
    #[cfg(test)]
    pub(super) park: Option<WorkerTestHooks>,
    #[cfg(test)]
    pub(super) lock_probe: Option<std::sync::mpsc::SyncSender<()>>,
    #[cfg(test)]
    pub(super) exit: Option<WorkerTestHooks>,
}

impl Hooks {
    #[cfg(test)]
    pub(super) fn gate(hooks: WorkerTestHooks) -> Self {
        Self {
            gate: Some(hooks),
            ..Self::default()
        }
    }

    fn before_park(&mut self, _shared: &Shared) {
        #[cfg(test)]
        if let Some(hooks) = self.park.take() {
            let _ = hooks.started.try_send(());
            let end = Instant::now() + Duration::from_secs(2);
            while !_shared.stopping.load(Ordering::Acquire) && Instant::now() < end {
                // recv_timeout uses thread::park internally and could consume
                // the very unpark token this race fixture is meant to prove.
                match hooks.release.try_recv() {
                    Ok(()) | Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        thread::sleep(Duration::from_millis(1))
                    }
                }
            }
        }
    }

    fn before_sql(&mut self, _shared: &Shared) -> Result<(), DbError> {
        #[cfg(test)]
        if let Some(hooks) = self.gate.take() {
            let _ = hooks.started.try_send(());
            loop {
                check(_shared)?;
                match hooks.release.recv_timeout(Duration::from_millis(5)) {
                    Ok(()) => break,
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => (),
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        return Err(DbError::Cancelled)
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod race_tests {
    use super::*;

    #[test]
    fn stop_after_final_precheck_before_sql_is_sticky_even_with_early_interrupt() {
        let mut connection = Connection::open_in_memory().unwrap();
        let shared = Arc::new(Shared {
            stopping: AtomicBool::new(false),
            active: Mutex::new(Some(Active {
                cancelled: Arc::new(AtomicBool::new(false)),
                deadline: Instant::now() + Duration::from_secs(1),
                progress: None,
            })),
        });
        install_progress(&mut connection, Arc::clone(&shared)).unwrap();
        let interrupt = connection.get_interrupt_handle();
        let result = query(&connection, &shared, || {
            // query has performed its final precheck; stop lands before SQL.
            shared.stopping.store(true, Ordering::Release);
            interrupt.interrupt();
            let error = connection.query_row(
                "WITH RECURSIVE work(n) AS (VALUES(0) UNION ALL SELECT n+1 FROM work WHERE n<1000000000) SELECT sum(n) FROM work",
                [], |row| row.get::<_, i64>(0),
            ).unwrap_err();
            assert_eq!(
                error.sqlite_error_code(),
                Some(rusqlite::ErrorCode::OperationInterrupted)
            );
            Err::<(), _>(error)
        });
        assert!(matches!(result, Err(DbError::Stopping)));
        connection.close().unwrap();
    }
}

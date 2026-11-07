//! No-wait admission and bounded retention of completed and running handlers.

use crate::{
    limits::RuntimeLimits,
    platform,
    runtime_metrics::{Counter, Gauge, GaugeGuard, RuntimeMetrics},
};
use std::{future::Future, io, sync::Arc};
use tokio::{
    net::UnixStream,
    sync::{OwnedSemaphorePermit, Semaphore},
    task::{JoinError, JoinSet},
    time::{timeout_at, Instant},
};

pub(crate) struct Handlers {
    tasks: JoinSet<()>,
    permits: Arc<Semaphore>,
    cap: usize,
    metrics: Arc<RuntimeMetrics>,
    stopping: bool,
}

impl Handlers {
    pub(crate) fn new(limits: &RuntimeLimits, metrics: Arc<RuntimeMetrics>) -> io::Result<Self> {
        limits.validate()?;
        Ok(Self {
            tasks: JoinSet::new(),
            permits: Arc::new(Semaphore::new(limits.connections.get())),
            cap: limits.connections.get(),
            metrics,
            stopping: false,
        })
    }

    /// The factory is invoked only after authentication and nonwaiting admission.
    pub(crate) fn try_spawn<F, Make>(&mut self, stream: UnixStream, make: Make) -> bool
    where
        F: Future<Output = ()> + Send + 'static,
        Make: FnOnce(UnixStream) -> F,
    {
        self.reap_ready();
        if self.stopping
            || platform::ensure_same_user(&stream).is_err()
            || self.tasks.len() >= self.cap
        {
            return self.reject(stream);
        }
        let Ok(permit) = Arc::clone(&self.permits).try_acquire_owned() else {
            return self.reject(stream);
        };
        let admission = RequestGuard {
            _gauge: self.metrics.enter(Gauge::Connections, 1),
            _permit: permit,
        };
        let future = make(stream);
        self.tasks.spawn(async move {
            let _admission = admission;
            future.await;
        });
        self.metrics.increment(Counter::Accepted);
        self.metrics.increment(Counter::HandlersSpawned);
        self.metrics.set_tracked_tasks(self.tasks.len());
        true
    }

    fn reject(&self, stream: UnixStream) -> bool {
        let _guard = self.metrics.enter(Gauge::RejectionWork, 1);
        self.metrics.increment(Counter::ConnectionRejected);
        drop(stream);
        false
    }

    pub(crate) fn len(&self) -> usize {
        self.tasks.len()
    }

    pub(crate) fn reap_ready(&mut self) {
        while self.tasks.try_join_next().is_some() {
            self.reaped();
        }
    }

    /// Select alongside accept/signals; cancellation of this future is safe.
    pub(crate) async fn join_next(&mut self) -> Option<Result<(), JoinError>> {
        let result = self.tasks.join_next().await;
        if result.is_some() {
            self.reaped();
        }
        result
    }

    fn reaped(&self) {
        self.metrics.increment(Counter::HandlersReaped);
        self.metrics.set_tracked_tasks(self.tasks.len());
    }

    pub(crate) async fn shutdown(&mut self, deadline: Instant) -> io::Result<()> {
        self.stopping = true;
        self.tasks.abort_all();
        while !self.tasks.is_empty() {
            timeout_at(deadline, self.join_next()).await.map_err(|_| {
                io::Error::new(io::ErrorKind::TimedOut, "handler shutdown deadline expired")
            })?;
        }
        Ok(())
    }
}

#[derive(Clone)]
pub(crate) struct Requests {
    permits: Arc<Semaphore>,
    metrics: Arc<RuntimeMetrics>,
}

impl Requests {
    pub(crate) fn new(limits: &RuntimeLimits, metrics: Arc<RuntimeMetrics>) -> io::Result<Self> {
        limits.validate()?;
        Ok(Self {
            permits: Arc::new(Semaphore::new(limits.requests.get())),
            metrics,
        })
    }

    pub(crate) fn try_admit(&self) -> Option<RequestGuard> {
        match Arc::clone(&self.permits).try_acquire_owned() {
            Ok(permit) => Some(RequestGuard {
                _permit: permit,
                _gauge: self.metrics.enter(Gauge::Requests, 1),
            }),
            Err(_) => {
                self.metrics.increment(Counter::RequestRejected);
                None
            }
        }
    }
}

// Decrement accounting before releasing admission; concurrent replacement must
// never temporarily count the old and new owner against the same permit.
pub(crate) struct RequestGuard {
    _gauge: GaugeGuard,
    _permit: OwnedSemaphorePermit,
}

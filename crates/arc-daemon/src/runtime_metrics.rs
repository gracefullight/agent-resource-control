//! Fixed-size resource accounting. Snapshots are independently atomic gauges.

use crate::limits::RuntimeLimits;
use std::sync::{
    atomic::{AtomicU64, AtomicUsize, Ordering::SeqCst},
    Arc,
};

#[derive(Clone, Copy, Debug)]
#[repr(usize)]
pub(crate) enum Gauge {
    Connections,
    Requests,
    RequestBodyBytes,
    ResponseBytes,
    DbQueued,
    DbExecuting,
    DbOutstanding,
    RejectionWork,
}

impl Gauge {
    pub(crate) const ALL: [Self; 8] = [
        Self::Connections,
        Self::Requests,
        Self::RequestBodyBytes,
        Self::ResponseBytes,
        Self::DbQueued,
        Self::DbExecuting,
        Self::DbOutstanding,
        Self::RejectionWork,
    ];
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Connections => "connections",
            Self::Requests => "requests",
            Self::RequestBodyBytes => "request_body_bytes",
            Self::ResponseBytes => "response_bytes",
            Self::DbQueued => "db_queued",
            Self::DbExecuting => "db_executing",
            Self::DbOutstanding => "db_outstanding",
            Self::RejectionWork => "rejection_work",
        }
    }
}

#[derive(Clone, Copy, Debug)]
#[repr(usize)]
pub(crate) enum Counter {
    Accepted,
    ConnectionRejected,
    RequestRejected,
    DbRejected,
    FramesCompleted,
    HeaderTimeouts,
    FrameTimeouts,
    WriteTimeouts,
    DbTimeouts,
    WorkerStarted,
    WorkerJoined,
    HandlersSpawned,
    HandlersReaped,
}

impl Counter {
    pub(crate) const ALL: [Self; 13] = [
        Self::Accepted,
        Self::ConnectionRejected,
        Self::RequestRejected,
        Self::DbRejected,
        Self::FramesCompleted,
        Self::HeaderTimeouts,
        Self::FrameTimeouts,
        Self::WriteTimeouts,
        Self::DbTimeouts,
        Self::WorkerStarted,
        Self::WorkerJoined,
        Self::HandlersSpawned,
        Self::HandlersReaped,
    ];
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::ConnectionRejected => "connection_rejected",
            Self::RequestRejected => "request_rejected",
            Self::DbRejected => "db_rejected",
            Self::FramesCompleted => "frames_completed",
            Self::HeaderTimeouts => "header_timeouts",
            Self::FrameTimeouts => "frame_timeouts",
            Self::WriteTimeouts => "write_timeouts",
            Self::DbTimeouts => "db_timeouts",
            Self::WorkerStarted => "worker_started",
            Self::WorkerJoined => "worker_joined",
            Self::HandlersSpawned => "handlers_spawned",
            Self::HandlersReaped => "handlers_reaped",
        }
    }
}

#[derive(Default)]
struct AtomicGauge {
    current: AtomicUsize,
    high_water: AtomicUsize,
}

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct GaugeSnapshot {
    pub current: usize,
    pub high_water: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct Snapshot {
    pub gauges: [GaugeSnapshot; 8],
    pub counters: [u64; 13],
    pub tracked_tasks: GaugeSnapshot,
    pub connection_cap: usize,
    pub request_cap: usize,
    pub db_queue_cap: usize,
}

impl Snapshot {
    pub(crate) fn gauge(&self, gauge: Gauge) -> GaugeSnapshot {
        self.gauges[gauge as usize]
    }
    pub(crate) fn counter(&self, counter: Counter) -> u64 {
        self.counters[counter as usize]
    }

    /// Fixed keys and integer values only; no request-derived labels or history.
    pub(crate) fn json(&self) -> serde_json::Value {
        let mut result = serde_json::Map::new();
        for gauge in Gauge::ALL {
            let value = self.gauge(gauge);
            result.insert(
                gauge.name().into(),
                serde_json::json!({
                    "current": value.current, "high_water": value.high_water,
                }),
            );
        }
        for counter in Counter::ALL {
            result.insert(counter.name().into(), self.counter(counter).into());
        }
        result.insert(
            "tracked_tasks".into(),
            serde_json::json!({
                "current": self.tracked_tasks.current, "high_water": self.tracked_tasks.high_water,
            }),
        );
        result.insert(
            "caps".into(),
            serde_json::json!({
                "connections": self.connection_cap, "requests": self.request_cap,
                "db_queue": self.db_queue_cap, "db_executing": 1, "supervisor": 1,
            }),
        );
        result.into()
    }
}

pub(crate) struct RuntimeMetrics {
    gauges: [AtomicGauge; 8],
    counters: [AtomicU64; 13],
    tracked_tasks: AtomicGauge,
    caps: [usize; 3],
    #[cfg(test)]
    changed: tokio::sync::Notify,
}

impl RuntimeMetrics {
    pub(crate) fn new(limits: &RuntimeLimits) -> Self {
        Self {
            gauges: std::array::from_fn(|_| AtomicGauge::default()),
            counters: std::array::from_fn(|_| AtomicU64::new(0)),
            tracked_tasks: AtomicGauge::default(),
            caps: [
                limits.connections.get(),
                limits.requests.get(),
                limits.db_queue.get(),
            ],
            #[cfg(test)]
            changed: tokio::sync::Notify::new(),
        }
    }

    pub(crate) fn enter(self: &Arc<Self>, gauge: Gauge, amount: usize) -> GaugeGuard {
        let value = &self.gauges[gauge as usize];
        let previous = value
            .current
            .fetch_update(SeqCst, SeqCst, |n| n.checked_add(amount))
            .expect("resource gauge overflow");
        value.high_water.fetch_max(previous + amount, SeqCst);
        self.notify();
        GaugeGuard {
            metrics: Arc::clone(self),
            gauge,
            amount,
        }
    }

    pub(crate) fn increment(&self, counter: Counter) {
        let _ = self.counters[counter as usize]
            .fetch_update(SeqCst, SeqCst, |n| Some(n.saturating_add(1)));
        self.notify();
    }

    pub(crate) fn set_tracked_tasks(&self, count: usize) {
        self.tracked_tasks.current.store(count, SeqCst);
        self.tracked_tasks.high_water.fetch_max(count, SeqCst);
        self.notify();
    }

    pub(crate) fn snapshot(&self) -> Snapshot {
        fn read(value: &AtomicGauge) -> GaugeSnapshot {
            GaugeSnapshot {
                current: value.current.load(SeqCst),
                high_water: value.high_water.load(SeqCst),
            }
        }
        Snapshot {
            gauges: std::array::from_fn(|n| read(&self.gauges[n])),
            counters: std::array::from_fn(|n| self.counters[n].load(SeqCst)),
            tracked_tasks: read(&self.tracked_tasks),
            connection_cap: self.caps[0],
            request_cap: self.caps[1],
            db_queue_cap: self.caps[2],
        }
    }

    fn notify(&self) {
        #[cfg(test)]
        self.changed.notify_waiters();
    }

    #[cfg(test)]
    pub(crate) async fn wait_for(&self, predicate: impl Fn(&Snapshot) -> bool) {
        loop {
            let notified = self.changed.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();
            if predicate(&self.snapshot()) {
                return;
            }
            notified.await;
        }
    }
}

pub(crate) struct GaugeGuard {
    metrics: Arc<RuntimeMetrics>,
    gauge: Gauge,
    amount: usize,
}

impl Drop for GaugeGuard {
    fn drop(&mut self) {
        self.metrics.gauges[self.gauge as usize]
            .current
            .fetch_sub(self.amount, SeqCst);
        self.metrics.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifetime_counters_saturate_instead_of_wrapping() {
        let metrics = RuntimeMetrics::new(&RuntimeLimits::default());
        metrics.counters[Counter::Accepted as usize].store(u64::MAX, SeqCst);
        metrics.increment(Counter::Accepted);
        assert_eq!(metrics.snapshot().counter(Counter::Accepted), u64::MAX);
    }
}

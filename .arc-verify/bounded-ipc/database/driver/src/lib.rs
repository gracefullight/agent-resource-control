//! Actor-local driver: real worker and SQLite, metrics interface test double.
//! The IPC Actor owns production metrics and daemon module declarations.

#[allow(dead_code)]
#[path = "../../../../../crates/arc-daemon/src/db_worker.rs"]
mod db_worker;

#[allow(dead_code)]
mod runtime_metrics {
    use std::sync::{
        atomic::{AtomicU64, AtomicUsize, Ordering},
        Arc,
    };

    #[derive(Clone, Copy)]
    pub(crate) enum Gauge {
        DbQueued,
        DbExecuting,
        DbOutstanding,
        Connections,
        Requests,
        RequestBodyBytes,
        ResponseBytes,
        RejectionWork,
    }
    #[derive(Clone, Copy)]
    pub(crate) enum Counter {
        WorkerStarted,
        WorkerJoined,
        DbRejected,
        DbTimeouts,
    }

    #[derive(Default)]
    pub(crate) struct RuntimeMetrics {
        current: [AtomicUsize; 8],
        high: [AtomicUsize; 8],
        counters: [AtomicU64; 4],
    }

    pub(crate) struct GaugeGuard {
        metrics: Arc<RuntimeMetrics>,
        index: usize,
        amount: usize,
    }

    impl RuntimeMetrics {
        pub(crate) fn enter(self: &Arc<Self>, gauge: Gauge, amount: usize) -> GaugeGuard {
            let index = gauge as usize;
            let value = self.current[index].fetch_add(amount, Ordering::SeqCst) + amount;
            self.high[index].fetch_max(value, Ordering::SeqCst);
            let limit = [
                2,
                1,
                3,
                usize::MAX,
                usize::MAX,
                usize::MAX,
                usize::MAX,
                usize::MAX,
            ][index];
            assert!(
                value <= limit,
                "worker gauge {index} exceeded {limit}: {value}"
            );
            GaugeGuard {
                metrics: Arc::clone(self),
                index,
                amount,
            }
        }

        pub(crate) fn increment(&self, counter: Counter) {
            self.counters[counter as usize].fetch_add(1, Ordering::SeqCst);
        }
    }

    impl Drop for GaugeGuard {
        fn drop(&mut self) {
            let old = self.metrics.current[self.index].fetch_sub(self.amount, Ordering::SeqCst);
            assert!(old >= self.amount, "unbalanced worker gauge");
        }
    }

    impl Drop for RuntimeMetrics {
        fn drop(&mut self) {
            for gauge in &self.current {
                assert_eq!(gauge.load(Ordering::SeqCst), 0, "retained database work");
            }
            assert_eq!(
                self.counters[0].load(Ordering::SeqCst),
                1,
                "one native worker must start"
            );
            assert_eq!(
                self.counters[1].load(Ordering::SeqCst),
                1,
                "actual native worker must join"
            );
            eprintln!("worker-accounting: queued_high={} executing_high={} outstanding_high={} started=1 joined=1 final_gauges=0 rejected={} timeouts={}",
                self.high[0].load(Ordering::SeqCst), self.high[1].load(Ordering::SeqCst), self.high[2].load(Ordering::SeqCst),
                self.counters[2].load(Ordering::SeqCst), self.counters[3].load(Ordering::SeqCst));
        }
    }
}

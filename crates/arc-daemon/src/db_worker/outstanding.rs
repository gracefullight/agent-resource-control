//! Track the same admitted client jobs as `DbOutstanding`, including every
//! transition to zero. Maintenance never enters this tracker.
use crate::runtime_metrics::{Gauge, GaugeGuard, RuntimeMetrics};
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub(super) struct Tracker {
    state: Mutex<State>,
}

#[derive(Clone, Copy, Debug, Default)]
struct State {
    current: usize,
    zero_transitions: u64,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Snapshot(State);

impl Snapshot {
    /// True only when a client job was outstanding throughout the interval.
    pub(super) fn continuous_until(self, end: Self) -> bool {
        self.0.current > 0 && end.0.current > 0 && self.0.zero_transitions == end.0.zero_transitions
    }
}

impl Tracker {
    pub(super) fn enter(self: &Arc<Self>, metrics: &Arc<RuntimeMetrics>) -> Guard {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        let gauge = metrics.enter(Gauge::DbOutstanding, 1);
        state.current = state.current.checked_add(1).expect("client count overflow");
        Guard {
            tracker: Arc::clone(self),
            gauge: Some(gauge),
        }
    }

    pub(super) fn snapshot(&self) -> Snapshot {
        Snapshot(*self.state.lock().unwrap_or_else(|e| e.into_inner()))
    }
}

pub(super) struct Guard {
    tracker: Arc<Tracker>,
    gauge: Option<GaugeGuard>,
}

impl Drop for Guard {
    fn drop(&mut self) {
        let mut state = self.tracker.state.lock().unwrap_or_else(|e| e.into_inner());
        drop(self.gauge.take());
        state.current -= 1;
        if state.current == 0 {
            state.zero_transitions = state
                .zero_transitions
                .checked_add(1)
                .expect("client zero-transition count overflow");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::limits::RuntimeLimits;

    #[test]
    fn detects_an_idle_gap_even_when_both_snapshots_are_busy() {
        let tracker = Arc::new(Tracker::default());
        let metrics = Arc::new(RuntimeMetrics::new(&RuntimeLimits::default()));
        let first = tracker.enter(&metrics);
        let before = tracker.snapshot();
        let second = tracker.enter(&metrics);
        drop(first);
        assert!(before.continuous_until(tracker.snapshot()));
        assert_eq!(metrics.snapshot().gauge(Gauge::DbOutstanding).current, 1);
        drop(second);
        let third = tracker.enter(&metrics);
        let after = tracker.snapshot();
        assert!(!before.continuous_until(after));
        assert!(!after.continuous_until({
            drop(third);
            tracker.snapshot()
        }));
        assert_eq!(metrics.snapshot().gauge(Gauge::DbOutstanding).current, 0);
    }
}

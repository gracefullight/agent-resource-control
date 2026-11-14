//! Resolve the active sampler only inside the writer's transaction boundary.
use arc_meter::SnapshotReader;
use arc_policy::{ObservationInput, ObservationProvider};
use std::{
    sync::{Arc, Condvar, Mutex, MutexGuard},
    time::Instant,
};

#[derive(Default)]
pub(crate) struct Observations(
    pub(super) Mutex<Option<SnapshotReader>>,
    Arc<EmptyPublication>,
    Arc<EmptyPublication>,
    #[cfg(test)] Mutex<Option<Instant>>,
);
#[derive(Default)]
struct EmptyPublication {
    state: Mutex<(u64, Option<(u64, Instant)>)>,
    changed: Condvar,
}
struct EmptyLease(Arc<EmptyPublication>, u64);
impl Drop for EmptyLease {
    fn drop(&mut self) {
        let mut state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.1.is_some_and(|(id, _)| id == self.1) {
            state.1 = None;
        }
        self.0.changed.notify_all();
    }
}
impl Observations {
    pub(super) fn now(&self) -> Instant {
        #[cfg(test)]
        if let Some(now) = *self.3.lock().unwrap() {
            return now;
        }
        Instant::now()
    }
    #[cfg(test)]
    pub(super) fn freeze(&self, now: Instant) {
        *self.3.lock().unwrap() = Some(now);
        self.1.changed.notify_all();
        self.2.changed.notify_all();
    }

    pub(super) fn replace(&self, reader: SnapshotReader) {
        let mut active = self.reader();
        // An explicit Missing/L0 Ready must also precede any newly installed
        // Critical sampler. This wait owns no SQLite transaction or native call.
        let mut state = self.1.state.lock().unwrap_or_else(|e| e.into_inner());
        while let Some((_, end)) = state.1 {
            if self.now() >= end {
                state.1 = None;
                break;
            }
            state = self
                .1
                .changed
                .wait_timeout(state, end.saturating_duration_since(self.now()))
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
        drop(state);
        if let Some(previous) = active.as_ref() {
            if previous.same_sampler(&reader) {
                return;
            }
            // Replacement shares the old publication fence, including any
            // outstanding Ready lease. No old generation remains admissible.
            previous.retire();
        }
        *active = Some(reader);
    }

    /// Call with the registry locked and no active sampler. One bounded lease
    /// orders fallback launches against first binding, just like replacement.
    pub(super) fn fallback_fence(&self, end: Instant) -> Option<(Instant, Box<dyn Send>)> {
        let mut state = self.1.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.1.is_some_and(|(_, end)| self.now() < end) {
            return None;
        }
        state.0 = state.0.wrapping_add(1);
        let id = state.0;
        state.1 = Some((id, end));
        Some((end, Box::new(EmptyLease(Arc::clone(&self.1), id))))
    }

    /// Common nonblocking lease also exists when the sampler is absent/failed.
    /// Never wait here: its holder may need this writer for UNSPAWNED cleanup.
    pub(super) fn control_fence(&self, end: Instant) -> Option<(Instant, Box<dyn Send>)> {
        let mut state = self.2.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.1.is_some_and(|(_, end)| self.now() < end) {
            return None;
        }
        state.0 = state.0.wrapping_add(1);
        let id = state.0;
        state.1 = Some((id, end));
        Some((end, Box::new(EmptyLease(Arc::clone(&self.2), id))))
    }

    /// Call after acquiring IMMEDIATE. Keep the registry lock through the
    /// observation/launch callback so replacement linearizes with admission.
    pub(super) fn reader(&self) -> MutexGuard<'_, Option<SnapshotReader>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}
impl ObservationProvider for Observations {
    fn try_control(&self) -> Option<Box<dyn Send>> {
        self.control_fence(
            self.now() + std::time::Duration::from_millis(arc_policy::MAX_LAUNCH_FENCE_MS),
        )
        .map(|(_, lease)| lease)
    }

    fn with_observation(&self, consume: &mut dyn FnMut(ObservationInput<'_>)) {
        match self.reader().as_ref() {
            Some(reader) => reader.with_observation(consume),
            None => arc_policy::evaluator::MissingObservation.with_observation(consume),
        }
    }
}

#[cfg(test)]
mod clock_tests {
    use super::*;
    #[test]
    fn control_and_missing_sampler_leases_expire_only_when_their_clock_advances() {
        let source = Observations::default();
        let now = Instant::now();
        source.freeze(now);
        let end = now + std::time::Duration::from_millis(arc_policy::MAX_LAUNCH_FENCE_MS);
        let control = source.control_fence(end).unwrap();
        let fallback = source.fallback_fence(end).unwrap();
        assert!(source.control_fence(end).is_none());
        assert!(source.fallback_fence(end).is_none());
        source.freeze(end);
        let next_control = source
            .control_fence(end + std::time::Duration::from_secs(1))
            .unwrap();
        let next_fallback = source
            .fallback_fence(end + std::time::Duration::from_secs(1))
            .unwrap();
        drop((control, fallback));
        assert!(
            source.try_control().is_none(),
            "old Drop cannot release a renewed lease"
        );
        assert!(source
            .fallback_fence(end + std::time::Duration::from_secs(1))
            .is_none());
        drop((next_control, next_fallback));
        assert!(source.try_control().is_some());
    }
}

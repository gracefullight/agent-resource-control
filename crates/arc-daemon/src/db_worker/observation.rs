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
    pub(super) fn replace(&self, reader: SnapshotReader) {
        let mut active = self.reader();
        // An explicit Missing/L0 Ready must also precede any newly installed
        // Critical sampler. This wait owns no SQLite transaction or native call.
        let mut state = self.1.state.lock().unwrap_or_else(|e| e.into_inner());
        while let Some((_, end)) = state.1 {
            if Instant::now() >= end {
                state.1 = None;
                break;
            }
            state = self
                .1
                .changed
                .wait_timeout(state, end.saturating_duration_since(Instant::now()))
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
        if state.1.is_some_and(|(_, end)| Instant::now() < end) {
            return None;
        }
        state.0 = state.0.wrapping_add(1);
        let id = state.0;
        state.1 = Some((id, end));
        Some((end, Box::new(EmptyLease(Arc::clone(&self.1), id))))
    }

    /// Call after acquiring IMMEDIATE. Keep the registry lock through the
    /// observation/launch callback so replacement linearizes with admission.
    pub(super) fn reader(&self) -> MutexGuard<'_, Option<SnapshotReader>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}
impl ObservationProvider for Observations {
    fn with_observation(&self, consume: &mut dyn FnMut(ObservationInput<'_>)) {
        match self.reader().as_ref() {
            Some(reader) => reader.with_observation(consume),
            None => arc_policy::evaluator::MissingObservation.with_observation(consume),
        }
    }
}

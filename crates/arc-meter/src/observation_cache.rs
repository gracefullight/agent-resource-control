//! One coherent publication and one bounded launch lease, sharing the same fence.
use super::*;
use arc_policy::{
    ObservationCursor, ObservationInput, ObservationProvider, SamplerIdentity, SamplerStatus,
};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    MutexGuard,
};

pub type PublicationFence = (Instant, Box<dyn Send>);

static NEXT_GENERATION: AtomicU64 = AtomicU64::new(1);
pub(super) struct Cache {
    pub latest: Option<MeterSnapshot>,
    identity: SamplerIdentity,
    status: SamplerStatus,
    high_water: Option<ObservationCursor>,
    read_high_water_ms: Option<u64>,
    lease: Option<(u64, Instant)>,
    next_lease: u64,
    publishing: bool,
}
impl Cache {
    pub fn new(identity: ProcessIdentity) -> Self {
        let generation = NEXT_GENERATION.fetch_add(1, Ordering::Relaxed);
        Self {
            latest: None,
            identity: SamplerIdentity {
                daemon_instance: identity,
                generation,
                monotonic_epoch: generation,
            },
            status: SamplerStatus::Running,
            high_water: None,
            read_high_water_ms: None,
            lease: None,
            next_lease: 0,
            publishing: false,
        }
    }
    pub fn status(&mut self, status: SamplerStatus) {
        self.status = status;
    }
    pub fn publish(&mut self, snapshot: MeterSnapshot) {
        // Only the unique sampler publishes. Reject regression or mutation of
        // an equal sequence, including payload changes at the same timestamp.
        if self.latest.as_ref().is_some_and(|last| {
            snapshot.sequence < last.sequence
                || snapshot.sampled_monotonic_ms < last.sampled_monotonic_ms
                || (snapshot.sequence == last.sequence && snapshot != *last)
        }) {
            self.status = SamplerStatus::Failed;
            return;
        }
        self.latest = Some(snapshot);
    }
    fn reading(&mut self, now: Option<u64>) -> (Option<MeterSnapshot>, Option<u64>) {
        let now = now.filter(|now| {
            *now <= MAX_SAFE_INTEGER && self.read_high_water_ms.is_none_or(|prior| *now >= prior)
        });
        if let Some(now) = now {
            self.read_high_water_ms = Some(now);
        }
        let mut snapshot = self.latest.clone();
        if let (Some(sample), Some(now)) = (snapshot.as_mut(), now) {
            sample.age_ms = now
                .checked_sub(sample.sampled_monotonic_ms)
                .unwrap_or(MAX_SAFE_INTEGER);
        }
        (snapshot, now)
    }
    fn remember(&mut self, snapshot: Option<MeterSnapshot>) {
        if let Some(sample) = snapshot.filter(|s| s.is_valid()) {
            self.high_water = Some(ObservationCursor {
                sampler: self.identity.clone(),
                sequence: sample.sequence,
                sampled_monotonic_ms: sample.sampled_monotonic_ms,
            });
        }
    }
    fn input<'a>(
        &'a self,
        snapshot: Option<&'a MeterSnapshot>,
        now: Option<u64>,
    ) -> ObservationInput<'a> {
        ObservationInput {
            supported: cfg!(target_os = "macos"),
            sampler_status: self.status,
            active_sampler: Some(&self.identity),
            published_sampler: self.latest.as_ref().map(|_| &self.identity),
            snapshot,
            now_monotonic_ms: now,
            high_water: self.high_water.as_ref(),
        }
    }
}
impl Shared {
    pub(super) fn publication(&self) -> MutexGuard<'_, Cache> {
        let mut cache = self.latest.lock().unwrap_or_else(|e| e.into_inner());
        cache.publishing = true;
        while let Some((_, end)) = cache.lease {
            if Instant::now() >= end {
                cache.lease = None;
                break;
            }
            cache = self
                .published
                .wait_timeout(cache, end.saturating_duration_since(Instant::now()))
                .unwrap_or_else(|e| e.into_inner())
                .0;
        }
        cache.publishing = false;
        cache
    }
}
impl ObservationProvider for SnapshotReader {
    fn with_observation(&self, consume: &mut dyn FnMut(ObservationInput<'_>)) {
        self.with_observation_clock(
            || u64::try_from(self.0.epoch.elapsed().as_millis()).ok(),
            consume,
        );
    }
}
impl SnapshotReader {
    fn with_observation_clock(
        &self,
        clock: impl FnOnce() -> Option<u64>,
        consume: &mut dyn FnMut(ObservationInput<'_>),
    ) {
        let mut cache = self.0.latest.lock().unwrap_or_else(|e| e.into_inner());
        let (snapshot, now) = cache.reading(clock());
        consume(cache.input(snapshot.as_ref(), now));
        cache.remember(snapshot);
    }

    /// Permanently retire this generation after all already-issued bounded
    /// launch leases finish. Native sampling never runs under this fence.
    pub fn retire(&self) {
        self.0.publication().status(SamplerStatus::Stopped);
    }
    pub fn same_sampler(&self, other: &Self) -> bool {
        std::sync::Arc::ptr_eq(&self.0, &other.0)
    }
}
struct Lease {
    reader: SnapshotReader,
    id: u64,
}
impl Drop for Lease {
    fn drop(&mut self) {
        let mut cache = self
            .reader
            .0
            .latest
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if cache.lease.is_some_and(|(id, _)| id == self.id) {
            cache.lease = None;
        }
        self.reader.0.published.notify_all();
    }
}
impl SnapshotReader {
    /// The caller opens IMMEDIATE first. Evaluation, permit validation and
    /// COMMIT run under the cache lock; only after commit does a lease escape.
    /// No transaction survives the callback or the network acknowledgement.
    pub fn with_launch_fence<T>(
        &self,
        consume: impl FnOnce(ObservationInput<'_>, bool) -> (T, bool),
    ) -> (T, Option<PublicationFence>) {
        self.with_launch_deadline(|input, deadline| consume(input, deadline.is_some()))
    }
    /// The callback must check the supplied absolute deadline after its SQL
    /// work and before COMMIT. A committed Ready always returns its lease,
    /// even if COMMIT exhausted the deadline, so the caller can distinguish
    /// known-undelivered cleanup from an uncertain, possibly delivered release.
    pub fn with_launch_deadline<T>(
        &self,
        consume: impl FnOnce(ObservationInput<'_>, Option<Instant>) -> (T, bool),
    ) -> (T, Option<PublicationFence>) {
        self.with_launch_clock_deadline(
            || u64::try_from(self.0.epoch.elapsed().as_millis()).ok(),
            consume,
        )
    }
    #[cfg(test)]
    fn with_launch_clock<T>(
        &self,
        clock: impl FnOnce() -> Option<u64>,
        consume: impl FnOnce(ObservationInput<'_>, bool) -> (T, bool),
    ) -> (T, Option<PublicationFence>) {
        self.with_launch_clock_deadline(clock, |input, deadline| consume(input, deadline.is_some()))
    }
    fn with_launch_clock_deadline<T>(
        &self,
        clock: impl FnOnce() -> Option<u64>,
        consume: impl FnOnce(ObservationInput<'_>, Option<Instant>) -> (T, bool),
    ) -> (T, Option<PublicationFence>) {
        let mut cache = self.0.latest.lock().unwrap_or_else(|e| e.into_inner());
        let (snapshot, now) = cache.reading(clock());
        let input = cache.input(snapshot.as_ref(), now);
        let policy = arc_policy::evaluator::evaluate(input);
        let remaining =
            if policy.observation.availability == arc_policy::ObservationAvailability::Available {
                policy
                    .observation
                    .stale_after_ms
                    .unwrap_or(0)
                    .saturating_sub(policy.observation.age_ms.unwrap_or(u64::MAX))
            } else {
                arc_policy::MAX_LAUNCH_FENCE_MS
            };
        let bound = remaining.min(arc_policy::MAX_LAUNCH_FENCE_MS);
        let end = Instant::now() + Duration::from_millis(bound);
        let available = bound > 0
            && !cache.publishing
            && cache
                .lease
                .is_none_or(|(_, deadline)| Instant::now() >= deadline);
        let (value, ready) = consume(input, available.then_some(end));
        cache.remember(snapshot);
        if !ready || !available {
            return (value, None);
        }
        cache.next_lease = cache.next_lease.wrapping_add(1);
        let id = cache.next_lease;
        cache.lease = Some((id, end));
        (
            value,
            Some((
                end,
                Box::new(Lease {
                    reader: self.clone(),
                    id,
                }),
            )),
        )
    }
}

#[cfg(test)]
#[path = "observation_cache_tests.rs"]
mod tests;

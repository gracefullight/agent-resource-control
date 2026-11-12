use crate::{Sampler, SamplingPolicy};
use arc_core::{
    meter::{MemoryPressure, MeterSnapshot, SamplingCadence, MAX_SAFE_INTEGER},
    process_identity::ProcessIdentity,
};
use arc_platform::NativeSource;
use std::{
    io,
    sync::{Arc, Condvar, Mutex},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

#[derive(Default)]
struct Control {
    stopping: bool,
    active: bool,
}
struct Shared {
    control: Mutex<Control>,
    changed: Condvar,
    latest: Mutex<Option<MeterSnapshot>>,
    epoch: Instant,
}
#[derive(Clone)]
pub struct Activity(Arc<Shared>);
impl Activity {
    /// A latest boolean from the existing job owner, not a per-job queue.
    pub fn set_active(&self, active: bool) {
        let mut control = self.0.control.lock().unwrap_or_else(|e| e.into_inner());
        if control.active != active {
            control.active = active;
            self.0.changed.notify_one();
        }
    }
}
#[derive(Clone)]
pub struct SnapshotReader(Arc<Shared>);
impl SnapshotReader {
    /// No native work or renewed observation timestamp on the request path.
    pub fn snapshot(&self) -> Option<MeterSnapshot> {
        self.snapshot_at(u64::try_from(self.0.epoch.elapsed().as_millis()).ok()?)
    }
    pub fn snapshot_at(&self, monotonic_ms: u64) -> Option<MeterSnapshot> {
        let mut snapshot = self
            .0
            .latest
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()?;
        snapshot.age_ms = monotonic_ms.checked_sub(snapshot.sampled_monotonic_ms)?;
        (monotonic_ms <= MAX_SAFE_INTEGER).then_some(snapshot)
    }
}

/// One dedicated native worker, zero work queue, one latest snapshot. A slow
/// native call holds its unique source until completion; no timeout admits a
/// replacement call. Native calls run outside Tokio and outside the cache lock.
pub struct MeterWorker {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<Result<(), &'static str>>>,
}
impl MeterWorker {
    pub fn start<S: NativeSource>(
        source: S,
        identity: ProcessIdentity,
        epoch: Instant,
        policy: SamplingPolicy,
    ) -> io::Result<Self> {
        let sampler = Sampler::new(identity).map_err(io::Error::other)?;
        let shared = Arc::new(Shared {
            control: Mutex::new(Control::default()),
            changed: Condvar::new(),
            latest: Mutex::new(None),
            epoch,
        });
        let owned = Arc::clone(&shared);
        let thread = thread::Builder::new()
            .name("arc-meter".into())
            .spawn(move || run(source, sampler, owned, policy))?;
        Ok(Self {
            shared,
            thread: Some(thread),
        })
    }
    pub fn reader(&self) -> SnapshotReader {
        SnapshotReader(Arc::clone(&self.shared))
    }
    pub fn activity(&self) -> Activity {
        Activity(Arc::clone(&self.shared))
    }
    pub fn begin_shutdown(&self) {
        self.shared
            .control
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .stopping = true;
        self.shared.changed.notify_one();
    }
    /// Same absolute deadline as the daemon's other supervised resources. Keep
    /// ownership on timeout; Drop still stops and joins this finite worker.
    pub async fn shutdown(&mut self, deadline: tokio::time::Instant) -> io::Result<()> {
        self.begin_shutdown();
        while let Some(thread) = &self.thread {
            if thread.is_finished() {
                return self.join();
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "meter shutdown deadline",
                ));
            }
            tokio::time::sleep_until(
                deadline.min(tokio::time::Instant::now() + Duration::from_millis(1)),
            )
            .await;
        }
        Ok(())
    }
    fn join(&mut self) -> io::Result<()> {
        if let Some(thread) = self.thread.take() {
            thread
                .join()
                .map_err(|_| io::Error::other("meter worker panicked"))?
                .map_err(io::Error::other)?;
        }
        Ok(())
    }
}
impl Drop for MeterWorker {
    fn drop(&mut self) {
        self.begin_shutdown();
        let _ = self.join();
    }
}

/// Scheduling is based on completion, so delayed work never creates catch-up
/// calls. Activity can shorten the wait, but slowing down takes effect only once
/// a sample publishes the slower cadence. Never extend the published interval.
fn next_due(
    completed: Instant,
    published: SamplingCadence,
    active: bool,
    pressure: bool,
    policy: SamplingPolicy,
) -> Instant {
    completed
        + Duration::from_millis(
            published
                .interval_ms
                .min(policy.cadence(active, pressure).interval_ms),
        )
}
fn run<S: NativeSource>(
    mut source: S,
    mut sampler: Sampler,
    shared: Arc<Shared>,
    policy: SamplingPolicy,
) -> Result<(), &'static str> {
    loop {
        let control = shared.control.lock().unwrap_or_else(|e| e.into_inner());
        if control.stopping {
            return Ok(());
        }
        let active = control.active;
        drop(control);
        let sample = source.sample();
        let snapshot = sampler
            .observe(sample, active, policy)
            .ok_or("invalid meter clock or sequence")?;
        if !snapshot.is_valid() {
            return Err("invalid native meter snapshot");
        }
        let pressure = matches!(
            snapshot.host.memory_pressure.reading.value(),
            Some(MemoryPressure::Warning | MemoryPressure::Critical)
        );
        let cadence = snapshot.cadence;
        *shared.latest.lock().unwrap_or_else(|e| e.into_inner()) = Some(snapshot);
        let completed = Instant::now();
        let mut control = shared.control.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            if control.stopping {
                return Ok(());
            }
            let due = next_due(completed, cadence, control.active, pressure, policy);
            let now = Instant::now();
            if now >= due {
                break;
            }
            let (new_control, _) = shared
                .changed
                .wait_timeout(control, due - now)
                .unwrap_or_else(|e| e.into_inner());
            control = new_control;
        }
    }
}

#[cfg(test)]
#[path = "worker_tests.rs"]
mod tests;

use super::*;
use crate::fixtures::{identity, sample};
use std::sync::mpsc;

struct GatedSource {
    entered: mpsc::SyncSender<()>,
    release: mpsc::Receiver<()>,
    epoch: Instant,
}
impl NativeSource for GatedSource {
    fn sample(&mut self) -> arc_platform::NativeSample {
        self.entered.send(()).unwrap();
        self.release.recv().unwrap();
        sample(self.epoch.elapsed().as_millis() as u64)
    }
}

#[test]
fn cadence_changes_reschedule_from_completion_without_catchup_or_unbounded_work() {
    let policy = SamplingPolicy::default();
    let completed = Instant::now();
    let idle = policy.cadence(false, false);
    let active = policy.cadence(true, false);
    let pressure = policy.cadence(true, true);
    assert_eq!(
        next_due(completed, idle, false, false, policy) - completed,
        Duration::from_secs(15)
    );
    assert_eq!(
        next_due(completed, active, true, false, policy) - completed,
        Duration::from_secs(2)
    );
    assert_eq!(
        next_due(completed, pressure, true, true, policy) - completed,
        Duration::from_secs(1)
    );
    assert_eq!(
        next_due(completed, idle, true, false, policy) - completed,
        Duration::from_secs(2)
    );
    assert_eq!(
        next_due(completed, idle, false, true, policy) - completed,
        Duration::from_secs(1)
    );
    // If native work took 60 seconds, the next tick is still after completion.
    assert!(
        next_due(
            completed + Duration::from_secs(60),
            idle,
            false,
            false,
            policy
        ) > completed + Duration::from_secs(60)
    );
    assert_eq!(crate::PERSISTED_SAMPLE_CAPACITY, 0);
}

#[test]
fn becoming_idle_during_native_work_keeps_the_published_active_deadline() {
    let policy = SamplingPolicy::default();
    let completed = Instant::now();
    let mut sampler = Sampler::new(identity("1")).unwrap();
    // Activity was captured before native work; the job ends before publication.
    let snapshot = sampler.observe(sample(0), true, policy).unwrap();
    let due = next_due(completed, snapshot.cadence, false, false, policy);
    assert_eq!(
        due - completed,
        Duration::from_millis(snapshot.cadence.interval_ms)
    );
    assert_eq!(snapshot.cadence.stale_after_ms, 5000);
    assert_eq!(snapshot.sampled_monotonic_ms, 0);
    // The next pass publishes the idle cadence and may then use the longer wait.
    let next = sampler.observe(sample(2000), false, policy).unwrap();
    assert_eq!(next.cadence.interval_ms, 15_000);
    assert_eq!(
        next_due(due, next.cadence, false, false, policy) - due,
        Duration::from_secs(15)
    );
}

#[test]
fn becoming_idle_while_waiting_does_not_extend_the_published_active_deadline() {
    let policy = SamplingPolicy::default();
    let completed = Instant::now();
    let mut sampler = Sampler::new(identity("1")).unwrap();
    let snapshot = sampler.observe(sample(1000), true, policy).unwrap();
    let published_due = completed + Duration::from_millis(snapshot.cadence.interval_ms);
    for (active, elapsed_ms) in [(true, 0), (false, 500), (true, 750), (false, 1000)] {
        let due = next_due(completed, snapshot.cadence, active, false, policy);
        assert_eq!(due, published_due);
        assert!(due > completed + Duration::from_millis(elapsed_ms));
        assert_eq!(snapshot.sampled_monotonic_ms, 1000);
        assert_eq!(snapshot.cadence.interval_ms, 2000);
    }
}

#[tokio::test]
async fn native_work_is_serial_reads_never_admit_work_and_shutdown_joins() {
    let epoch = Instant::now();
    let (entered, calls) = mpsc::sync_channel(1);
    let (release, allowed) = mpsc::sync_channel(1);
    let mut worker = MeterWorker::start(
        GatedSource {
            entered,
            release: allowed,
            epoch,
        },
        identity("1"),
        epoch,
        SamplingPolicy::default(),
    )
    .unwrap();
    calls.recv_timeout(Duration::from_secs(2)).unwrap();
    let reader = worker.reader();
    for _ in 0..100 {
        assert!(reader.snapshot().is_none());
        worker.activity().set_active(true);
    }
    assert!(calls.try_recv().is_err());
    release.send(()).unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    while reader.snapshot().is_none() && Instant::now() < deadline {
        thread::yield_now();
    }
    let first = reader.snapshot().unwrap();
    for age in [0, 1, 31_001] {
        let cached = reader
            .snapshot_at(first.sampled_monotonic_ms + age)
            .unwrap();
        assert_eq!(cached.sequence, 1);
        assert_eq!(cached.age_ms, age);
        assert_eq!(cached.sampled_at_ms, first.sampled_at_ms);
    }
    if first.sampled_monotonic_ms > 0 {
        assert!(reader.snapshot_at(first.sampled_monotonic_ms - 1).is_none());
    }
    assert!(reader.snapshot_at(MAX_SAFE_INTEGER + 1).is_none());
    worker.activity().set_active(false);
    worker.begin_shutdown();
    worker
        .shutdown(tokio::time::Instant::now() + Duration::from_secs(2))
        .await
        .unwrap();
    worker.shutdown(tokio::time::Instant::now()).await.unwrap();
    assert!(worker.thread.is_none());
    assert!(calls.try_recv().is_err());
}

#[tokio::test]
async fn timeout_retains_worker_ownership_and_never_starts_a_replacement() {
    let epoch = Instant::now();
    let (entered, calls) = mpsc::sync_channel(1);
    let (release, allowed) = mpsc::sync_channel(1);
    let mut worker = MeterWorker::start(
        GatedSource {
            entered,
            release: allowed,
            epoch,
        },
        identity("1"),
        epoch,
        SamplingPolicy::default(),
    )
    .unwrap();
    calls.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(
        worker
            .shutdown(tokio::time::Instant::now())
            .await
            .unwrap_err()
            .kind(),
        io::ErrorKind::TimedOut
    );
    assert!(worker.thread.is_some());
    assert!(calls.try_recv().is_err());
    release.send(()).unwrap();
    worker
        .shutdown(tokio::time::Instant::now() + Duration::from_secs(2))
        .await
        .unwrap();
    assert!(worker.thread.is_none());
}

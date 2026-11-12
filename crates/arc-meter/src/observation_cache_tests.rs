use super::*;
use crate::fixtures::{identity, sample};
use arc_core::meter::MemoryPressure;
use arc_policy::{ObservationProvider, PolicyMode, PolicyReason, SamplerStatus};
use std::sync::mpsc;

fn reader(pressure: MemoryPressure) -> SnapshotReader {
    let epoch = Instant::now();
    let identity = identity("1");
    let mut native = sample(0);
    native.memory_pressure = arc_core::meter::Reading::Available { value: pressure };
    let snapshot = Sampler::new(identity.clone())
        .unwrap()
        .observe(native, true, SamplingPolicy::default())
        .unwrap();
    let mut cache = Cache::new(identity);
    cache.publish(snapshot);
    SnapshotReader(Arc::new(Shared {
        control: Mutex::new(Control::default()),
        changed: Condvar::new(),
        latest: Mutex::new(cache),
        published: Condvar::new(),
        epoch,
    }))
}
fn policy(reader: &SnapshotReader) -> arc_policy::EffectivePolicy {
    let mut result = None;
    reader.with_observation(&mut |input| result = Some(arc_policy::evaluator::evaluate(input)));
    result.unwrap()
}
#[test]
fn duplicate_publication_is_immutable_and_a_regression_fails_health() {
    let reader = reader(MemoryPressure::Normal);
    let original = reader.snapshot().unwrap();
    let mut snapshot = reader.0.latest.lock().unwrap().latest.clone().unwrap();
    reader.0.publication().publish(snapshot.clone());
    assert_eq!(reader.snapshot().unwrap().sequence, original.sequence);
    snapshot.host.memory_pressure.reading = arc_core::meter::Reading::Available {
        value: MemoryPressure::Critical,
    };
    reader.0.publication().publish(snapshot);
    let policy = policy(&reader);
    assert_eq!(policy.mode, PolicyMode::L0);
    if cfg!(target_os = "macos") {
        assert_eq!(policy.reason, PolicyReason::ObservationFailed);
    }
}
#[test]
fn replacement_generation_has_no_old_snapshot_or_cursor_and_stop_does_not_hold() {
    let first = reader(MemoryPressure::Critical);
    let first_identity = first.0.latest.lock().unwrap().identity.clone();
    first.with_observation(&mut |_| {});
    assert!(first.0.latest.lock().unwrap().high_water.is_some());
    let second = Cache::new(first_identity.daemon_instance.clone());
    assert_ne!(second.identity.generation, first_identity.generation);
    assert_ne!(
        second.identity.monotonic_epoch,
        first_identity.monotonic_epoch
    );
    assert!(second.latest.is_none());
    assert!(second.high_water.is_none());
    first.0.publication().status(SamplerStatus::Stopped);
    assert_eq!(policy(&first).mode, PolicyMode::L0);
    if cfg!(target_os = "macos") {
        assert_eq!(policy(&first).reason, PolicyReason::ObservationFailed);
    }
}
#[test]
fn publication_waits_through_the_observation_callback() {
    let reader = reader(MemoryPressure::Normal);
    let (entered, inside) = mpsc::sync_channel(1);
    let (release, allowed) = mpsc::sync_channel(1);
    let borrower = reader.clone();
    let reading = std::thread::spawn(move || {
        borrower.with_observation(&mut |_| {
            entered.send(()).unwrap();
            allowed.recv().unwrap();
        })
    });
    inside.recv_timeout(Duration::from_secs(2)).unwrap();
    let publisher = reader.clone();
    let (published, done) = mpsc::sync_channel(1);
    let publishing = std::thread::spawn(move || {
        let mut value = publisher.0.publication();
        let mut snapshot = value.latest.clone().unwrap();
        snapshot.sequence += 1;
        snapshot.host.memory_pressure.reading = arc_core::meter::Reading::Available {
            value: MemoryPressure::Critical,
        };
        value.publish(snapshot);
        published.send(()).unwrap();
    });
    assert!(done.recv_timeout(Duration::from_millis(10)).is_err());
    release.send(()).unwrap();
    reading.join().unwrap();
    done.recv_timeout(Duration::from_secs(2)).unwrap();
    publishing.join().unwrap();
    if cfg!(target_os = "macos") {
        assert_eq!(policy(&reader).mode, PolicyMode::MemoryHold);
    }
}
#[test]
fn launch_lease_is_bounded_nonrenewable_and_does_not_own_a_database() {
    let reader = reader(MemoryPressure::Normal);
    let ((), lease) = reader.with_launch_fence(|_, available| {
        assert!(available);
        ((), true)
    });
    let (end, lease) = lease.unwrap();
    assert!(end <= Instant::now() + Duration::from_millis(arc_policy::MAX_LAUNCH_FENCE_MS));
    let (_, second) = reader.with_launch_fence(|_, available| {
        assert!(!available);
        ((), false)
    });
    assert!(second.is_none());
    let publisher = reader.clone();
    let (published, done) = mpsc::sync_channel(1);
    let publishing = std::thread::spawn(move || {
        let mut cache = publisher.0.publication();
        let mut snapshot = cache.latest.clone().unwrap();
        snapshot.sequence += 1;
        cache.publish(snapshot);
        published.send(()).unwrap();
    });
    assert!(done.recv_timeout(Duration::from_millis(10)).is_err());
    // The same bounded cache remains readable while the native sampler's
    // already-completed publication waits. No native work is queued by reads.
    assert!(reader.snapshot().is_some());
    drop(lease);
    done.recv_timeout(Duration::from_secs(2)).unwrap();
    publishing.join().unwrap();
    assert_eq!(reader.snapshot().unwrap().sequence, 2);
}
#[test]
fn expired_lease_cannot_be_renewed_or_release_a_later_lease() {
    let reader = reader(MemoryPressure::Normal);
    let (_, lease) = reader.with_launch_fence(|_, _| ((), true));
    let (_, old) = lease.unwrap();
    reader.0.latest.lock().unwrap().lease.as_mut().unwrap().1 = Instant::now();
    let (_, lease) = reader.with_launch_fence(|_, available| {
        assert!(available);
        ((), true)
    });
    let (_, new) = lease.unwrap();
    drop(old);
    assert!(reader.0.latest.lock().unwrap().lease.is_some());
    drop(new);
    assert!(reader.0.latest.lock().unwrap().lease.is_none());
}
#[test]
fn exhausted_freshness_cannot_release_execution_but_stale_fallback_has_a_bound() {
    let mut reader = reader(MemoryPressure::Normal);
    let shared = Arc::get_mut(&mut reader.0).unwrap();
    shared.epoch = Instant::now() - Duration::from_millis(5001);
    let (_, lease) = reader.with_launch_fence(|input, available| {
        let effective = arc_policy::evaluator::evaluate(input);
        assert_eq!(effective.mode, PolicyMode::L0);
        assert!(available);
        ((), true)
    });
    assert!(
        lease.is_some(),
        "explicit L0 fallback cannot retain an indefinite stale hold"
    );
}

#[test]
#[cfg(target_os = "macos")]
fn final_launch_clock_caps_the_fence_at_exact_freshness_and_then_uses_explicit_l0() {
    let reader = reader(MemoryPressure::Normal);
    // Active cadence is 2000ms with a 5000ms freshness threshold.
    let (_, lease) = reader.with_launch_clock(
        || Some(5000),
        |input, available| {
            let effective = arc_policy::evaluator::evaluate(input);
            assert_eq!(
                effective.observation.availability,
                arc_policy::ObservationAvailability::Available
            );
            assert!(
                !available,
                "no time remains to execute under this fresh reading"
            );
            ((), false)
        },
    );
    assert!(lease.is_none());
    let (_, lease) = reader.with_launch_clock(
        || Some(5001),
        |input, available| {
            let effective = arc_policy::evaluator::evaluate(input);
            assert_eq!(effective.reason, PolicyReason::ObservationStale);
            assert!(available, "L0 uses a separate bounded launch lease");
            ((), true)
        },
    );
    assert!(lease.is_some());
}

#[test]
fn waiting_publication_cannot_be_starved_by_another_jobs_launch_lease() {
    let reader = reader(MemoryPressure::Normal);
    let (_, lease) = reader.with_launch_fence(|_, available| {
        assert!(available);
        ((), true)
    });
    let (_, lease) = lease.unwrap();
    let publisher = reader.clone();
    let publishing = std::thread::spawn(move || {
        let mut cache = publisher.0.publication();
        let mut snapshot = cache.latest.clone().unwrap();
        snapshot.sequence += 1;
        snapshot.host.memory_pressure.reading = arc_core::meter::Reading::Available {
            value: MemoryPressure::Critical,
        };
        cache.publish(snapshot);
    });
    let end = Instant::now() + Duration::from_secs(1);
    while !reader.0.latest.lock().unwrap().publishing {
        assert!(Instant::now() < end);
        std::thread::yield_now();
    }
    // Even an expired old lease cannot be replaced ahead of a waiting publisher.
    reader.0.latest.lock().unwrap().lease.as_mut().unwrap().1 = Instant::now();
    let (_, replacement) = reader.with_launch_fence(|_, available| {
        assert!(!available);
        ((), false)
    });
    assert!(replacement.is_none());
    drop(lease);
    publishing.join().unwrap();
    if cfg!(target_os = "macos") {
        assert_eq!(policy(&reader).mode, PolicyMode::MemoryHold);
    }
}

#[test]
#[cfg(target_os = "macos")]
fn stale_critical_cannot_become_fresh_when_admission_or_launch_clock_regresses() {
    for launch_first in [false, true] {
        let reader = reader(MemoryPressure::Critical);
        let mut observed = Vec::new();
        reader.with_observation_clock(|| Some(1000), &mut |input| {
            observed.push(arc_policy::evaluator::evaluate(input));
        });
        if launch_first {
            reader.with_launch_clock(
                || Some(5001),
                |input, _| {
                    observed.push(arc_policy::evaluator::evaluate(input));
                    ((), false)
                },
            );
        } else {
            reader.with_observation_clock(|| Some(5001), &mut |input| {
                observed.push(arc_policy::evaluator::evaluate(input));
            });
        }
        assert_eq!(observed[0].mode, PolicyMode::MemoryHold);
        assert_eq!(observed[1].reason, PolicyReason::ObservationStale);
        for time in [2000, 5000] {
            reader.with_observation_clock(|| Some(time), &mut |input| {
                let policy = arc_policy::evaluator::evaluate(input);
                assert_eq!(policy.mode, PolicyMode::L0);
                assert_eq!(policy.reason, PolicyReason::ClockInvalid);
                assert_eq!(policy.observation.pressure, None);
            });
            reader.with_launch_clock(
                || Some(time),
                |input, available| {
                    let policy = arc_policy::evaluator::evaluate(input);
                    assert_eq!(policy.mode, PolicyMode::L0);
                    assert_eq!(policy.reason, PolicyReason::ClockInvalid);
                    assert!(available, "invalid clock uses explicit bounded L0");
                    ((), false)
                },
            );
        }
        assert_eq!(
            reader.0.latest.lock().unwrap().read_high_water_ms,
            Some(5001)
        );
    }
}

#[test]
fn retirement_waits_for_ready_and_old_generation_never_returns_a_healthy_reading() {
    let reader = reader(MemoryPressure::Normal);
    let (_, lease) = reader.with_launch_fence(|_, available| ((), available));
    let (_, lease) = lease.unwrap();
    let old = reader.clone();
    let (retired, done) = mpsc::sync_channel(1);
    let replacing = std::thread::spawn(move || {
        old.retire();
        retired.send(()).unwrap();
    });
    let end = Instant::now() + Duration::from_secs(1);
    while !reader.0.latest.lock().unwrap().publishing {
        assert!(Instant::now() < end);
        std::thread::yield_now();
    }
    assert!(matches!(done.try_recv(), Err(mpsc::TryRecvError::Empty)));
    drop(lease);
    done.recv_timeout(Duration::from_secs(1)).unwrap();
    replacing.join().unwrap();
    assert_eq!(policy(&reader).mode, PolicyMode::L0);
    let mut cache = reader.0.publication();
    let mut snapshot = cache.latest.clone().unwrap();
    snapshot.sequence += 1;
    cache.publish(snapshot);
    drop(cache);
    if cfg!(target_os = "macos") {
        assert_eq!(policy(&reader).reason, PolicyReason::ObservationFailed);
    }
}

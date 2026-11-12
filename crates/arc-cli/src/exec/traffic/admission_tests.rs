use super::*;
use std::cell::Cell;

#[test]
fn responsive_busy_admission_retries_without_becoming_outage_or_spawning() {
    let calls = Cell::new(0);
    let result = admission_with(
        Instant::now() + Duration::from_millis(10),
        &|| false,
        |deadline| {
            assert!(deadline > Instant::now());
            calls.set(calls.get() + 1);
            if calls.get() < 3 {
                Err(Failure::Busy)
            } else {
                Ok(JobResult::Queued {
                    sequence: 1,
                    queued_at_ms: 1,
                    reason: arc_ipc::traffic::WaitReason::MemoryBudget,
                })
            }
        },
    );
    assert!(matches!(result, Ok(JobResult::Queued { .. })));
    assert_eq!(calls.get(), 3);
}

#[test]
fn busy_admission_cancellation_remains_active_and_actual_outage_is_not_retried() {
    let cancelled = Cell::new(false);
    let result = admission_with(Instant::now(), &|| cancelled.get(), |_| {
        cancelled.set(true);
        Err(Failure::Busy)
    });
    assert!(matches!(result, Err(Failure::Cancelled)));
    let calls = Cell::new(0);
    let result = admission_with(Instant::now(), &|| false, |_| {
        calls.set(calls.get() + 1);
        Err(Failure::Unavailable)
    });
    assert!(matches!(result, Err(Failure::Unavailable)));
    assert_eq!(calls.get(), 1);
}

use super::*;
use crate::exec::preparation::NoticeWrite;
use std::cell::Cell;

#[test]
fn queued_notice_retries_only_after_confirmed_non_emission_or_pressure_change() {
    let calls = Cell::new(0);
    let mut state = NoticeState::default();
    let emit = |_: bool| {
        calls.set(calls.get() + 1);
        if calls.get() == 1 {
            NoticeWrite::NotWritten
        } else {
            NoticeWrite::Complete
        }
    };
    announce_if_needed(&mut state, false, emit);
    assert_eq!(state.last, None);
    announce_if_needed(&mut state, false, emit);
    assert_eq!(state.last, Some(false));
    announce_if_needed(&mut state, false, emit);
    assert_eq!(calls.get(), 2);
    announce_if_needed(&mut state, true, emit);
    assert_eq!(state.last, Some(true));
    assert_eq!(calls.get(), 3);
}

#[test]
fn uncertain_or_partial_notice_never_retries_even_if_pressure_changes() {
    let mut state = NoticeState::default();
    let calls = Cell::new(0);
    announce_if_needed(&mut state, false, |_| {
        calls.set(calls.get() + 1);
        NoticeWrite::Uncertain
    });
    for pressure in [false, true, false] {
        announce_if_needed(&mut state, pressure, |_| {
            calls.set(calls.get() + 1);
            NoticeWrite::Complete
        });
    }
    assert_eq!(calls.get(), 1);
    assert!(state.uncertain);
}

#[test]
fn confirmed_failures_are_bounded_per_pressure_and_retry_after_a_change() {
    let mut state = NoticeState::default();
    let calls = Cell::new(0);
    for pressure in [false, false, false] {
        announce_if_needed(&mut state, pressure, |_| {
            calls.set(calls.get() + 1);
            NoticeWrite::NotWritten
        });
    }
    assert_eq!(calls.get(), 2);
    assert_eq!(state.last, None);
    announce_if_needed(&mut state, true, |pressure| {
        assert!(pressure);
        calls.set(calls.get() + 1);
        NoticeWrite::Complete
    });
    assert_eq!(calls.get(), 3);
    assert_eq!(state.last, Some(true));
}

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

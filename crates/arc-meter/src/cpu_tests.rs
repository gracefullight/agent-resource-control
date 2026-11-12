use super::*;

fn ticks(user: u64, system: u64, nice: u64, idle: u64) -> Reading<CpuCounters> {
    Reading::Available {
        value: CpuCounters {
            user,
            system,
            nice,
            idle,
            counter_bits: 32,
            logical_cpu_count: 4,
        },
    }
}
fn update(
    previous: &mut Option<PreviousCpu>,
    ticks: Reading<CpuCounters>,
    wall: Option<u64>,
    mono: u64,
) -> (Reading<f64>, Option<CpuInterval>) {
    utilization(previous, &ticks, wall, mono)
}

#[test]
fn counter_deltas_normalize_whole_host_once_and_include_nice() {
    let mut previous = None;
    assert_eq!(
        update(&mut previous, ticks(0, 0, 0, 0), Some(10), 0).0,
        Reading::FirstInterval {}
    );
    let (reading, interval) = update(&mut previous, ticks(100, 50, 50, 200), Some(1010), 1000);
    assert_eq!(reading.value(), Some(&50.0));
    let interval = interval.unwrap();
    assert_eq!(interval.logical_cpu_count, 4);
    assert_eq!(interval.normalization, CpuNormalization::HostCapacity);
    assert_eq!(interval.elapsed_ms, 1000);
    assert_eq!(interval.started_at_ms, Some(10));
    // A whole-host idle interval is a real measured zero, not unknown.
    assert_eq!(
        update(&mut previous, ticks(100, 50, 50, 600), None, 2000)
            .0
            .value(),
        Some(&0.0)
    );
    assert_eq!(
        update(&mut previous, ticks(300, 150, 150, 600), None, 3000)
            .0
            .value(),
        Some(&100.0)
    );
}

#[test]
fn declared_wrap_is_accepted_only_near_boundary_with_plausible_elapsed_time() {
    for bits in [8, 32, 64] {
        let maximum = if bits == 64 {
            u64::MAX
        } else {
            (1u64 << bits) - 1
        };
        let before = Reading::Available {
            value: CpuCounters {
                user: maximum - 5,
                system: 0,
                nice: 0,
                idle: 0,
                counter_bits: bits,
                logical_cpu_count: 1,
            },
        };
        let after = Reading::Available {
            value: CpuCounters {
                user: 4,
                system: 0,
                nice: 0,
                idle: 1,
                counter_bits: bits,
                logical_cpu_count: 1,
            },
        };
        let mut previous = None;
        update(&mut previous, before.clone(), None, 0);
        let result = update(&mut previous, after.clone(), None, 10);
        assert!((result.0.value().unwrap() - 1000.0 / 11.0).abs() < 1e-10);
        if bits == 8 {
            update(&mut previous, before, None, 20);
            assert!(matches!(
                update(&mut previous, after, None, 1020).0,
                Reading::Failed { .. }
            ));
        }
    }
    let mut previous = None;
    update(&mut previous, ticks(1000, 1000, 1000, 1000), None, 1000);
    assert!(matches!(
        update(&mut previous, ticks(1, 1, 1, 1), None, 2000).0,
        Reading::Failed { .. }
    ));
    assert_eq!(
        update(&mut previous, ticks(2, 2, 2, 2), None, 3000)
            .0
            .value(),
        Some(&75.0)
    );
}

#[test]
fn long_gaps_with_increasing_endpoints_cannot_hide_complete_wraps() {
    let elapsed = 4_320_000_000;
    // Fifty days on twelve 100 Hz cores: 90% busy crosses the 32-bit boundary
    // yet leaves an endpoint greater than its baseline. Endpoint subtraction
    // alone would incorrectly report about 41.69% utilization.
    let total = elapsed / 10 * 12;
    let busy = total * 9 / 10;
    let idle = total - busy;
    let mut before = ticks(100, 0, 0, 100).value().unwrap().clone();
    before.logical_cpu_count = 12;
    let mut after = before.clone();
    after.user = (before.user + busy) % (1u64 << 32);
    after.idle += idle;
    assert!(after.user > before.user && after.idle > before.idle);
    assert_eq!((after.user, after.idle), (370_632_804, 518_400_100));

    let mut previous = None;
    update(&mut previous, Reading::Available { value: before }, None, 0);
    let (reading, interval) = update(
        &mut previous,
        Reading::Available {
            value: after.clone(),
        },
        None,
        elapsed,
    );
    assert!(matches!(reading, Reading::Failed { .. }));
    assert!(interval.is_none());
    // The rejected interval still seeds the next, short comparable interval.
    after.user += 600;
    after.idle += 600;
    assert_eq!(
        update(
            &mut previous,
            Reading::Available { value: after },
            None,
            elapsed + 1000,
        )
        .0
        .value(),
        Some(&50.0)
    );
}

#[test]
fn ambiguity_bound_applies_to_increasing_endpoints_at_every_counter_width() {
    for bits in [8, 32, 64] {
        let half_range = 1u64 << (bits - 1);
        for (elapsed, ambiguous) in [
            (half_range - 2, false),
            (half_range - 1, true),
            (half_range, true),
        ] {
            let mut before = ticks(1, 0, 0, 1).value().unwrap().clone();
            before.counter_bits = bits;
            before.logical_cpu_count = 1;
            let mut after = before.clone();
            after.user += 1;
            after.idle += 1;
            let mut previous = None;
            update(&mut previous, Reading::Available { value: before }, None, 0);
            let (reading, interval) = update(
                &mut previous,
                Reading::Available { value: after },
                None,
                elapsed,
            );
            if ambiguous {
                assert!(
                    matches!(reading, Reading::Failed { .. }),
                    "{bits}/{elapsed}"
                );
                assert!(interval.is_none());
            } else {
                assert_eq!(reading.value(), Some(&50.0));
                assert_eq!(interval.unwrap().elapsed_ms, elapsed);
            }
        }
    }
}

#[test]
fn invalid_counters_zero_delta_discontinuities_and_topology_are_unknown() {
    let mut previous = None;
    update(&mut previous, ticks(0, 0, 0, 0), None, 0);
    assert!(matches!(
        update(&mut previous, ticks(0, 0, 0, 0), None, 1000).0,
        Reading::Failed { .. }
    ));
    assert!(matches!(
        update(&mut previous, ticks(1_000_000, 0, 0, 0), None, 1001).0,
        Reading::Failed { .. }
    ));
    for (bits, cpus, user) in [(0, 1, 0), (65, 1, 0), (32, 0, 0), (8, 1, 256)] {
        let invalid = Reading::Available {
            value: CpuCounters {
                user,
                system: 0,
                nice: 0,
                idle: 0,
                counter_bits: bits,
                logical_cpu_count: cpus,
            },
        };
        assert!(matches!(
            update(&mut previous, invalid, None, 2000).0,
            Reading::Failed { .. }
        ));
        assert!(previous.is_none());
    }
    update(&mut previous, ticks(1, 1, 1, 1), None, 2000);
    let mut topology = ticks(2, 2, 2, 2).value().unwrap().clone();
    topology.logical_cpu_count = 8;
    assert_eq!(
        update(
            &mut previous,
            Reading::Available {
                value: topology.clone()
            },
            None,
            3000
        )
        .0,
        Reading::FirstInterval {}
    );
    topology.counter_bits = 64;
    assert_eq!(
        update(
            &mut previous,
            Reading::Available { value: topology },
            None,
            4000
        )
        .0,
        Reading::FirstInterval {}
    );
}

#[test]
fn monotonic_clock_and_wall_clock_changes_have_distinct_effects() {
    let mut previous = None;
    update(&mut previous, ticks(0, 0, 0, 0), Some(10_000), 1000);
    let (result, interval) = update(&mut previous, ticks(100, 0, 0, 100), Some(1), 2000);
    assert_eq!(result.value(), Some(&50.0));
    assert_eq!(interval.unwrap().elapsed_ms, 1000);
    assert!(matches!(
        update(&mut previous, ticks(101, 0, 0, 101), None, 2000).0,
        Reading::Failed { .. }
    ));
    assert!(matches!(
        update(&mut previous, ticks(102, 0, 0, 102), None, 1999).0,
        Reading::Failed { .. }
    ));
}

#[test]
fn partial_native_cpu_failure_breaks_only_the_cpu_baseline() {
    let mut previous = None;
    update(&mut previous, ticks(0, 0, 0, 0), None, 0);
    for reading in [
        Reading::Denied {},
        Reading::Unsupported {},
        failed("native_cpu_failed"),
    ] {
        let (result, interval) = update(&mut previous, reading.clone(), None, 1000);
        assert_eq!(result, unknown(&reading));
        assert!(interval.is_none());
        assert!(previous.is_none());
        assert_eq!(
            update(&mut previous, ticks(10, 0, 0, 10), None, 2000).0,
            Reading::FirstInterval {}
        );
    }
}

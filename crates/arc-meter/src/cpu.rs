use arc_core::meter::{CpuInterval, CpuNormalization, Diagnostic, Reading};
use arc_platform::CpuCounters;

#[derive(Clone)]
pub(super) struct PreviousCpu {
    pub counters: CpuCounters,
    pub wall: Option<u64>,
    pub monotonic: u64,
}

pub(super) fn failed<T>(label: &'static str) -> Reading<T> {
    Reading::Failed {
        diagnostic: Diagnostic::bounded(label),
    }
}

/// Propagate availability without inventing a value of a different type.
pub(super) fn unknown<T, U>(reading: &Reading<T>) -> Reading<U> {
    match reading {
        Reading::Unsupported {} => Reading::Unsupported {},
        Reading::Denied {} => Reading::Denied {},
        Reading::Failed { diagnostic } => Reading::Failed {
            diagnostic: diagnostic.clone(),
        },
        _ => failed("measurement_input_unavailable"),
    }
}

fn values(counters: &CpuCounters) -> [u64; 4] {
    [counters.user, counters.system, counters.nice, counters.idle]
}
fn valid(counters: &CpuCounters) -> bool {
    (1..=64).contains(&counters.counter_bits)
        && counters.logical_cpu_count > 0
        && values(counters)
            .iter()
            .all(|n| u128::from(*n) < (1u128 << counters.counter_bits))
}

pub(super) fn utilization(
    previous: &mut Option<PreviousCpu>,
    reading: &Reading<CpuCounters>,
    wall: Option<u64>,
    monotonic: u64,
) -> (Reading<f64>, Option<CpuInterval>) {
    let Some(current) = reading.value() else {
        *previous = None;
        return (unknown(reading), None);
    };
    if !valid(current) {
        *previous = None;
        return (failed("invalid_cpu_counters"), None);
    }
    let old = previous.replace(PreviousCpu {
        counters: current.clone(),
        wall,
        monotonic,
    });
    let Some(old) = old else {
        return (Reading::FirstInterval {}, None);
    };
    let Some(elapsed) = monotonic.checked_sub(old.monotonic).filter(|v| *v > 0) else {
        return (failed("cpu_monotonic_clock_anomaly"), None);
    };
    if old.counters.counter_bits != current.counter_bits
        || old.counters.logical_cpu_count != current.logical_cpu_count
    {
        return (Reading::FirstInterval {}, None);
    }
    // Darwin exports integral scheduler ticks (normally 100 Hz). Allow up to
    // 1000 ticks/second/core plus one rounding tick/core. This generous physical
    // bound rejects discontinuities; it does not assume exact timer alignment.
    let limit = (u128::from(elapsed) + 1) * u128::from(current.logical_cpu_count);
    let modulus = 1u128 << current.counter_bits;
    // Apply the conservative half-range ambiguity bound to every endpoint pair.
    // A complete wrap can leave after >= before, so increasing endpoints alone
    // cannot establish that subtraction measures the whole interval.
    if limit >= modulus / 2 {
        return (failed("cpu_counter_reset_or_ambiguous_wrap"), None);
    }
    let delta = |before: u64, after: u64| -> Option<u128> {
        let (before, after) = (u128::from(before), u128::from(after));
        if after >= before {
            return Some(after - before);
        }
        // Only one boundary crossing, with both ends in a physically plausible
        // edge window, is accepted. Long gaps/ambiguous wraps are unknown.
        if before < modulus.saturating_sub(limit) || after > limit {
            return None;
        }
        Some(modulus - before + after)
    };
    let mut deltas = [0u128; 4];
    for (n, (before, after)) in values(&old.counters)
        .into_iter()
        .zip(values(current))
        .enumerate()
    {
        let Some(value) = delta(before, after) else {
            return (failed("cpu_counter_reset_or_ambiguous_wrap"), None);
        };
        deltas[n] = value;
    }
    let total: u128 = deltas.iter().sum();
    if total == 0 || total > limit {
        return (failed("cpu_counter_delta_implausible"), None);
    }
    let busy = total - deltas[3];
    let percent = busy as f64 / total as f64 * 100.0;
    (
        Reading::Available { value: percent },
        Some(CpuInterval {
            started_at_ms: old.wall,
            ended_at_ms: wall,
            started_monotonic_ms: old.monotonic,
            ended_monotonic_ms: monotonic,
            elapsed_ms: elapsed,
            logical_cpu_count: current.logical_cpu_count,
            normalization: CpuNormalization::HostCapacity,
        }),
    )
}

#[cfg(test)]
#[path = "cpu_tests.rs"]
mod tests;

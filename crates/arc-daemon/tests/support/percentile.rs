//! Nearest-rank percentile shared by the native measurement targets.
#![allow(dead_code)] // Each integration binary uses the subset it needs.

/// The nearest-rank percentile `ceil(percent/100 * N)` of the samples: always
/// a member of the sample set, never an interpolated value. Order of the
/// input does not matter. Panics on an empty set or a percent outside 1..=100.
pub fn nearest_rank(samples: &[u64], percent: usize) -> u64 {
    assert!(!samples.is_empty(), "percentile of an empty sample set");
    assert!((1..=100).contains(&percent), "percent {percent}");
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    sorted[(sorted.len() * percent).div_ceil(100) - 1]
}

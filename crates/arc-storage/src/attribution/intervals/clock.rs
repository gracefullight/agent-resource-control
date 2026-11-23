//! The two clocks of a scan, and the floor a backward wall-clock step implies.

/// A reading of the host's boot clock (it advances in sleep and never steps):
/// the boot session and nanoseconds since boot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootClock {
    pub boot_id: String,
    pub ns: u64,
}

/// The wall clock (whole ms, floored) and the boot clock read together.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanPoint {
    pub wall_ms: u64,
    pub boot: Option<BootClock>,
}
impl ScanPoint {
    pub fn wall(wall_ms: u64) -> Self {
        Self {
            wall_ms,
            boot: None,
        }
    }
}

/// A scan whose stretch onward is unresolved: its start, its floor and its
/// boot-clock reading when known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frontier {
    pub start_ms: u64,
    pub low_ms: u64,
    pub boot: Option<BootClock>,
}

/// The lowest wall time that a reading `at` shows to lie inside any of
/// `frontiers`' unresolved stretches below its floor, when the wall clock
/// stepped back since. In the frontier's boot session, the boot clock says
/// how much time passed; the wall time it had then by the current clock is
/// `at - elapsed`, and a value below the floor (beyond 1 ms plus 0.1% of the
/// elapsed time) is the new floor. Without a comparable boot reading, a wall
/// clock behind a frontier's start has stepped back by an unknown amount:
/// the floor is 0 (fail closed). `None`: no step back.
pub fn floor_of(frontiers: &[Frontier], at: &ScanPoint) -> Option<u64> {
    frontiers
        .iter()
        .filter_map(|frontier| match (&frontier.boot, &at.boot) {
            (Some(then), Some(now)) if then.boot_id == now.boot_id && now.ns >= then.ns => {
                let elapsed_ms = (now.ns - then.ns) / 1_000_000;
                let implied = at.wall_ms.saturating_sub(elapsed_ms);
                (implied + 1 + elapsed_ms / 1000 < frontier.low_ms).then_some(implied)
            }
            _ => (at.wall_ms < frontier.start_ms && frontier.low_ms > 0).then_some(0),
        })
        .min()
}

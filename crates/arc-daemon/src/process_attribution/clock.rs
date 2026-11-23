//! The scan's clocks: the wall clock (rounded outward to whole ms), the
//! monotonic clock that bounds a delta's envelope, and the host's boot clock
//! (advancing in sleep, never stepped) that shows how far a wall clock
//! stepped back between two scans, even across a daemon restart.
//!
//! **Test seam.** With `ARC_ATTRIBUTION_WALL_SHIFT_FILE` set to an absolute
//! path, every wall reading of this scanner adds the signed whole ms that
//! file holds (none, unreadable or invalid: 0). It moves only this scanner's
//! wall clock, never a CPU reading, the monotonic or boot clock, or any
//! other component; without the variable nothing is read.
use std::{
    path::PathBuf,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

const NS_PER_MS: i128 = 1_000_000;
const SHIFT_ENV: &str = "ARC_ATTRIBUTION_WALL_SHIFT_FILE";

/// The opt-in wall-clock shift of [`SHIFT_ENV`].
#[derive(Debug, Clone, Default)]
pub(super) struct WallShift(Option<PathBuf>);
impl WallShift {
    pub(super) fn from_env() -> Self {
        Self(
            std::env::var_os(SHIFT_ENV)
                .map(PathBuf::from)
                .filter(|path| path.is_absolute()),
        )
    }
    fn ns(&self) -> i128 {
        let Some(path) = &self.0 else { return 0 };
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| text.trim().parse::<i64>().ok())
            .map_or(0, |ms| i128::from(ms) * NS_PER_MS)
    }
}

/// Nanoseconds of the host's boot clock: `CLOCK_MONOTONIC` on macOS and
/// `CLOCK_BOOTTIME` on Linux, both counting sleep.
fn boot_ns() -> Option<u64> {
    #[cfg(target_os = "macos")]
    let id = libc::CLOCK_MONOTONIC;
    #[cfg(not(target_os = "macos"))]
    let id = libc::CLOCK_BOOTTIME;
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: a valid clock id and a writable timespec.
    if unsafe { libc::clock_gettime(id, &mut time) } != 0 {
        return None;
    }
    u64::try_from(time.tv_sec)
        .ok()?
        .checked_mul(1_000_000_000)?
        .checked_add(u64::try_from(time.tv_nsec).ok()?)
}

/// One reading of the three clocks.
#[derive(Debug, Clone, Copy)]
pub(super) struct Clock {
    pub(super) wall_ns: i128,
    pub(super) mono: Instant,
    pub(super) boot_ns: Option<u64>,
}
impl Clock {
    pub(super) fn now(shift: &WallShift) -> Self {
        let mono = Instant::now();
        let boot_ns = boot_ns();
        let wall_ns = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() as i128)
            + shift.ns();
        Self {
            wall_ns,
            mono,
            boot_ns,
        }
    }
    pub(super) fn wall_ms(&self) -> u64 {
        floor_ms(self.wall_ns)
    }
}

/// The clocks just before and after one native scan.
#[derive(Debug, Clone, Copy)]
pub(super) struct ScanClock {
    pub(super) start: Clock,
    pub(super) end: Clock,
}
impl ScanClock {
    pub(super) fn start_ms(&self) -> u64 {
        floor_ms(self.start.wall_ns)
    }
    pub(super) fn end_ms(&self) -> u64 {
        ceil_ms(self.end.wall_ns).max(self.start_ms() + 1)
    }
}

pub(super) fn floor_ms(ns: i128) -> u64 {
    u64::try_from(ns.max(0).div_euclid(NS_PER_MS)).unwrap_or(u64::MAX)
}
fn ceil_ms(ns: i128) -> u64 {
    u64::try_from((ns.max(0) + NS_PER_MS - 1).div_euclid(NS_PER_MS)).unwrap_or(u64::MAX)
}

/// The envelope `[earlier.start, later.end]` in whole ms, containing every
/// read of both scans. Wall time that advanced less than monotonic time
/// stepped back by that deficit; the envelope widens by it on both sides.
/// `None` when the deficit exceeds 1 ms plus 0.1% of the span: the wall
/// clock is not trusted to bound the reads.
pub(super) fn envelope(earlier: &ScanClock, later: &ScanClock) -> Option<(u64, u64)> {
    let points = [earlier.start, earlier.end, later.start, later.end];
    let deficit: i128 = points
        .windows(2)
        .map(|pair| {
            let mono = pair[1]
                .mono
                .saturating_duration_since(pair[0].mono)
                .as_nanos() as i128;
            (mono - (pair[1].wall_ns - pair[0].wall_ns)).max(0)
        })
        .sum();
    let span = later
        .end
        .mono
        .saturating_duration_since(earlier.start.mono)
        .as_nanos() as i128;
    if deficit > NS_PER_MS + span / 1000 {
        return None;
    }
    let start = floor_ms(earlier.start.wall_ns - deficit);
    let end = ceil_ms(later.end.wall_ns + deficit);
    (start < end && (start..end).contains(&later.start_ms())).then_some((start, end))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn at(wall_ms: i128, mono: Instant, mono_ms: u64) -> Clock {
        Clock {
            wall_ns: wall_ms * NS_PER_MS,
            mono: mono + Duration::from_millis(mono_ms),
            boot_ns: None,
        }
    }

    #[test]
    fn an_envelope_contains_both_scans_and_a_reversed_clock_is_refused() {
        let t = Instant::now();
        let scan = |wall: i128, mono: u64, length: u64| ScanClock {
            start: at(wall, t, mono),
            end: at(wall + length as i128, t, mono + length),
        };
        // Reads at 10-15 ms and 2010-2014 ms: the earlier scan's start to
        // the later scan's end, not one completion to the next.
        assert_eq!(
            envelope(&scan(10, 10, 5), &scan(2010, 2010, 4)),
            Some((10, 2014))
        );
        // Sub-millisecond bounds round outward.
        let fine = ScanClock {
            start: Clock {
                wall_ns: 10 * NS_PER_MS + 600_000,
                mono: t,
                boot_ns: None,
            },
            end: Clock {
                wall_ns: 11 * NS_PER_MS + 100_000,
                mono: t + Duration::from_micros(500),
                boot_ns: None,
            },
        };
        assert_eq!((fine.start_ms(), fine.end_ms()), (10, 12));
        // A 2 ms backward step within the tolerance widens both sides.
        assert_eq!(
            envelope(&scan(10, 10, 5), &scan(2008, 2010, 4)),
            Some((8, 2014))
        );
        // A 1 s backward step is not trusted to bound the reads.
        assert_eq!(envelope(&scan(10, 10, 5), &scan(1010, 2010, 4)), None);
        // A wall clock that moved forward keeps the reads inside.
        assert_eq!(
            envelope(&scan(10, 10, 5), &scan(9010, 2010, 4)),
            Some((10, 9014))
        );
    }
    #[test]
    fn the_wall_shift_moves_only_the_wall_clock_and_only_when_opted_in() {
        let file = std::env::temp_dir().join(format!(
            "arc-wall-shift-{}-{}",
            std::process::id(),
            arc_core::process_identity::random_job_id().unwrap()
        ));
        std::fs::write(&file, "-3600000\n").unwrap();
        let shifted = WallShift(Some(file.clone()));
        let (plain, moved) = (Clock::now(&WallShift::default()), Clock::now(&shifted));
        let back = plain.wall_ns - moved.wall_ns;
        assert!(
            (3_599_000 * NS_PER_MS..=3_600_000 * NS_PER_MS).contains(&back),
            "{back}"
        );
        assert!(moved.mono >= plain.mono);
        assert!(moved.boot_ns.unwrap() >= plain.boot_ns.unwrap());
        std::fs::write(&file, "not a number").unwrap();
        assert!(Clock::now(&shifted).wall_ns >= moved.wall_ns + 3_599_000 * NS_PER_MS);
        std::fs::remove_file(&file).unwrap();
    }
}

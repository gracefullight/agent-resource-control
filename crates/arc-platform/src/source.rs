use crate::{NativeSample, NativeSource};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
#[cfg(any(not(target_os = "macos"), test))]
use {crate::NativeFilesystem, arc_core::meter::Reading};

/// Finite, in-process observations of this host and this process only. The
/// sampler supplies its epoch; IPC reads never construct or invoke this source.
pub struct SystemSource {
    epoch: Instant,
}

impl SystemSource {
    pub fn new(epoch: Instant) -> Self {
        Self { epoch }
    }
}

impl NativeSource for SystemSource {
    fn sample(&mut self) -> NativeSample {
        let observed_at_ms = wall_time(SystemTime::now());
        let monotonic_ms = self.epoch.elapsed().as_millis().min(u64::MAX as u128) as u64;
        #[cfg(target_os = "macos")]
        {
            crate::macos::sample(observed_at_ms, monotonic_ms)
        }
        #[cfg(not(target_os = "macos"))]
        {
            unsupported(observed_at_ms, monotonic_ms)
        }
    }
}

fn wall_time(now: SystemTime) -> Option<u64> {
    now.duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|value| u64::try_from(value.as_millis()).ok())
        .filter(|value| *value <= arc_core::meter::MAX_SAFE_INTEGER)
}

#[cfg(any(not(target_os = "macos"), test))]
fn unsupported(observed_at_ms: Option<u64>, monotonic_ms: u64) -> NativeSample {
    NativeSample {
        observed_at_ms,
        monotonic_ms,
        host_cpu: Reading::Unsupported {},
        memory_available_bytes: Reading::Unsupported {},
        memory_pressure: Reading::Unsupported {},
        swap_used_bytes: Reading::Unsupported {},
        filesystem: NativeFilesystem {
            identity: None,
            capacity_bytes: Reading::Unsupported {},
            free_bytes: Reading::Unsupported {},
            available_bytes: Reading::Unsupported {},
        },
        daemon_cpu_core_seconds: Reading::Unsupported {},
        daemon_rss_bytes: Reading::Unsupported {},
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn unsupported_is_unknown_and_bad_wall_clocks_never_become_epoch_zero() {
        let sample = unsupported(None, 1234);
        assert_eq!(sample.monotonic_ms, 1234);
        assert_eq!(sample.observed_at_ms, None);
        assert!(matches!(sample.host_cpu, Reading::Unsupported {}));
        assert_eq!(sample.daemon_rss_bytes.value(), None);
        assert_eq!(sample.filesystem.capacity_bytes.value(), None);
        assert_eq!(wall_time(UNIX_EPOCH - Duration::from_millis(1)), None);
        assert_eq!(wall_time(UNIX_EPOCH + Duration::from_millis(17)), Some(17));
        assert_eq!(
            wall_time(UNIX_EPOCH + Duration::from_millis(arc_core::meter::MAX_SAFE_INTEGER + 1)),
            None
        );
    }
}

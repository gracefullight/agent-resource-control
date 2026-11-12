//! Native observation and owned-child accounting. No scheduling, IPC or history.
#[cfg(unix)]
pub mod child_wait;
#[cfg(unix)]
pub mod job_lineage;
use arc_core::meter::{FilesystemIdentity, MemoryPressure, Reading};

mod source;
pub use source::SystemSource;
#[cfg(target_os = "macos")]
mod macos;

/// Cumulative host CPU ticks. Native providers declare the actual counter width;
/// conversion, reset/wrap decisions and normalization belong to arc-meter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CpuCounters {
    pub user: u64,
    pub system: u64,
    pub nice: u64,
    pub idle: u64,
    pub counter_bits: u8,
    pub logical_cpu_count: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeFilesystem {
    pub identity: Option<FilesystemIdentity>,
    pub capacity_bytes: Reading<u64>,
    pub free_bytes: Reading<u64>,
    pub available_bytes: Reading<u64>,
}

/// One finite native pass. UTC can be unavailable independently of the readings.
/// monotonic_ms uses the owning sampler's epoch and never a process-wide clock
/// value that another sampler or restarted daemon could accidentally compare.
#[derive(Debug, Clone, PartialEq)]
pub struct NativeSample {
    pub observed_at_ms: Option<u64>,
    pub monotonic_ms: u64,
    pub host_cpu: Reading<CpuCounters>,
    /// Darwin free + inactive pages, multiplied by native page size. The native
    /// free count already includes speculative pages; never count those twice.
    /// This reclaimable-page estimate excludes compressed and purgeable overlap.
    pub memory_available_bytes: Reading<u64>,
    pub memory_pressure: Reading<MemoryPressure>,
    pub swap_used_bytes: Reading<u64>,
    pub filesystem: NativeFilesystem,
    /// This process's user + system CPU time since process launch, in core-seconds.
    pub daemon_cpu_core_seconds: Reading<f64>,
    pub daemon_rss_bytes: Reading<u64>,
}

/// The daemon gives exactly one worker mutable ownership of this source. It may
/// not spawn subprocesses, retain an unbounded native queue or consult GUI state.
/// Partial native failures are per-field readings, never a failed whole sample.
pub trait NativeSource: Send + 'static {
    fn sample(&mut self) -> NativeSample;
}

//! Seams for an explicitly selected fixture daemon. The resident daemon never
//! installs them: its clock is the system clock, its passes follow the policy
//! interval and no evidence fault exists.
use std::sync::OnceLock;

pub(crate) struct Seams {
    /// Injected wall clock in milliseconds.
    pub now_ms: fn() -> u64,
    /// The sequence number of a requested pass, if any.
    pub requested: fn() -> Option<u64>,
    /// Called after the pass with that sequence number has finished.
    pub completed: fn(u64),
    /// An injected evidence fault by name. Always reported as injected.
    pub fault: fn(&str) -> Option<String>,
}

static SEAMS: OnceLock<Seams> = OnceLock::new();

#[allow(dead_code)] // Only the Reaper fixture daemon installs seams.
pub(crate) fn install(seams: Seams) {
    assert!(SEAMS.set(seams).is_ok(), "Reaper seams installed twice");
}

pub(super) fn seams() -> Option<&'static Seams> {
    SEAMS.get()
}

pub(super) fn now_ms() -> u64 {
    match seams() {
        Some(seams) => (seams.now_ms)(),
        None => std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
    }
}

pub(super) fn fault(name: &str) -> Option<String> {
    seams().and_then(|seams| (seams.fault)(name))
}

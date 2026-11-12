//! Test-only failure injection around real native observations. No production
//! daemon environment variable, simulated native proof or global OS mutation.
use arc_core::meter::{Diagnostic, Reading};
use arc_platform::{NativeSample, NativeSource, SystemSource};
use std::time::Instant;

pub(super) struct Source(SystemSource);
impl Source {
    pub(super) fn new(epoch: Instant) -> Self {
        Self(SystemSource::new(epoch))
    }
}
impl NativeSource for Source {
    fn sample(&mut self) -> NativeSample {
        let mut sample = self.0.sample();
        if std::env::var_os("ARC_TEST_METER_PARTIAL").as_deref() == Some(std::ffi::OsStr::new("1"))
        {
            sample.memory_available_bytes = Reading::Denied {};
            sample.swap_used_bytes = Reading::Failed {
                diagnostic: Diagnostic::bounded("injected_native_failure"),
            };
        }
        sample
    }
}

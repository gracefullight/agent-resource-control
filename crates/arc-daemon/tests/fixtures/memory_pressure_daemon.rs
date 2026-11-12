//! Explicit injected source; all startup, routing and supervision is production code.
#![allow(dead_code)]
#[path = "../../src/accept_recovery.rs"]
mod accept_recovery;
#[path = "../../src/admission.rs"]
mod admission;
#[path = "../../src/connection.rs"]
mod connection;
#[path = "../../src/db_worker.rs"]
mod db_worker;
#[path = "../../src/entry.rs"]
mod entry;
#[path = "../../src/instance.rs"]
mod instance;
#[path = "../../src/job_meter_dispatch.rs"]
mod job_meter_dispatch;
#[path = "../../src/lifecycle.rs"]
mod lifecycle;
#[path = "../../src/limits.rs"]
mod limits;
#[path = "../../src/meter.rs"]
mod meter;
#[path = "../../src/platform.rs"]
mod platform;
#[path = "../../src/runtime_metrics.rs"]
mod runtime_metrics;
#[path = "../../src/runtime_report.rs"]
mod runtime_report;
#[path = "../../src/server.rs"]
mod server;
#[path = "../../src/transport.rs"]
mod transport;
#[cfg(test)]
#[path = "../../src/transport_test_support.rs"]
mod transport_test_support;
use arc_core::{
    meter::{Diagnostic, MemoryPressure, Reading},
    process_identity::ProcessIdentity,
};
use arc_meter::{MeterWorker, NativeSource, SamplingPolicy, SystemSource};
use std::{io::Read, path::PathBuf, time::Instant};

struct Controlled {
    system: SystemSource,
    control: PathBuf,
}
impl NativeSource for Controlled {
    fn sample(&mut self) -> arc_platform::NativeSample {
        let mut sample = self.system.sample();
        // No native sample is called Critical by this fixture: this field is
        // explicitly injected. Other fields still come from SystemSource.
        let mut bytes = Vec::with_capacity(32);
        let control = std::fs::File::open(&self.control)
            .and_then(|file| file.take(32).read_to_end(&mut bytes))
            .map(|_| bytes);
        sample.memory_pressure = match control.as_deref() {
            Ok(b"Normal") => Reading::Available {
                value: MemoryPressure::Normal,
            },
            Ok(b"Critical") => Reading::Available {
                value: MemoryPressure::Critical,
            },
            Ok(b"Warning") => Reading::Available {
                value: MemoryPressure::Warning,
            },
            Ok(b"Unsupported") => Reading::Unsupported {},
            Ok(b"Denied") => Reading::Denied {},
            _ => Reading::Failed {
                diagnostic: Diagnostic::bounded("injected_source_unavailable"),
            },
        };
        sample
    }
}
#[tokio::main(worker_threads = 2)]
async fn main() -> std::process::ExitCode {
    let control =
        PathBuf::from(std::env::var_os("ARC_PRESSURE_CONTROL").expect("explicit control file"));
    let epoch = Instant::now();
    let meter = MeterWorker::start(
        Controlled {
            system: SystemSource::new(epoch),
            control,
        },
        ProcessIdentity::capture(std::process::id()).unwrap(),
        epoch,
        SamplingPolicy::new(10_000, 1000, 1000).unwrap(),
    )
    .unwrap();
    // The worker's active cadence bounds synchronization during acceptance.
    meter.activity().set_active(true);
    entry::run(Some(meter)).await
}

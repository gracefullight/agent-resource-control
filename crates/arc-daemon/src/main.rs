//! Resident ARC daemon: migrate, then serve owner-only local IPC until SIGTERM.
//!
//! The single SQLite writer stays inside this process. The full Sweeper does
//! not start. The bounded process Reaper (`reaper.rs`) runs only when
//! `ARC_REAPER=process` is set; it then records its decisions in
//! `gc_operations` and `audit_events`, and only a managed-exec wrapper ever
//! signals its own child.

mod accept_recovery;
mod admission;
mod connection;
mod db_worker;
mod instance;
mod job_meter_dispatch;
mod lifecycle;
mod limits;
mod meter;
mod platform;
mod reaper;
mod runtime_metrics;
mod runtime_report;
mod server;
mod transport;

#[cfg(test)]
mod lifecycle_tests;
#[cfg(test)]
#[allow(dead_code)]
#[path = "../tests/support/native_fixture.rs"]
mod native_fixture;
#[cfg(test)]
#[allow(dead_code)] // The standalone transport target uses the remaining test helpers.
mod transport_test_support;

mod entry;
#[cfg(test)]
use entry::prepare_instance;

#[tokio::main(worker_threads = 2)]
async fn main() -> std::process::ExitCode {
    entry::run(None).await
}

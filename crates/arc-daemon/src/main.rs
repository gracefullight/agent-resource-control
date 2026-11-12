//! Resident ARC daemon: migrate, then serve owner-only local IPC until SIGTERM.
//!
//! The single SQLite writer stays inside this process. No sweeper starts and
//! no `gc_operations` rows are inserted.

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
mod runtime_metrics;
mod runtime_report;
mod server;
mod transport;

#[cfg(test)]
mod lifecycle_tests;
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

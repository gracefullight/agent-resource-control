//! Deterministic Reaper passes around the production daemon and its writer.
//!
//! Startup, IPC, storage, validation and the wrapper round trip are production
//! code. This binary adds only what a test controls through one directory:
//! `clock` (milliseconds), `pass` (requested sequence), `done` (finished
//! sequence) and `fault-<name>` (an explicitly injected evidence fault).
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
#[cfg(test)]
#[path = "../support/native_fixture.rs"]
mod native_fixture;
#[path = "../../src/platform.rs"]
mod platform;
#[path = "../../src/reaper.rs"]
mod reaper;
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

use std::{path::PathBuf, sync::OnceLock};

static CONTROL: OnceLock<PathBuf> = OnceLock::new();

fn read(name: &str) -> Option<String> {
    let control = CONTROL.get().expect("fixture control directory");
    std::fs::read_to_string(control.join(name)).ok()
}

fn now_ms() -> u64 {
    read("clock")
        .expect("fixture clock readable")
        .trim()
        .parse()
        .expect("fixture clock is milliseconds")
}

fn requested() -> Option<u64> {
    read("pass")?.trim().parse().ok()
}

fn completed(sequence: u64) {
    let control = CONTROL.get().expect("fixture control directory");
    // Rename so a reader never observes a partially written sequence.
    let pending = control.join("done.tmp");
    std::fs::write(&pending, sequence.to_string()).expect("write finished pass");
    std::fs::rename(pending, control.join("done")).expect("publish finished pass");
}

fn fault(name: &str) -> Option<String> {
    read(&format!("fault-{name}")).map(|value| value.trim().to_owned())
}

#[tokio::main(worker_threads = 2)]
async fn main() -> std::process::ExitCode {
    let mut args = std::env::args_os().skip(1);
    let (Some(option), Some(path), None) = (args.next(), args.next(), args.next()) else {
        eprintln!("usage: --fixture-control <directory>");
        return std::process::ExitCode::FAILURE;
    };
    if option != "--fixture-control" || CONTROL.set(PathBuf::from(path)).is_err() {
        eprintln!("invalid Reaper fixture option");
        return std::process::ExitCode::FAILURE;
    }
    reaper::fixture::install(reaper::fixture::Seams {
        now_ms,
        requested,
        completed,
        fault,
    });
    entry::run(None).await
}

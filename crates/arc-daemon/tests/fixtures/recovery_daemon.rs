//! Explicit two-stage commit barrier around the production daemon's SQLite worker.
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

use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        OnceLock,
    },
    time::Duration,
};

static BARRIER: OnceLock<PathBuf> = OnceLock::new();
static COMMITTING: AtomicBool = AtomicBool::new(false);

/// The writer calls this once inside the commit hook of a lifecycle start and
/// once more after that start committed. Each call announces its stage on the
/// test's socket and stays there until the test releases it with one byte; a
/// test that never releases a stage kills the daemon inside it.
fn commit_boundary() {
    let stage: &[u8] = if COMMITTING.fetch_xor(true, Ordering::AcqRel) {
        b"after-commit\n"
    } else {
        b"before-commit\n"
    };
    // The fixture waits only on its explicitly supplied test socket. Failure to
    // connect cannot masquerade as a reached barrier: the test must receive it.
    if let Ok(mut stream) = UnixStream::connect(BARRIER.get().expect("fixture barrier")) {
        let _ = stream.set_read_timeout(Some(Duration::from_secs(120)));
        if stream.write_all(stage).is_ok() {
            let mut release = [0];
            let _ = stream.read(&mut release);
        }
    }
}

#[tokio::main(worker_threads = 2)]
async fn main() -> std::process::ExitCode {
    let mut args = std::env::args_os().skip(1);
    if let Some(option) = args.next() {
        if option != "--fixture-commit-barrier" {
            eprintln!("invalid recovery fixture option");
            return std::process::ExitCode::FAILURE;
        }
        let Some(path) = args.next() else {
            eprintln!("missing recovery fixture barrier");
            return std::process::ExitCode::FAILURE;
        };
        if args.next().is_some() || BARRIER.set(PathBuf::from(path)).is_err() {
            return std::process::ExitCode::FAILURE;
        }
        db_worker::install_fixture_before_commit(commit_boundary);
    }
    entry::run(None).await
}

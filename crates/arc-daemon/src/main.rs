//! Resident ARC daemon: migrate, then serve owner-only local IPC until SIGTERM.
//!
//! The single SQLite writer stays inside this process. No sweeper starts and
//! no `gc_operations` rows are inserted.

mod accept_recovery;
mod admission;
mod connection;
mod db_worker;
mod instance;
mod lifecycle;
mod limits;
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

use arc_storage::open_and_migrate;
use std::process::ExitCode;

#[tokio::main(worker_threads = 2)]
async fn main() -> ExitCode {
    let database_path = match arc_core::database_path() {
        Ok(path) => path,
        Err(error) => return fail_database(&error),
    };
    // Hold per-user admission before opening the single SQLite writer.
    let (socket, _instance) = match prepare_instance() {
        Ok(instance) => instance,
        Err(error) => return fail_daemon(&error),
    };
    let connection = match open_and_migrate(&database_path) {
        Ok(connection) => connection,
        Err(error) => return fail_database(&error),
    };

    match server::serve(connection, socket).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => fail_daemon(&stage("server", error)),
    }
}

fn prepare_instance() -> std::io::Result<(std::path::PathBuf, instance::InstanceLock)> {
    let socket = arc_core::socket_path().map_err(|error| stage("socket path", error))?;
    server::validate_socket_path(&socket).map_err(|error| stage("socket directory", error))?;
    let anchor = arc_core::instance_anchor().map_err(|error| stage("instance path", error))?;
    instance::validate_lock_path(&anchor).map_err(|error| stage("instance lock", error))?;
    server::prepare_socket_directory(&anchor)
        .map_err(|error| stage("instance directory", error))?;
    let instance =
        instance::InstanceLock::acquire(&anchor).map_err(|error| stage("instance lock", error))?;
    server::prepare_socket_directory(&socket).map_err(|error| stage("socket directory", error))?;
    Ok((socket, instance))
}

fn fail_daemon(error: &(impl std::fmt::Display + ?Sized)) -> ExitCode {
    eprintln!("Daemon startup failed: {}.", sanitize(error));
    ExitCode::FAILURE
}

fn fail_database(error: &(impl std::fmt::Display + ?Sized)) -> ExitCode {
    eprintln!("Database startup failed: {}.", sanitize(error));
    ExitCode::FAILURE
}

fn sanitize(error: &(impl std::fmt::Display + ?Sized)) -> String {
    error.to_string().replace(['\r', '\n'], " ")
}

fn stage(label: &str, error: impl std::fmt::Display) -> std::io::Error {
    std::io::Error::other(format!("{label}: {error}"))
}

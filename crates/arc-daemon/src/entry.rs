//! Shared resident startup for the default daemon and owned native fixtures.
use crate::{instance, server};
use arc_storage::open_and_migrate;
use std::process::ExitCode;

pub(crate) async fn run(meter: Option<arc_meter::MeterWorker>) -> ExitCode {
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

    match server::serve_with_source(connection, socket, meter).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => fail_daemon(&stage("server", error)),
    }
}

pub(crate) fn prepare_instance() -> std::io::Result<(std::path::PathBuf, instance::InstanceLock)> {
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

//! Atomic, forward-only SQLite migrations with compiled inputs.
//!
//! Callers own the single writer; no pool, background worker, or cleanup starts
//! here. Store resource metadata, never credentials or original chat text.

mod history;
pub mod job_meter;
pub mod jobs;
mod migration;
pub mod policy;

pub use migration::{
    assess_schema, Migration, MigrationError, SchemaCompatibility, SchemaStatus, MIGRATIONS,
    MIGRATION_ID,
};
use rusqlite::Connection;
use std::{error::Error, fs, io, path::Path, time::Duration};

pub type StorageResult<T> = Result<T, Box<dyn Error>>;

/// Resolve the user-local path through arc-core, then open and migrate it.
pub fn open_default() -> StorageResult<Connection> {
    open_and_migrate(arc_core::database_path()?)
}

/// Configure a local connection and atomically apply its pending migrations.
pub fn open_and_migrate(path: impl AsRef<Path>) -> StorageResult<Connection> {
    migration::validate_registry(MIGRATIONS)?;
    let mut connection = open_connection(path.as_ref())?;
    migration::migrate(&mut connection, MIGRATIONS)?;
    Ok(connection)
}

fn open_connection(path: &Path) -> StorageResult<Connection> {
    if path.as_os_str().is_empty() || path == Path::new(":memory:") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a local database file is required",
        )
        .into());
    }
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)?;
    }
    let connection = Connection::open(path)?;
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.execute_batch("PRAGMA foreign_keys=ON;")?;
    let mode: String = connection.query_row("PRAGMA journal_mode=WAL", [], |row| row.get(0))?;
    if !mode.eq_ignore_ascii_case("wal") {
        return Err(io::Error::other("database did not enable WAL").into());
    }
    Ok(connection)
}

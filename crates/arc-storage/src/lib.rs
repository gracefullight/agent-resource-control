//! One SQLite connection and one atomic, forward-only Phase 0 migration.
//!
//! Callers own the single writer; no pool, background worker, or cleanup starts
//! here. Only resource metadata belongs in this database: never credentials,
//! authentication tokens, original chat text, or cookies.

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use sha2::{Digest, Sha256};
use std::{error::Error, fs, io, path::Path, time::Duration};

pub type StorageResult<T> = Result<T, Box<dyn Error>>;

pub const MIGRATION_ID: &str = "0001_phase0";
const MIGRATION_SQL: &str = include_str!("../migrations/0001_phase0.sql");

/// Resolve the user-local path through arc-core, then open and migrate it.
pub fn open_default() -> StorageResult<Connection> {
    open_and_migrate(arc_core::database_path()?)
}

/// Open a file on a user-local filesystem and configure each new connection.
///
/// An IMMEDIATE transaction serializes the migration with other SQLite writers.
/// DDL and its checksum record commit together, or both roll back on failure.
pub fn open_and_migrate(path: impl AsRef<Path>) -> StorageResult<Connection> {
    let path = path.as_ref();
    if path.as_os_str().is_empty() || path == Path::new(":memory:") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "a local database file is required",
        )
        .into());
    }
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    let mut connection = Connection::open(path)?;
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.execute_batch("PRAGMA foreign_keys=ON;")?;
    let journal_mode: String =
        connection.query_row("PRAGMA journal_mode=WAL", [], |row| row.get(0))?;
    if !journal_mode.eq_ignore_ascii_case("wal") {
        return Err(io::Error::other("database did not enable WAL").into());
    }

    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let has_history: bool = transaction.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name='schema_migrations')",
        [],
        |row| row.get(0),
    )?;
    let checksum: String = Sha256::digest(MIGRATION_SQL.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if has_history {
        let stored: Option<String> = transaction
            .query_row(
                "SELECT checksum FROM schema_migrations WHERE id=?1",
                [MIGRATION_ID],
                |row| row.get(0),
            )
            .optional()?;
        if stored.as_deref() != Some(checksum.as_str()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Phase 0 migration checksum is missing or mismatched",
            )
            .into());
        }
        let migration_count: i64 =
            transaction.query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })?;
        if migration_count != 1 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unsupported database migration history",
            )
            .into());
        }
    } else {
        transaction.execute_batch(MIGRATION_SQL)?;
        transaction.execute(
            "INSERT INTO schema_migrations (id, applied_at, checksum) VALUES (?1, unixepoch(), ?2)",
            params![MIGRATION_ID, checksum],
        )?;
    }
    transaction.commit()?;
    Ok(connection)
}

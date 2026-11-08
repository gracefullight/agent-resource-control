use arc_storage::{
    assess_schema, open_and_migrate, MigrationError, SchemaCompatibility, SchemaStatus,
};
use rusqlite::Connection;
use std::{
    error::Error,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "arc-storage-coverage-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn invalid_database_locations_preserve_caller_files() {
    for path in ["", ":memory:"] {
        let error = open_and_migrate(path).unwrap_err();
        let error = error.downcast_ref::<std::io::Error>().unwrap();
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
        assert_eq!(error.to_string(), "a local database file is required");
    }
    let directory = Directory::new();
    let blocker = directory.0.join("not-a-directory");
    fs::write(&blocker, b"preserved").unwrap();
    let error = open_and_migrate(blocker.join("db.sqlite3")).unwrap_err();
    assert!(error.downcast_ref::<std::io::Error>().is_some());
    assert_eq!(fs::read(&blocker).unwrap(), b"preserved");
    assert!(open_and_migrate(&directory.0)
        .unwrap_err()
        .downcast_ref::<rusqlite::Error>()
        .is_some());
}

#[test]
fn a_memory_uri_cannot_satisfy_the_required_wal_mode() {
    let error = open_and_migrate("file::memory:?cache=private").unwrap_err();
    let error = error.downcast_ref::<std::io::Error>().unwrap();
    assert_eq!(error.to_string(), "database did not enable WAL");
}

#[test]
fn view_named_history_is_invalid_and_assessment_is_read_only() {
    let connection = Connection::open_in_memory().unwrap();
    connection
        .execute_batch("CREATE VIEW schema_migrations AS SELECT 1 AS id")
        .unwrap();
    let before = connection.total_changes();
    let report = assess_schema(&connection).unwrap();
    assert_eq!(report.status, SchemaStatus::InvalidHistory);
    assert!(!report.history_verified);
    assert_eq!(report.applied_version, None);
    assert_eq!(report.schema_migrations, 0);
    assert_eq!(connection.total_changes(), before);
    assert!(connection.is_autocommit());
}

#[test]
fn invalid_utf8_history_is_rejected_without_rewriting_bytes() {
    let connection = Connection::open_in_memory().unwrap();
    connection
        .execute_batch(
            "CREATE TABLE schema_migrations (id TEXT, applied_at INTEGER, checksum TEXT)",
        )
        .unwrap();
    for (id, hash) in [
        (vec![0xff], b"hash".to_vec()),
        (b"0001_phase0".to_vec(), vec![0xff]),
    ] {
        connection
            .execute("DELETE FROM schema_migrations", [])
            .unwrap();
        connection
            .execute(
                "INSERT INTO schema_migrations VALUES (CAST(?1 AS TEXT), 1, CAST(?2 AS TEXT))",
                rusqlite::params![id, hash],
            )
            .unwrap();
        let before = connection.total_changes();
        let report = assess_schema(&connection).unwrap();
        assert_eq!(report.status, SchemaStatus::InvalidHistory);
        assert!(!report.history_verified);
        assert_eq!(report.schema_migrations, 1);
        assert_eq!(connection.total_changes(), before);
        let stored: (Vec<u8>, Vec<u8>) = connection
            .query_row(
                "SELECT CAST(id AS BLOB), CAST(checksum AS BLOB) FROM schema_migrations",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(stored, (id, hash));
    }
}

#[test]
fn migration_errors_keep_diagnostics_and_sources() {
    let sql = rusqlite::Error::InvalidQuery;
    let expected = format!("SQLite migration operation failed: {sql}");
    let error = MigrationError::from(sql);
    assert_eq!(error.to_string(), expected);
    assert!(error
        .source()
        .unwrap()
        .downcast_ref::<rusqlite::Error>()
        .is_some());
    let invalid = MigrationError::InvalidRegistry("fixture reason");
    assert_eq!(
        invalid.to_string(),
        "invalid compiled migration registry: fixture reason"
    );
    assert!(invalid.source().is_none());
}

#[test]
fn incompatible_schema_messages_preserve_recovery_guidance_and_versions() {
    for (status, applied_version, message) in [
        (SchemaStatus::UnsupportedNewer, Some(9), "unsupported newer schema version 9 (supported 1); use a binary supporting that version; downgrade execution is not supported"),
        (SchemaStatus::UnsupportedNewer, None, "unsupported newer schema version 0 (supported 1); use a binary supporting that version; downgrade execution is not supported"),
        (SchemaStatus::ChecksumMismatch, Some(1), "migration checksum mismatch; preserve the database and investigate against a trusted copy"),
        (SchemaStatus::MissingHistory, None, "missing migration history; preserve the database and investigate against a trusted copy"),
        (SchemaStatus::InvalidHistory, None, "invalid migration history; preserve the database and investigate against a trusted copy"),
    ] {
        let error = MigrationError::Incompatible(SchemaCompatibility {
            status, applied_version, supported_version: 1,
            history_verified: false, schema_migrations: 0,
        });
        assert_eq!(error.to_string(), message);
        assert!(error.source().is_none());
    }
}

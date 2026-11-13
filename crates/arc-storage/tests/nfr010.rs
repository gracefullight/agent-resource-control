use arc_storage::{open_and_migrate, open_default, MIGRATION_ID};
use rusqlite::Connection;
use sha2::{Digest, Sha256};
use std::{
    env, fs,
    os::unix::fs::DirBuilderExt,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

struct TempDatabase(PathBuf);

impl TempDatabase {
    fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let directory = arc_core::runtime_root().unwrap().join(format!(
            "b{:x}-{:x}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new()
            .mode(0o700)
            .create(&directory)
            .unwrap();
        Self(directory.join("arc.sqlite3"))
    }
}

impl Drop for TempDatabase {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(self.0.parent().unwrap());
    }
}

#[test]
fn nfr010_wal_migration_survives_close_and_reopen_without_cleanup() {
    if env::var_os("ARC_STORAGE_CHILD").is_none() {
        let database = TempDatabase::new();
        let output = std::process::Command::new(env::current_exe().unwrap())
            .args([
                "--exact",
                "nfr010_wal_migration_survives_close_and_reopen_without_cleanup",
            ])
            .env("ARC_STORAGE_CHILD", "1")
            .env(arc_core::ARC_DB_PATH, &database.0)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        return;
    }
    let database = TempDatabase(PathBuf::from(env::var_os(arc_core::ARC_DB_PATH).unwrap()));
    assert_eq!(arc_core::database_path().unwrap(), database.0);
    let connection = open_default().unwrap();
    let mode: String = connection
        .query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .unwrap();
    assert_eq!(mode, "wal");
    let foreign_keys: i64 = connection
        .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
        .unwrap();
    assert_eq!(foreign_keys, 1);

    let expected = [
        "vendors",
        "agent_instances",
        "projects",
        "agent_sessions",
        "jobs",
        "job_lifecycle",
        "job_meter_summaries",
        "governor_jobs",
        "governor_mutation_receipts",
        "traffic_entries",
        "traffic_tombstones",
        "permits",
        "resource_objects",
        "resource_leases",
        "gc_operations",
        "resource_samples",
        "policy_states",
        "policy_decisions",
        "audit_events",
    ];
    let mut statement = connection.prepare(
        "SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name"
    ).unwrap();
    let mut tables: Vec<String> = statement
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let mut expected_tables: Vec<String> = expected.into_iter().map(str::to_owned).collect();
    expected_tables.push("schema_migrations".into());
    expected_tables.sort();
    tables.sort();
    assert_eq!(tables, expected_tables);
    drop(statement);

    let count: i64 = connection
        .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(count, 6);
    let (id, applied_at, checksum): (String, i64, String) = connection
        .query_row(
            "SELECT id, applied_at, checksum FROM schema_migrations WHERE id='0001_phase0'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(id, MIGRATION_ID);
    assert!(applied_at > 0);
    assert_eq!(
        checksum,
        Sha256::digest(include_bytes!("../migrations/0001_phase0.sql"))
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    let second: (String, i64, String) = connection
        .query_row(
            "SELECT id, applied_at, checksum FROM schema_migrations WHERE id='0002_managed_exec'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(second.0, "0002_managed_exec");
    assert!(second.1 > 0);
    assert_eq!(
        second.2,
        Sha256::digest(include_bytes!("../migrations/0002_managed_exec.sql"))
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    assert_eq!(checksum.len(), 64);
    assert_eq!(
        checksum,
        "046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384"
    );
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM gc_operations", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );

    connection.execute("INSERT INTO vendors (id, display_name, discovered_at) VALUES ('test-vendor', 'Test vendor', 1)", []).unwrap();
    // close without wal_checkpoint: use SQLite's normal connection close path.
    connection.close().unwrap();

    let reopened = open_default().unwrap();
    let integrity: String = reopened
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");
    assert_eq!(
        reopened
            .query_row("PRAGMA journal_mode", [], |row| row.get::<_, String>(0))
            .unwrap(),
        "wal"
    );
    assert_eq!(
        reopened
            .query_row("PRAGMA foreign_keys", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        reopened
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        6
    );
    let recorded: (String, i64, String) = reopened
        .query_row(
            "SELECT id, applied_at, checksum FROM schema_migrations WHERE id='0001_phase0'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(recorded, (id, applied_at, checksum));
    let recorded_second: (String, i64, String) = reopened
        .query_row(
            "SELECT id, applied_at, checksum FROM schema_migrations WHERE id='0002_managed_exec'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(recorded_second, second);

    assert_eq!(
        reopened
            .query_row(
                "SELECT COUNT(*) FROM vendors WHERE id='test-vendor'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
    assert_eq!(
        reopened
            .query_row("SELECT COUNT(*) FROM gc_operations", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
    reopened.close().unwrap();
}

#[test]
fn mismatched_checksum_is_rejected_without_reapplying_migration() {
    let database = TempDatabase::new();
    let connection = open_and_migrate(&database.0).unwrap();
    connection
        .execute("UPDATE schema_migrations SET checksum='invalid'", [])
        .unwrap();
    connection.close().unwrap();
    assert!(open_and_migrate(&database.0)
        .unwrap_err()
        .to_string()
        .contains("checksum"));
    let connection = Connection::open(&database.0).unwrap();
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM gc_operations", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn migration_failure_rolls_back_all_new_tables() {
    let database = TempDatabase::new();
    let connection = Connection::open(&database.0).unwrap();
    connection
        .execute("CREATE TABLE projects (existing_data TEXT)", [])
        .unwrap();
    connection.close().unwrap();
    assert!(open_and_migrate(&database.0).is_err());
    let connection = Connection::open(&database.0).unwrap();
    let tables: Vec<String> = connection
        .prepare("SELECT name FROM sqlite_schema WHERE type='table'")
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(tables, ["projects"]);
}

#[test]
fn foreign_keys_and_unique_resource_identity_are_enforced() {
    let database = TempDatabase::new();
    let connection = open_and_migrate(&database.0).unwrap();
    assert!(connection.execute("INSERT INTO agent_instances (id, vendor_id, state, created_at) VALUES ('orphan', 'missing', 'NEW', 1)", []).is_err());
    connection.execute("INSERT INTO resource_objects (id, resource_type, resource_identity, ownership_level, state) VALUES ('one', 'test', 'identity', 'verified', 'NEW')", []).unwrap();
    assert!(connection.execute("INSERT INTO resource_objects (id, resource_type, resource_identity, ownership_level, state) VALUES ('two', 'test', 'identity', 'verified', 'NEW')", []).is_err());
}

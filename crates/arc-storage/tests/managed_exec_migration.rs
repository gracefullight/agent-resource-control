//! Current-registry counterparts to the immutable schema-1 regression fixtures.

use arc_storage::{assess_schema, open_and_migrate, open_default, SchemaStatus, MIGRATIONS};
use rusqlite::{types::Value, Connection};
use sha2::{Digest, Sha256};
use std::{
    env, fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

struct Database(PathBuf);

impl Database {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = env::temp_dir().join(format!(
            "arc-current-schema-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root.join("arc.sqlite3"))
    }

    fn open(&self) -> Connection {
        open_and_migrate(&self.0).unwrap()
    }
}

impl Drop for Database {
    fn drop(&mut self) {
        fs::remove_dir_all(self.0.parent().unwrap()).unwrap();
    }
}

type History = Vec<(String, i64, String)>;

fn history(c: &Connection) -> History {
    c.prepare("SELECT id, applied_at, checksum FROM schema_migrations ORDER BY id")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn checksum(sql: &str) -> String {
    Sha256::digest(sql.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[derive(Debug, PartialEq)]
struct Snapshot {
    schema: Vec<(String, String, Option<String>)>,
    data: Vec<(String, Vec<Vec<Value>>)>,
    witness: i64,
}

fn snapshot(c: &Connection) -> Snapshot {
    let schema: Vec<(String, String, Option<String>)> = c
        .prepare("SELECT type, name, sql FROM sqlite_schema ORDER BY type, name")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let data = schema
        .iter()
        .filter(|(kind, _, _)| kind == "table")
        .map(|(_, name, _)| {
            let mut statement = c
                .prepare(&format!(
                    "SELECT * FROM \"{}\" ORDER BY rowid",
                    name.replace('"', "\"\"")
                ))
                .unwrap();
            let columns = statement.column_count();
            let rows = statement
                .query_map([], |r| (0..columns).map(|n| r.get(n)).collect())
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            (name.clone(), rows)
        })
        .collect();
    Snapshot {
        schema,
        data,
        witness: c
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap(),
    }
}

fn assert_current(c: &Connection) {
    let assessment = assess_schema(c).unwrap();
    assert_eq!(assessment.status, SchemaStatus::Compatible);
    assert_eq!(assessment.applied_version, Some(6));
    assert_eq!(assessment.supported_version, 6);
    assert_eq!(assessment.schema_migrations, 6);
    assert!(assessment.history_verified);
    assert_eq!(MIGRATIONS.len(), 6);
    assert_eq!(MIGRATIONS[0].id, "0001_phase0");
    assert_eq!(MIGRATIONS[1].id, "0002_managed_exec");
    assert_eq!(MIGRATIONS[2].id, "0003_traffic");
    assert_eq!(MIGRATIONS[3].id, "0004_job_meter");
    assert_eq!(MIGRATIONS[4].id, "0005_memory_pressure");
    assert_eq!(MIGRATIONS[5].id, "0006_governor");
    let entries = history(c);
    assert_eq!(entries.len(), 6);
    for (entry, migration) in entries.iter().zip(MIGRATIONS) {
        assert_eq!(entry.0, migration.id);
        assert!(entry.1 > 0);
        assert_eq!(entry.2, checksum(migration.sql));
    }
    assert_eq!(
        entries[0].2,
        "046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384"
    );
    assert_eq!(
        entries[1].2,
        "464a1c88270f01a56ccab6610f87eff7648e0640f276ebce53b8a97026c50814"
    );
    assert_eq!(snapshot(c).witness, 6);
}

#[test]
fn current_registry_default_open_preserves_wal_inventory_history_and_reopen() {
    // Exercise default path resolution without mutating this test process's env.
    const CHILD: &str = "ARC_CURRENT_SCHEMA_CHILD";
    if env::var_os(CHILD).is_none() {
        let database = Database::new();
        let output = Command::new(env::current_exe().unwrap())
            .args([
                "--exact",
                "current_registry_default_open_preserves_wal_inventory_history_and_reopen",
            ])
            .env(CHILD, "1")
            .env(arc_core::ARC_DB_PATH, &database.0)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        return;
    }
    let c = open_default().unwrap();
    assert_current(&c);
    let actual: Vec<String> = snapshot(&c)
        .data
        .into_iter()
        .map(|(name, _)| name)
        .collect();
    assert_eq!(
        actual,
        [
            "agent_instances",
            "agent_sessions",
            "audit_events",
            "gc_operations",
            "governor_jobs",
            "governor_mutation_receipts",
            "job_lifecycle",
            "job_meter_summaries",
            "jobs",
            "permits",
            "policy_decisions",
            "policy_states",
            "projects",
            "resource_leases",
            "resource_objects",
            "resource_samples",
            "schema_migrations",
            "sqlite_sequence",
            "traffic_entries",
            "traffic_tombstones",
            "vendors",
        ]
    );
    c.execute(
        "INSERT INTO vendors (id, display_name, discovered_at) VALUES ('kept', 'original', 1)",
        [],
    )
    .unwrap();
    let before = snapshot(&c);
    c.close().unwrap();
    for _ in 0..3 {
        let c = open_default().unwrap();
        assert_current(&c);
        assert_eq!(snapshot(&c), before);
        assert_eq!(
            c.query_row("PRAGMA journal_mode", [], |r| r.get::<_, String>(0))
                .unwrap(),
            "wal"
        );
        assert_eq!(
            c.query_row("PRAGMA foreign_keys", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            c.query_row("PRAGMA integrity_check", [], |r| r.get::<_, String>(0))
                .unwrap(),
            "ok"
        );
        c.close().unwrap();
    }
}

#[test]
fn legacy_v1_without_witness_is_read_only_then_upgrades_without_rewriting_history() {
    let database = Database::new();
    let c = Connection::open(&database.0).unwrap();
    c.execute_batch(MIGRATIONS[0].sql).unwrap();
    c.execute(
        "INSERT INTO schema_migrations VALUES (?1, 123, ?2)",
        (MIGRATIONS[0].id, checksum(MIGRATIONS[0].sql)),
    )
    .unwrap();
    c.execute(
        "INSERT INTO vendors (id, display_name, discovered_at) VALUES ('kept', 'original', 1)",
        [],
    )
    .unwrap();
    let before = snapshot(&c);
    let old_history = history(&c);
    let assessment = assess_schema(&c).unwrap();
    assert_eq!(assessment.status, SchemaStatus::UpgradeRequired);
    assert_eq!(assessment.applied_version, Some(1));
    assert_eq!(assessment.supported_version, 6);
    assert_eq!(assessment.schema_migrations, 1);
    assert!(assessment.history_verified);
    assert_eq!(snapshot(&c), before);
    c.close().unwrap();
    let c = database.open();
    assert_current(&c);
    assert_eq!(history(&c)[..1], old_history);
    assert_eq!(
        c.query_row(
            "SELECT display_name FROM vendors WHERE id='kept'",
            [],
            |r| { r.get::<_, String>(0) }
        )
        .unwrap(),
        "original"
    );
    let upgraded = snapshot(&c);
    c.close().unwrap();
    assert_eq!(snapshot(&database.open()), upgraded);
}

#[test]
fn current_schema_rejects_invalid_history_and_witness_without_mutation() {
    for (sql, expected) in [
        ("PRAGMA user_version=0", SchemaStatus::InvalidHistory),
        ("PRAGMA user_version=1", SchemaStatus::InvalidHistory),
        ("PRAGMA user_version=2", SchemaStatus::InvalidHistory),
        ("PRAGMA user_version=3", SchemaStatus::InvalidHistory),
        ("PRAGMA user_version=4", SchemaStatus::InvalidHistory),
        ("PRAGMA user_version=5", SchemaStatus::InvalidHistory),
        ("PRAGMA user_version=7", SchemaStatus::UnsupportedNewer),
        ("DELETE FROM schema_migrations WHERE id='0001_phase0'", SchemaStatus::MissingHistory),
        ("DELETE FROM schema_migrations", SchemaStatus::MissingHistory),
        ("UPDATE schema_migrations SET checksum='invalid' WHERE id='0002_managed_exec'", SchemaStatus::ChecksumMismatch),
        ("UPDATE schema_migrations SET id='invalid' WHERE id='0002_managed_exec'", SchemaStatus::InvalidHistory),
        ("UPDATE schema_migrations SET id='0002_other' WHERE id='0002_managed_exec'", SchemaStatus::InvalidHistory),
        ("INSERT INTO schema_migrations VALUES ('0002_duplicate', 123, 'untrusted')", SchemaStatus::InvalidHistory),
        ("INSERT INTO schema_migrations VALUES ('0007_future', 123, 'untrusted'); PRAGMA user_version=7", SchemaStatus::UnsupportedNewer),
    ] {
        let database = Database::new();
        let c = database.open();
        c.execute_batch(sql).unwrap();
        let before = snapshot(&c);
        let assessment = assess_schema(&c).unwrap();
        assert_eq!(assessment.status, expected, "{sql}");
        assert!(!assessment.history_verified, "{sql}");
        assert_eq!(assessment.supported_version, 6);
        assert_eq!(snapshot(&c), before, "assessment changed database: {sql}");
        c.close().unwrap();
        let error = open_and_migrate(&database.0).unwrap_err();
        assert_eq!(
            error
                .downcast_ref::<arc_storage::MigrationError>()
                .map(|error| match error {
                    arc_storage::MigrationError::Incompatible(report) => Some(report),
                    _ => None,
                }),
            Some(Some(&assessment)),
            "{sql}"
        );
        if expected == SchemaStatus::UnsupportedNewer {
            assert!(error.to_string().contains("schema version 7 (supported 6)"));
        }
        assert_eq!(
            snapshot(&Connection::open(&database.0).unwrap()),
            before,
            "rejected migration changed database: {sql}"
        );
    }
}

fn initialize_v2(c: &Connection) {
    for migration in &MIGRATIONS[..2] {
        c.execute_batch(migration.sql).unwrap();
        c.execute(
            "INSERT INTO schema_migrations VALUES (?1, 123, ?2)",
            (migration.id, checksum(migration.sql)),
        )
        .unwrap();
    }
    c.execute_batch("PRAGMA user_version=2; INSERT INTO jobs (id,job_class,locality,enforcement,state,created_at) VALUES ('legacy','BUILD','HOST_LOCAL','OBSERVE','NEW',5); INSERT INTO permits (id,job_id,state,cpu_cores_est,memory_bytes_est,reserved_at) VALUES ('legacy-permit','legacy','RELEASED',1.5,1024,6);").unwrap();
}

#[test]
fn version_two_upgrade_preserves_existing_jobs_permits_and_both_checksums() {
    let db = Database::new();
    let c = Connection::open(&db.0).unwrap();
    initialize_v2(&c);
    let old_history = history(&c);
    assert_eq!(
        old_history[0].2,
        "046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384"
    );
    assert_eq!(
        old_history[1].2,
        "464a1c88270f01a56ccab6610f87eff7648e0640f276ebce53b8a97026c50814"
    );
    assert_eq!(
        assess_schema(&c).unwrap().status,
        SchemaStatus::UpgradeRequired
    );
    c.close().unwrap();
    let c = db.open();
    assert_current(&c);
    assert_eq!(history(&c)[..2], old_history);
    let permit: (String, String, f64, i64, Option<i64>, Option<i64>) = c.query_row(
        "SELECT job_id,state,cpu_cores_est,memory_bytes_est,cpu_millis_est,generation FROM permits WHERE id='legacy-permit'", [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
    ).unwrap();
    assert_eq!(
        permit,
        ("legacy".into(), "RELEASED".into(), 1.5, 1024, None, None)
    );
    let upgraded = snapshot(&c);
    c.close().unwrap();
    assert_eq!(snapshot(&db.open()), upgraded);
}

#[test]
fn version_three_failure_rolls_back_ddl_history_and_witness() {
    let db = Database::new();
    let c = Connection::open(&db.0).unwrap();
    initialize_v2(&c);
    // Force the migration to fail after both ALTERs have executed.
    c.execute_batch("CREATE TABLE traffic_entries (original TEXT); INSERT INTO traffic_entries VALUES ('preserve')").unwrap();
    let before = snapshot(&c);
    c.close().unwrap();
    assert!(open_and_migrate(&db.0).is_err());
    let c = Connection::open(&db.0).unwrap();
    assert_eq!(snapshot(&c), before);
    let columns: Vec<String> = c
        .prepare("PRAGMA table_info(permits)")
        .unwrap()
        .query_map([], |r| r.get(1))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(!columns.contains(&"cpu_millis_est".to_owned()));
    assert!(!columns.contains(&"generation".to_owned()));
}

#[test]
fn version_two_checksum_mismatch_never_applies_traffic_migration() {
    for id in ["0001_phase0", "0002_managed_exec"] {
        let db = Database::new();
        let c = Connection::open(&db.0).unwrap();
        initialize_v2(&c);
        c.execute(
            "UPDATE schema_migrations SET checksum='invalid' WHERE id=?1",
            [id],
        )
        .unwrap();
        let before = snapshot(&c);
        c.close().unwrap();
        assert!(open_and_migrate(&db.0)
            .unwrap_err()
            .to_string()
            .contains("checksum"));
        assert_eq!(snapshot(&Connection::open(&db.0).unwrap()), before);
    }
}

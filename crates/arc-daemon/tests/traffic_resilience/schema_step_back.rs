//! Put a real daemon database into the shape of an earlier schema version.
//!
//! The later migrations are undone in reverse order, their history rows are
//! removed and the version witness is lowered, so the database is what a
//! daemon of that version left behind and the current daemon must apply the
//! later migrations again. Rows are kept; only the values of the removed
//! columns are lost, exactly as a database of that version never held them.
//! The daemon that owns the database must have exited.
use arc_storage::{assess_schema, SchemaStatus, MIGRATIONS};
use rusqlite::Connection;
use std::path::Path;

/// The earliest version `step_back` can restore.
pub const EARLIEST: u32 = 6;

/// How to undo one migration: its version and the statements removing
/// exactly what it added. Extend this list with every new migration.
const UNDO: &[(u32, &str)] = &[
    (
        9,
        "DROP TABLE resource_rollups;
         ALTER TABLE job_meter_summaries DROP COLUMN process_tree_json;
         DROP INDEX idx_resource_samples_job_time;
         DROP INDEX idx_resource_samples_time;
         ALTER TABLE resource_samples DROP COLUMN cpu_core_percent;
         ALTER TABLE jobs DROP COLUMN project_key;
         ALTER TABLE jobs DROP COLUMN hook_overhead_ms;
         ALTER TABLE jobs DROP COLUMN vendor;",
    ),
    (
        8,
        "DROP INDEX idx_job_lifecycle_event_key;
         ALTER TABLE job_lifecycle DROP COLUMN event_key;",
    ),
    (
        7,
        "DROP INDEX idx_traffic_project_grant;
         ALTER TABLE traffic_entries DROP COLUMN project_key;",
    ),
];

/// Tables with their columns, and indexes with theirs, by name.
fn shape(connection: &Connection) -> Vec<(String, String, String, Vec<String>)> {
    let objects: Vec<(String, String, String)> = connection
        .prepare(
            "SELECT type,name,tbl_name FROM sqlite_schema \
             WHERE name NOT LIKE 'sqlite_%' ORDER BY type,name",
        )
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    objects
        .into_iter()
        .map(|(kind, name, table)| {
            let pragma = if kind == "table" {
                "SELECT name||' '||type||' '||\"notnull\"||' '||pk FROM pragma_table_info(?1)"
            } else {
                "SELECT name FROM pragma_index_info(?1)"
            };
            let columns = connection
                .prepare(pragma)
                .unwrap()
                .query_map([&name], |row| row.get(0))
                .unwrap()
                .map(Result::unwrap)
                .collect();
            (kind, name, table, columns)
        })
        .collect()
}

/// Step `database` back from the current schema to `version`. Afterwards the
/// schema has the shape the first `version` migrations create, its history
/// names exactly those migrations and an upgrade is required.
pub fn step_back(database: &Path, version: u32) {
    let current = MIGRATIONS.last().unwrap().version;
    assert!(
        (EARLIEST..current).contains(&version),
        "cannot step back to version {version}"
    );
    assert_eq!(
        UNDO.first().map(|(newest, _)| *newest),
        Some(current),
        "schema_step_back.rs has no undo for the newest migration"
    );
    let connection = Connection::open(database).unwrap();
    let before = assess_schema(&connection).unwrap();
    assert_eq!(
        (before.status, before.applied_version),
        (SchemaStatus::Compatible, Some(current)),
        "step back starts from a current database"
    );
    for (undone, sql) in UNDO.iter().filter(|(undone, _)| *undone > version) {
        let id = MIGRATIONS[*undone as usize - 1].id;
        connection
            .execute_batch(&format!("BEGIN IMMEDIATE; {sql} COMMIT;"))
            .unwrap_or_else(|error| panic!("undo {id}: {error}"));
        assert_eq!(
            connection
                .execute("DELETE FROM schema_migrations WHERE id=?1", [id])
                .unwrap(),
            1,
            "{id} was not in the history"
        );
    }
    connection
        .pragma_update(None, "user_version", version)
        .unwrap();

    let reference = Connection::open_in_memory().unwrap();
    for migration in &MIGRATIONS[..version as usize] {
        reference.execute_batch(migration.sql).unwrap();
    }
    assert_eq!(
        shape(&connection),
        shape(&reference),
        "not the schema of version {version}"
    );
    let after = assess_schema(&connection).unwrap();
    assert_eq!(
        (after.status, after.applied_version, after.history_verified),
        (SchemaStatus::UpgradeRequired, Some(version), true)
    );
    assert_eq!(after.schema_migrations, i64::from(version));
    let integrity: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");
    connection.close().unwrap();
}

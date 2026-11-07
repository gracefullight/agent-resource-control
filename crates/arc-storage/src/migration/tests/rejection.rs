use super::*;

#[test]
fn checksum_defects_in_every_known_entry_leave_all_state_unchanged() {
    for version in 1..=2 {
        let database = Database::new();
        let mut connection = database.v1();
        migrate(&mut connection, FIXTURES).unwrap();
        connection
            .execute(
                "UPDATE schema_migrations SET checksum='altered' WHERE id=?1",
                [FIXTURES[version - 1].id],
            )
            .unwrap();
        reject_unchanged(&mut connection, FIXTURES, SchemaStatus::ChecksumMismatch);
    }
}

#[test]
fn missing_history_table_rows_gaps_and_trailing_entry_are_rejected() {
    for mutation in [
        "DROP TABLE schema_migrations",
        "DELETE FROM schema_migrations",
        "DELETE FROM schema_migrations WHERE id='0001_fixture'",
        "DELETE FROM schema_migrations WHERE id='0002_additive'",
    ] {
        let database = Database::new();
        let mut connection = database.v1();
        migrate(&mut connection, FIXTURES).unwrap();
        connection.execute_batch(mutation).unwrap();
        reject_unchanged(&mut connection, FIXTURES, SchemaStatus::MissingHistory);
    }
}

#[test]
fn empty_arc_objects_or_history_and_nonzero_witness_are_never_fresh() {
    for setup in [
        "CREATE TABLE vendors (id TEXT)",
        "CREATE TABLE Vendors (id TEXT)",
        "CREATE TABLE unrelated (id INTEGER); CREATE INDEX idx_gc_state ON unrelated(id)",
        "CREATE TABLE schema_migrations (id TEXT PRIMARY KEY, applied_at INTEGER NOT NULL, checksum TEXT NOT NULL)",
        "PRAGMA user_version=1",
    ] {
        let database = Database::new();
        let mut connection = database.connect();
        connection.execute_batch(setup).unwrap();
        reject_unchanged(&mut connection, FIXTURES, SchemaStatus::MissingHistory);
    }
}

#[test]
fn unsupported_newer_history_and_witness_and_downgrade_are_rejected() {
    let database = Database::new();
    let mut connection = database.v1();
    migrate(&mut connection, FIXTURES).unwrap();
    // This is an explicit refusal to open a newer DB; no DOWN SQL is executed.
    reject_unchanged(
        &mut connection,
        &FIXTURES[..1],
        SchemaStatus::UnsupportedNewer,
    );
    let report = assess_with_registry(&connection, &FIXTURES[..1]).unwrap();
    assert_eq!(report.applied_version, Some(2));
    connection
        .execute(
            "INSERT INTO schema_migrations VALUES ('0003_future', 9, 'unknown-to-this-binary')",
            [],
        )
        .unwrap();
    connection.pragma_update(None, "user_version", 3).unwrap();
    reject_unchanged(&mut connection, FIXTURES, SchemaStatus::UnsupportedNewer);
    connection
        .execute("DELETE FROM schema_migrations WHERE id='0003_future'", [])
        .unwrap();
    reject_unchanged(&mut connection, FIXTURES, SchemaStatus::UnsupportedNewer);
    assert_eq!(
        assess_with_registry(&connection, FIXTURES)
            .unwrap()
            .applied_version,
        Some(3)
    );
}

#[test]
fn invalid_history_ids_types_duplicates_and_witness_are_rejected() {
    for mutation in [
        "UPDATE schema_migrations SET id='0001_wrong' WHERE id='0001_fixture'",
        "UPDATE schema_migrations SET id='000x_bad' WHERE id='0001_fixture'",
        "UPDATE schema_migrations SET applied_at='not-an-integer'",
        "UPDATE schema_migrations SET checksum=X'0102'",
        "INSERT INTO schema_migrations VALUES ('0001_duplicate', 5, 'duplicate-version')",
        "PRAGMA user_version=0",
        "PRAGMA user_version=1",
        "PRAGMA user_version=-1",
    ] {
        let database = Database::new();
        let mut connection = database.v1();
        migrate(&mut connection, FIXTURES).unwrap();
        connection.execute_batch(mutation).unwrap();
        reject_unchanged(&mut connection, FIXTURES, SchemaStatus::InvalidHistory);
    }
}

#[test]
fn known_prefix_defect_takes_precedence_over_future_history() {
    let database = Database::new();
    let mut connection = database.v1();
    connection.execute_batch("UPDATE schema_migrations SET checksum='altered'; INSERT INTO schema_migrations VALUES ('0002_future', 4, 'future'); PRAGMA user_version=2;").unwrap();
    reject_unchanged(
        &mut connection,
        &FIXTURES[..1],
        SchemaStatus::ChecksumMismatch,
    );
}

#[test]
fn numeric_version_order_ignores_history_insertion_and_timestamp_order() {
    let database = Database::new();
    let mut connection = database.v1();
    migrate(&mut connection, FIXTURES).unwrap();
    let stored = history(&connection);
    connection
        .execute("DELETE FROM schema_migrations", [])
        .unwrap();
    for (index, entry) in stored.iter().enumerate().rev() {
        connection
            .execute(
                "INSERT INTO schema_migrations VALUES (?1, ?2, ?3)",
                params![entry.0, 10 - index as i64, entry.2],
            )
            .unwrap();
    }
    let before = snapshot(&connection);
    migrate(&mut connection, FIXTURES).unwrap();
    assert_eq!(snapshot(&connection), before);
}

#[test]
fn unreadable_layout_stays_a_typed_sql_error() {
    let database = Database::new();
    let mut connection = database.connect();
    connection
        .execute("CREATE TABLE schema_migrations (unknown_column TEXT)", [])
        .unwrap();
    let before = snapshot(&connection);
    assert!(matches!(
        assess_with_registry(&connection, FIXTURES),
        Err(MigrationError::Sql(_))
    ));
    assert!(matches!(
        migrate(&mut connection, FIXTURES),
        Err(MigrationError::Sql(_))
    ));
    assert_eq!(snapshot(&connection), before);
}

#[test]
fn assessment_reuses_existing_transaction_without_committing_it() {
    let database = Database::new();
    let mut connection = database.v1();
    let before = snapshot(&connection);
    let transaction = connection.transaction().unwrap();
    transaction
        .execute("UPDATE vendors SET display_name='uncommitted'", [])
        .unwrap();
    assert_eq!(
        assess_with_registry(&transaction, FIXTURES).unwrap().status,
        SchemaStatus::UpgradeRequired
    );
    assert!(!transaction.is_autocommit());
    transaction.rollback().unwrap();
    assert_eq!(snapshot(&connection), before);
}

use super::*;

#[test]
fn fixture_initialization_additive_upgrade_reopen_and_idempotency() {
    let database = Database::new();
    let mut connection = database.connect();
    let fresh = assess_with_registry(&connection, FIXTURES).unwrap();
    assert_eq!(fresh.status, SchemaStatus::UpgradeRequired);
    assert_eq!(fresh.applied_version, Some(0));
    assert_eq!(fresh.schema_migrations, 0);
    migrate(&mut connection, &FIXTURES[..1]).unwrap();
    connection
        .execute("INSERT INTO vendors VALUES ('kept', 'original')", [])
        .unwrap();
    connection.close().unwrap();
    let mut connection = database.connect();
    let v1 = snapshot(&connection);
    migrate(&mut connection, &FIXTURES[..1]).unwrap();
    assert_eq!(snapshot(&connection), v1, "existing-database reopen");
    let first_history = history(&connection);
    assert_eq!(first_history.len(), 1);
    assert_eq!(first_history[0].0, FIXTURES[0].id);
    assert!(first_history[0].1 > 0);
    assert_eq!(first_history[0].2, checksum(FIXTURES[0].sql));
    let pending = assess_with_registry(&connection, FIXTURES).unwrap();
    assert_eq!(pending.status, SchemaStatus::UpgradeRequired);
    assert_eq!(pending.applied_version, Some(1));
    migrate(&mut connection, FIXTURES).unwrap();
    let rows: (String, String, String) = connection
        .query_row("SELECT id, display_name, note FROM vendors", [], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .unwrap();
    assert_eq!(rows, ("kept".into(), "original".into(), "preserved".into()));
    let note: (String, String) = connection
        .query_row("SELECT vendor_id, note FROM migration_notes", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .unwrap();
    assert_eq!(note, ("kept".into(), "upgraded".into()));
    let upgraded_history = history(&connection);
    assert_eq!(upgraded_history[0], first_history[0]);
    assert_eq!(upgraded_history.len(), 2);
    assert_eq!(upgraded_history[1].0, FIXTURES[1].id);
    assert!(upgraded_history[1].1 > 0);
    assert_eq!(upgraded_history[1].2, checksum(FIXTURES[1].sql));
    let upgraded = snapshot(&connection);
    for _ in 0..3 {
        connection.close().unwrap();
        connection = database.connect();
        migrate(&mut connection, FIXTURES).unwrap();
        assert_eq!(snapshot(&connection), upgraded, "repeat-open idempotency");
        let assessment = assess_with_registry(&connection, FIXTURES).unwrap();
        assert_eq!(assessment.status, SchemaStatus::Compatible);
        assert_eq!(assessment.applied_version, Some(2));
        assert_eq!(assessment.supported_version, 2);
        assert_eq!(assessment.schema_migrations, 2);
        assert!(assessment.history_verified);
    }
}

#[test]
fn production_legacy_history_is_read_only_then_stamped_atomically() {
    let database = Database::new();
    let mut connection = database.connect();
    migrate(&mut connection, &MIGRATIONS[..1]).unwrap();
    connection
        .execute(
            "INSERT INTO vendors VALUES ('kept', 'original', NULL, 17)",
            [],
        )
        .unwrap();
    let original_history = history(&connection);
    connection.pragma_update(None, "user_version", 0).unwrap();
    let legacy = snapshot(&connection);
    let report = assess_schema(&connection).unwrap();
    assert_eq!(report.status, SchemaStatus::UpgradeRequired);
    assert_eq!(report.applied_version, Some(1));
    assert_eq!(report.supported_version, 2);
    assert_eq!(report.schema_migrations, 1);
    assert!(report.history_verified);
    assert_eq!(snapshot(&connection), legacy);
    migrate(&mut connection, MIGRATIONS).unwrap();
    let stamped = snapshot(&connection);
    assert_eq!(stamped.tables.len(), legacy.tables.len() + 1);
    for (name, rows) in &legacy.tables {
        let updated = &stamped.tables.iter().find(|(n, _)| n == name).unwrap().1;
        if name == "schema_migrations" {
            assert_eq!(updated.len(), 2);
            assert_eq!(updated.first(), rows.first());
        } else {
            assert_eq!(updated, rows);
        }
    }
    assert_eq!(stamped.schema.len(), legacy.schema.len() + 3);
    for object in &legacy.schema {
        assert!(stamped.schema.contains(object));
    }
    assert_eq!(history(&connection)[0], original_history[0]);
    assert_eq!(stamped.witness, 2);
    connection.close().unwrap();
    assert_eq!(
        snapshot(&crate::open_and_migrate(&database.0).unwrap()),
        stamped
    );
}

#[test]
fn sql_failure_rolls_back_schema_application_data_history_and_witness() {
    let database = Database::new();
    let mut connection = database.v1();
    let before = snapshot(&connection);
    let broken = [FIXTURES[0], Migration {
        version: 2, id: "0002_failure",
        sql: "ALTER TABLE vendors ADD COLUMN pending TEXT; UPDATE vendors SET display_name='changed'; CREATE TABLE pending_data (id INTEGER); INSERT INTO absent_table VALUES (1);",
    }];
    let error = migrate(&mut connection, &broken).unwrap_err();
    assert!(matches!(error, MigrationError::Sql(_)));
    assert!(error.source().is_some());
    assert!(connection.is_autocommit());
    assert_eq!(snapshot(&connection), before);
    migrate(&mut connection, FIXTURES).unwrap();
    assert_eq!(history(&connection).len(), 2);
}

#[test]
fn initial_sql_failure_rolls_back_every_pending_object() {
    let database = Database::new();
    let mut connection = database.connect();
    let before = snapshot(&connection);
    let broken = [Migration { version: 1, id: "0001_failure", sql:
        "CREATE TABLE vendors (id TEXT); CREATE TABLE schema_migrations (id TEXT PRIMARY KEY, applied_at INTEGER NOT NULL, checksum TEXT NOT NULL); INSERT INTO absent_table VALUES (1);" }];
    assert!(matches!(
        migrate(&mut connection, &broken),
        Err(MigrationError::Sql(_))
    ));
    assert_eq!(snapshot(&connection), before);
}

#[test]
fn malformed_registry_is_rejected_before_any_sql() {
    let database = Database::new();
    let mut connection = database.connect();
    let before = snapshot(&connection);
    let cases: Vec<Vec<Migration>> = vec![
        vec![],
        vec![FIXTURES[1]],
        vec![FIXTURES[1], FIXTURES[0]],
        vec![FIXTURES[0], FIXTURES[0]],
        vec![Migration {
            version: 1,
            id: "0002_wrong",
            ..FIXTURES[0]
        }],
        vec![Migration {
            id: "0001_",
            ..FIXTURES[0]
        }],
        vec![Migration {
            id: "0001_Upper",
            ..FIXTURES[0]
        }],
        vec![Migration {
            id: "1_short",
            ..FIXTURES[0]
        }],
        vec![Migration {
            id: "00001_padding",
            ..FIXTURES[0]
        }],
        vec![Migration {
            id: "",
            ..FIXTURES[0]
        }],
        vec![Migration {
            sql: " \n",
            ..FIXTURES[0]
        }],
        vec![Migration {
            version: u32::MAX,
            id: "4294967295_overflow",
            ..FIXTURES[0]
        }],
    ];
    for registry in cases {
        assert!(matches!(
            migrate(&mut connection, &registry),
            Err(MigrationError::InvalidRegistry(_))
        ));
        assert_eq!(snapshot(&connection), before);
    }
}

#[test]
fn later_failure_rolls_back_prior_migration_history_in_same_batch() {
    let database = Database::new();
    let mut connection = database.connect();
    let before = snapshot(&connection);
    let broken = [
        FIXTURES[0],
        Migration {
            version: 2,
            id: "0002_failure",
            sql:
                "ALTER TABLE vendors ADD COLUMN pending TEXT; INSERT INTO absent_table VALUES (1);",
        },
    ];
    assert!(matches!(
        migrate(&mut connection, &broken),
        Err(MigrationError::Sql(_))
    ));
    assert_eq!(snapshot(&connection), before);
    migrate(&mut connection, FIXTURES).unwrap();
    assert_eq!(history(&connection).len(), 2);
}

#[test]
fn fixed_schema1_registry_stamps_legacy_witness_without_upgrading() {
    let database = Database::new();
    let mut connection = database.connect();
    migrate(&mut connection, &MIGRATIONS[..1]).unwrap();
    connection
        .execute(
            "INSERT INTO vendors VALUES ('kept', 'original', NULL, 17)",
            [],
        )
        .unwrap();
    connection.pragma_update(None, "user_version", 0).unwrap();
    let legacy = snapshot(&connection);
    let report = assess_with_registry(&connection, &MIGRATIONS[..1]).unwrap();
    assert_eq!(report.status, SchemaStatus::Compatible);
    assert_eq!(report.applied_version, Some(1));
    assert_eq!(report.supported_version, 1);
    assert_eq!(report.schema_migrations, 1);
    assert!(report.history_verified);
    assert_eq!(snapshot(&connection), legacy);
    migrate(&mut connection, &MIGRATIONS[..1]).unwrap();
    let stamped = snapshot(&connection);
    assert_eq!(stamped.tables, legacy.tables);
    assert_eq!(stamped.schema, legacy.schema);
    assert_eq!(stamped.witness, 1);
    connection.close().unwrap();
    let mut connection = database.connect();
    migrate(&mut connection, &MIGRATIONS[..1]).unwrap();
    assert_eq!(snapshot(&connection), stamped);
}

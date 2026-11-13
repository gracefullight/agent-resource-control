//! Task-1 migration contracts; task 2 adds transactional pressure acceptance.
use arc_storage::{assess_schema, MIGRATIONS};
use rusqlite::Connection;
use sha2::{Digest, Sha256};

#[test]
fn all_four_accepted_migration_bytes_retain_their_sha256() {
    let expected = [
        "046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384",
        "464a1c88270f01a56ccab6610f87eff7648e0640f276ebce53b8a97026c50814",
        "1f4522e3f27c124aea7127d92434c7bd22be36168fef0a4bf665f8cf16debe90",
        "e4c40db876032d858667e2af5d6be524c2f0bd204a7c6b97ba878e082496c540",
    ];
    for (migration, expected) in MIGRATIONS.iter().take(4).zip(expected) {
        assert_eq!(
            Sha256::digest(migration.sql.as_bytes())
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
            expected
        );
    }
}

#[test]
fn additive_pressure_migration_preserves_legacy_rows_and_bounds_new_metadata() {
    let connection = Connection::open_in_memory().unwrap();
    for migration in &MIGRATIONS[..4] {
        connection.execute_batch(migration.sql).unwrap();
    }
    connection
        .execute("INSERT INTO policy_states VALUES ('kept',1,7,42)", [])
        .unwrap();
    connection
        .execute(
            "INSERT INTO policy_decisions VALUES ('old',NULL,'legacy','WAIT','kept',NULL,42)",
            [],
        )
        .unwrap();
    assert_eq!(MIGRATIONS[4].id, "0005_memory_pressure");
    assert_eq!(MIGRATIONS[4].version, 5);
    connection.execute_batch(MIGRATIONS[4].sql).unwrap();
    let original: (String, String, i64, String) = connection.query_row(
        "SELECT policy_version,reason_code,created_at,observation_availability FROM policy_decisions WHERE id='old'",
        [], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?))).unwrap();
    assert_eq!(
        original,
        ("legacy".into(), "kept".into(), 42, "missing".into())
    );
    let state: (i64, i64, i64) = connection
        .query_row(
            "SELECT enabled,policy_revision,updated_at FROM policy_states WHERE key='kept'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(state, (1, 7, 42));
    for invalid in ["not-a-reading", "normal", ""] {
        assert!(connection
            .execute(
                "UPDATE policy_decisions SET observation_availability=?1 WHERE id='old'",
                [invalid]
            )
            .is_err());
    }
    assert!(connection
        .execute(
            "UPDATE policy_decisions SET observation_age_ms=-1 WHERE id='old'",
            []
        )
        .is_err());
    assert!(connection
        .execute(
            "UPDATE policy_decisions SET observation_json=?1 WHERE id='old'",
            ["x".repeat(8193)]
        )
        .is_err());
    assert!(connection
        .execute(
            "UPDATE policy_states SET state_json=?1 WHERE key='kept'",
            ["x".repeat(16385)]
        )
        .is_err());
    for index in [
        "idx_policy_decisions_job_time",
        "idx_policy_decisions_expiry",
    ] {
        assert!(connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='index' AND name=?1)",
                [index],
                |row| row.get::<_, bool>(0)
            )
            .unwrap());
    }
}
#[test]
fn registered_pressure_schema_is_current_after_actual_local_migration_and_reopen() {
    let directory = std::env::temp_dir().join(format!(
        "arc-pressure-schema-{}-{}",
        std::process::id(),
        arc_core::process_identity::random_job_id().unwrap()
    ));
    let path = directory.join("db.sqlite3");
    let connection = arc_storage::open_and_migrate(&path).unwrap();
    let schema = assess_schema(&connection).unwrap();
    assert_eq!(schema.applied_version, Some(6));
    assert_eq!(schema.supported_version, 6);
    assert_eq!(schema.schema_migrations, 6);
    assert!(schema.history_verified);
    connection.close().unwrap();
    let reopened = arc_storage::open_and_migrate(&path).unwrap();
    assert_eq!(assess_schema(&reopened).unwrap(), schema);
    reopened.close().unwrap();
    std::fs::remove_dir_all(directory).unwrap();
}

#[path = "memory_pressure/admission.rs"]
mod admission;
#[path = "memory_pressure/backlog.rs"]
mod backlog;
#[path = "memory_pressure/clock_recovery.rs"]
mod clock_recovery;
#[path = "../../arc-policy/tests/support/memory_observation.rs"]
mod observation;
#[path = "memory_pressure/retention.rs"]
mod retention;
#[path = "memory_pressure/running_launch.rs"]
mod running_launch;

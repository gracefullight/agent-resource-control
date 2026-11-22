//! Seam for vendor hook event identity (AT-TRF-09, ADR-014): the version-8
//! migration and the constraints of the stored key. Recording a key at
//! admission and answering a duplicate delivery are not covered here.
use super::*;
use crate::{assess_schema, migration::migrate, MIGRATIONS};

const KEY: &str = "00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff";

/// One registered job as raw rows, valid on every schema from version 2.
fn register(c: &Connection, n: u32) {
    c.execute(
        "INSERT INTO jobs (id,job_class,locality,enforcement,state,created_at) VALUES (?1,'BUILD','HOST_LOCAL','SCHEDULED','NEW',5)",
        [id(n).as_str()],
    )
    .unwrap();
    c.execute(
        "INSERT INTO job_lifecycle (job_id,registered_at_ms,observer_pid,observer_start_token,observer_boot_id,
         observer_verified_at_ms,classification_reason,classification_uncertainty,observation_target,
         observation_status,observation_at_ms,observation_reason,revision,updated_at_ms,reason)
         VALUES (?1,5,?2,'1','01234567-89ab-cdef-0123-456789abcdef',5,'CARGO_BUILD','NONE','OBSERVER',
         'VERIFIED_MATCH',5,'REGISTERED',1,5,'REGISTERED')",
        (id(n).as_str(), 100 + i64::from(n)),
    )
    .unwrap();
}

fn set_key(c: &Connection, n: u32, key: Option<&str>) -> rusqlite::Result<usize> {
    c.execute(
        "UPDATE job_lifecycle SET event_key=?2 WHERE job_id=?1",
        (id(n).as_str(), key),
    )
}

fn keys(c: &Connection) -> Vec<(String, Option<String>)> {
    c.prepare("SELECT job_id,event_key FROM job_lifecycle ORDER BY job_id")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

#[test]
fn migration_8_is_expand_only() {
    let migration = MIGRATIONS[7];
    assert_eq!((migration.version, migration.id), (8, "0008_hook_event"));
    let statements: Vec<String> = migration
        .sql
        .lines()
        .filter(|line| !line.trim_start().starts_with("--"))
        .collect::<Vec<_>>()
        .join(" ")
        .split(';')
        .map(|statement| statement.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|statement| !statement.is_empty())
        .collect();
    assert_eq!(statements.len(), 2, "{statements:?}");
    assert!(
        statements[0].starts_with("ALTER TABLE job_lifecycle ADD COLUMN event_key TEXT CHECK("),
        "{statements:?}"
    );
    assert!(
        statements[1].starts_with(
            "CREATE UNIQUE INDEX idx_job_lifecycle_event_key ON job_lifecycle(event_key) WHERE"
        ),
        "{statements:?}"
    );
}

#[test]
fn version_7_database_migrates_to_event_key_with_rows_intact() {
    let db = Database::new();
    let mut c = crate::open_connection(&db.0).unwrap();
    migrate(&mut c, &MIGRATIONS[..7]).unwrap();
    assert_eq!(assess_schema(&c).unwrap().applied_version, Some(7));
    for n in 1..=2u32 {
        register(&c, n);
    }
    let rows = |c: &Connection| -> Vec<(String, i64, i64, String)> {
        c.prepare(
            "SELECT job_id,registered_at_ms,observer_pid,reason FROM job_lifecycle ORDER BY job_id",
        )
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect()
    };
    let history = |c: &Connection| -> Vec<(String, i64, String)> {
        c.prepare("SELECT id,applied_at,checksum FROM schema_migrations ORDER BY id")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    };
    let (before_rows, before_history) = (rows(&c), history(&c));
    assert_eq!(before_rows.len(), 2);
    drop(c);

    let c = db.open();
    let schema = assess_schema(&c).unwrap();
    assert_eq!(schema.applied_version, Some(9));
    assert!(schema.history_verified);
    assert_eq!(rows(&c), before_rows);
    let after_history = history(&c);
    assert_eq!(after_history[..7], before_history[..]);
    assert_eq!(after_history[7].0, "0008_hook_event");
    // No backfill: the event of a job registered before the upgrade is unknown.
    assert_eq!(
        keys(&c),
        [
            (id(1).as_str().to_owned(), None),
            (id(2).as_str().to_owned(), None)
        ]
    );
    let integrity: String = c
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");
    // Reopening applies nothing further.
    drop(c);
    assert_eq!(history(&db.open()), after_history);
}

#[test]
fn stored_key_is_64_lowercase_hex_and_names_at_most_one_job() {
    let db = Database::new();
    let c = db.open();
    for n in 1..=3u32 {
        register(&c, n);
    }
    for invalid in [
        "",
        "session-1:toolu_01",
        &KEY[..63],
        &format!("{KEY}0"),
        &KEY.to_uppercase(),
    ] {
        assert!(set_key(&c, 1, Some(invalid)).is_err(), "{invalid}");
    }
    assert_eq!(set_key(&c, 1, Some(KEY)), Ok(1));
    // A second job cannot take a recorded key; any number may have none.
    assert!(set_key(&c, 2, Some(KEY)).is_err());
    assert_eq!(
        keys(&c),
        [
            (id(1).as_str().to_owned(), Some(KEY.to_owned())),
            (id(2).as_str().to_owned(), None),
            (id(3).as_str().to_owned(), None)
        ]
    );
    // The key is deleted with its job row and is then free again.
    c.execute(
        "DELETE FROM job_lifecycle WHERE job_id=?1",
        [id(1).as_str()],
    )
    .unwrap();
    assert_eq!(set_key(&c, 2, Some(KEY)), Ok(1));
}

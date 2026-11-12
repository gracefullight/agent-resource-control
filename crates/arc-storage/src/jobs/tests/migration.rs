use super::{claimed, get, id, Database};
use crate::{assess_schema, migration::migrate, SchemaStatus, MIGRATIONS};

#[test]
fn upgrade_from_original_registry_preserves_rows_checksums_and_reopen() {
    let db = Database::new();
    let mut c = crate::open_connection(&db.0).unwrap();
    migrate(&mut c, &MIGRATIONS[..1]).unwrap();
    c.execute("INSERT INTO jobs (id,job_class,locality,enforcement,state,created_at) VALUES ('legacy','TEST','HOST_LOCAL','OBSERVE','NEW',5)", []).unwrap();
    let old: (String, i64, String) = c
        .query_row(
            "SELECT id,applied_at,checksum FROM schema_migrations",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(
        assess_schema(&c).unwrap().status,
        SchemaStatus::UpgradeRequired
    );
    drop(c);
    let mut c = db.open();
    assert_eq!(assess_schema(&c).unwrap().applied_version, Some(5));
    let retained: (String, i64, String) = c
        .query_row(
            "SELECT id,applied_at,checksum FROM schema_migrations WHERE id='0001_phase0'",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .unwrap();
    assert_eq!(retained, old);
    assert_eq!(
        c.query_row("SELECT state FROM jobs WHERE id='legacy'", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        "NEW"
    );
    let j = claimed(&mut c, 1);
    drop(c);
    let c = db.open();
    assert_eq!(get(&c, &id(1)).unwrap(), Some(j));
    assert_eq!(
        c.query_row("SELECT COUNT(*) FROM schema_migrations", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        5
    );
}
#[test]
fn additive_schema_checks_reject_partial_identity_and_exclusive_outcome_violation() {
    let db = Database::new();
    let mut c = db.open();
    claimed(&mut c, 1);
    for sql in [
        "UPDATE job_lifecycle SET child_pid=1",
        "UPDATE job_lifecycle SET signal=15",
        "UPDATE job_lifecycle SET outcome_kind='SIGNALED'",
        "UPDATE job_lifecycle SET observer_start_token='01'",
        "UPDATE job_lifecycle SET observer_start_token='18446744073709551616'",
        "UPDATE job_lifecycle SET revision=0",
    ] {
        assert!(c.execute(sql, []).is_err(), "{sql}");
    }
}

#[test]
fn existing_legacy_job_key_cannot_be_overwritten_or_attached() {
    let db = Database::new();
    let mut c = db.open();
    c.execute("INSERT INTO jobs (id,job_class,locality,enforcement,state,created_at) VALUES (?1,'UNKNOWN','UNKNOWN','OBSERVE','NEW',5)", [id(1).as_str()]).unwrap();
    let request = super::register(1);
    let error = super::start(
        &mut c,
        &request,
        &super::StartEvidence::gather(&request),
        100,
    )
    .unwrap_err();
    assert_eq!(error, super::JobError::Conflict);
    assert!(get(&c, &id(1)).unwrap().is_none());
    assert_eq!(
        c.query_row("SELECT state FROM jobs", [], |row| row.get::<_, String>(0))
            .unwrap(),
        "NEW"
    );
}

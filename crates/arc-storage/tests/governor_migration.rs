//! Additive schema acceptance. Runtime setter/launch behavior is not exercised here.
use arc_storage::{assess_schema, open_and_migrate, MIGRATIONS};
use rusqlite::Connection;
use sha2::{Digest, Sha256};

fn checksum(sql: &str) -> String {
    Sha256::digest(sql.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn v5(c: &Connection) {
    c.execute_batch("PRAGMA foreign_keys=ON;").unwrap();
    for m in &MIGRATIONS[..5] {
        c.execute_batch(m.sql).unwrap();
        c.execute(
            "INSERT INTO schema_migrations VALUES (?1,42,?2)",
            (m.id, checksum(m.sql)),
        )
        .unwrap();
    }
    c.pragma_update(None, "user_version", 5).unwrap();
    c.execute("INSERT INTO policy_states(key,enabled,policy_revision,updated_at) VALUES ('memory_admission',1,7,42)",[]).unwrap();
}
fn job(c: &Connection) {
    c.execute_batch("INSERT INTO jobs(id,job_class,locality,enforcement,state,created_at) VALUES ('job','BUILD','HOST_LOCAL','ENFORCE','NEW',1);
      INSERT INTO job_lifecycle(job_id,registered_at_ms,observer_pid,observer_start_token,observer_boot_id,observer_verified_at_ms,classification_reason,classification_uncertainty,observation_target,observation_status,observation_at_ms,observation_reason,revision,updated_at_ms,reason)
      VALUES ('job',1,42,'123','aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee',1,'CARGO_BUILD','NONE','OBSERVER','VERIFIED_MATCH',1,'NONE',1,1,'NONE');").unwrap();
}

#[test]
fn original_five_migration_bytes_are_immutable() {
    let hashes = [
        "046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384",
        "464a1c88270f01a56ccab6610f87eff7648e0640f276ebce53b8a97026c50814",
        "1f4522e3f27c124aea7127d92434c7bd22be36168fef0a4bf665f8cf16debe90",
        "e4c40db876032d858667e2af5d6be524c2f0bd204a7c6b97ba878e082496c540",
        "6396adb0918a1bacfea16945caa866b12e91d322454ec64a00d316b0bccf0326",
    ];
    assert_eq!(MIGRATIONS.len(), 6);
    for (m, hash) in MIGRATIONS[..5].iter().zip(hashes) {
        assert_eq!(checksum(m.sql), hash);
    }
}

#[test]
fn authoritative_read_defaults_only_when_absent_and_rejects_corrupt_state() {
    let c = Connection::open_in_memory().unwrap();
    v5(&c);
    assert_eq!(
        arc_storage::governor::get_switch(&c).unwrap(),
        arc_ipc::governor::GovernorSwitchState::default()
    );
    c.execute("INSERT INTO policy_states(key,enabled,policy_revision,updated_at) VALUES ('governor.enabled',0,1,42)",[]).unwrap();
    let state = arc_storage::governor::get_switch(&c).unwrap();
    assert!(!state.enabled);
    assert_eq!(state.revision, 1);
    assert_eq!(state.updated_at_ms, Some(42));
    for sql in [
        "UPDATE policy_states SET enabled=2 WHERE key='governor.enabled'",
        "UPDATE policy_states SET enabled=0,policy_revision=-1 WHERE key='governor.enabled'",
        "UPDATE policy_states SET policy_revision=9007199254740992 WHERE key='governor.enabled'",
        "UPDATE policy_states SET policy_revision=1,updated_at=-1 WHERE key='governor.enabled'",
    ] {
        c.execute(sql, []).unwrap();
        assert!(arc_storage::governor::get_switch(&c).is_err());
    }
}

#[test]
fn governor_schema_expands_v5_preserving_rows_and_rejects_invalid_facts() {
    let c = Connection::open_in_memory().unwrap();
    v5(&c);
    job(&c);
    c.execute_batch(MIGRATIONS[5].sql).unwrap();
    assert_eq!(
        c.query_row(
            "SELECT policy_revision FROM policy_states WHERE key='memory_admission'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        7
    );
    assert_eq!(
        c.query_row(
            "SELECT COUNT(*) FROM policy_states WHERE key='governor.enabled'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    c.execute(
        "INSERT INTO governor_jobs(job_id,mode,switch_revision) VALUES ('job','SCHEDULED',0)",
        [],
    )
    .unwrap();
    assert!(c.execute("INSERT INTO governor_jobs(job_id,mode,switch_revision) VALUES ('absent','SCHEDULED',0)",[]).is_err());
    for sql in [
        "UPDATE governor_jobs SET mode='RUNNING'",
        "UPDATE governor_jobs SET switch_revision=-1",
        "UPDATE governor_jobs SET switch_revision=9007199254740992",
        "UPDATE governor_jobs SET switch_revision=1.5",
        "UPDATE governor_jobs SET unreserved_at_ms=2",
        "UPDATE governor_jobs SET mode='UNRESERVED'",
        "UPDATE governor_jobs SET release_id='aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'",
    ] {
        assert!(c.execute(sql, []).is_err(), "{sql}");
    }
    c.execute("UPDATE governor_jobs SET mode='UNRESERVED',unreserved_at_ms=2,switch_revision=1,previous_generation=7",[]).unwrap();
    for sql in [
        "UPDATE governor_jobs SET release_id='invalid',release_claimed_at_ms=3",
        "UPDATE governor_jobs SET release_id='aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',release_claimed_at_ms=1",
        "UPDATE governor_jobs SET previous_generation=0",
        "UPDATE governor_jobs SET mode='SCHEDULED'",
    ] { assert!(c.execute(sql,[]).is_err(),"{sql}"); }
    c.execute("UPDATE governor_jobs SET release_id='aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',release_claimed_at_ms=3",[]).unwrap();
    let receipt = "INSERT INTO governor_mutation_receipts(mutation_id,expected_revision,desired_enabled,result_enabled,result_revision,result_updated_at_ms,changed,committed_at_ms) VALUES (?1,0,0,0,1,2,1,2)";
    c.execute(receipt, ["0-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"])
        .unwrap();
    for id in [
        "0-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "00-bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "1-bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
        "0-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        "0-bad",
    ] {
        assert!(c.execute(receipt, [id]).is_err(), "{id}");
    }
    for sql in [
        "UPDATE governor_mutation_receipts SET expected_revision=9007199254740992",
        "UPDATE governor_mutation_receipts SET result_revision=2",
        "UPDATE governor_mutation_receipts SET result_updated_at_ms=NULL",
        "UPDATE governor_mutation_receipts SET result_enabled=1",
        "UPDATE governor_mutation_receipts SET committed_at_ms=1.5",
    ] {
        assert!(c.execute(sql, []).is_err(), "{sql}");
    }
}

#[test]
fn production_migrator_commits_v6_atomically_and_reopens_without_rewriting_history() {
    let dir = std::env::temp_dir().join(format!(
        "arc-governor-schema-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("db.sqlite3");
    let c = Connection::open(&path).unwrap();
    v5(&c);
    job(&c);
    c.close().unwrap();
    let c = open_and_migrate(&path).unwrap();
    assert_eq!(assess_schema(&c).unwrap().applied_version, Some(6));
    assert!(assess_schema(&c).unwrap().history_verified);
    for m in &MIGRATIONS[..5] {
        let row: (i64, String) = c
            .query_row(
                "SELECT applied_at,checksum FROM schema_migrations WHERE id=?1",
                [m.id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(row, (42, checksum(m.sql)));
    }
    c.execute(
        "INSERT INTO governor_jobs(job_id,mode,switch_revision) VALUES ('job','SCHEDULED',0)",
        [],
    )
    .unwrap();
    let schema = assess_schema(&c).unwrap();
    c.close().unwrap();
    let c = open_and_migrate(&path).unwrap();
    assert_eq!(assess_schema(&c).unwrap(), schema);
    assert_eq!(
        c.query_row("SELECT COUNT(*) FROM governor_jobs", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    c.close().unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn failed_governor_upgrade_rolls_back_new_table_and_preserves_v5_history() {
    let dir = std::env::temp_dir().join(format!(
        "arc-governor-rollback-{}",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("db.sqlite3");
    let c = Connection::open(&path).unwrap();
    v5(&c);
    c.execute_batch(
        "CREATE TABLE governor_jobs(original TEXT); INSERT INTO governor_jobs VALUES ('kept');",
    )
    .unwrap();
    c.close().unwrap();
    assert!(open_and_migrate(&path).is_err());
    let c = Connection::open(&path).unwrap();
    assert_eq!(
        c.query_row("PRAGMA user_version", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        5
    );
    assert_eq!(
        c.query_row("SELECT COUNT(*) FROM schema_migrations", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        5
    );
    assert_eq!(
        c.query_row("SELECT original FROM governor_jobs", [], |r| r
            .get::<_, String>(0))
            .unwrap(),
        "kept"
    );
    assert_eq!(
        c.query_row(
            "SELECT COUNT(*) FROM sqlite_schema WHERE name='governor_mutation_receipts'",
            [],
            |r| r.get::<_, i64>(0)
        )
        .unwrap(),
        0
    );
    c.close().unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}

use super::*;
use crate::{
    migration::{checksum, migrate},
    MIGRATIONS,
};

fn history(c: &Connection) -> Vec<(String, i64, String)> {
    c.prepare("SELECT id,applied_at,checksum FROM schema_migrations ORDER BY id")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}
#[test]
fn three_migration_bytes_and_checksums_are_preserved_on_upgrade() {
    let expected = [
        "046610c920709f0d9371ccaff6254fbd1c1c46cc6e930890869ff0ffc4fb1384",
        "464a1c88270f01a56ccab6610f87eff7648e0640f276ebce53b8a97026c50814",
        "1f4522e3f27c124aea7127d92434c7bd22be36168fef0a4bf665f8cf16debe90",
    ];
    for (migration, expected) in MIGRATIONS.iter().zip(expected) {
        assert_eq!(checksum(migration.sql), expected);
    }
    let mut c = Connection::open_in_memory().unwrap();
    c.execute_batch("PRAGMA foreign_keys=ON").unwrap();
    migrate(&mut c, &MIGRATIONS[..3]).unwrap();
    let old = history(&c);
    let r = fixture(1);
    c.execute(
        "INSERT INTO jobs(id,job_class,locality,enforcement,state,created_at,finished_at,exit_code)
        VALUES(?1,'TEST','HOST_LOCAL','OBSERVE','COMPLETED',100,200,0)",
        [&r.job_id],
    )
    .unwrap();
    c.execute("INSERT INTO vendors VALUES('kept','original',NULL,17)", [])
        .unwrap();
    migrate(&mut c, MIGRATIONS).unwrap();
    assert_eq!(&history(&c)[..3], old.as_slice());
    assert_eq!(history(&c).len(), 6);
    assert_eq!(crate::assess_schema(&c).unwrap().supported_version, 6);
    unknown(&mut c, &r, UnknownReason::Legacy);
    assert_eq!(binding(&c, &r.job_id).unwrap().unwrap().terminal, Some(200));
    assert_eq!(
        summary(&mut c, &r.job_id, 300).unwrap().queue_wait,
        QueueWait::unknown()
    );
    assert_eq!(
        c.query_row(
            "SELECT display_name FROM vendors WHERE id='kept'",
            [],
            |r| r.get::<_, String>(0)
        )
        .unwrap(),
        "original"
    );
    let upgraded = history(&c);
    migrate(&mut c, MIGRATIONS).unwrap();
    assert_eq!(history(&c), upgraded);
    // The protected migration assessor still rejects altered historical bytes.
    c.execute(
        "UPDATE schema_migrations SET checksum='bad' WHERE id='0002_managed_exec'",
        [],
    )
    .unwrap();
    assert_eq!(
        crate::assess_schema(&c).unwrap().status,
        crate::SchemaStatus::ChecksumMismatch
    );
}

#[test]
fn committed_report_survives_file_reopen_and_lost_ack_retry_without_renewing_anchor() {
    let path = std::env::temp_dir().join(format!(
        "arc-meter-store-{}.sqlite3",
        arc_core::process_identity::random_job_id().unwrap()
    ));
    let mut c = crate::open_and_migrate(&path).unwrap();
    let r = seed(&c, 1);
    report(&mut c, &r, 300).unwrap();
    let accepted = payload(&c, &r.job_id);
    c.close().unwrap();
    let mut c = crate::open_and_migrate(&path).unwrap();
    assert_eq!(
        report(&mut c, &r, 500).unwrap().disposition,
        ReportDisposition::Unchanged
    );
    assert_eq!(payload(&c, &r.job_id), accepted);
    assert_eq!(binding(&c, &r.job_id).unwrap().unwrap().terminal, Some(200));
    assert_eq!(
        summary(&mut c, &r.job_id, 501).unwrap().accounting,
        SummaryAccounting::Recorded {
            report: Box::new(r)
        }
    );
    c.close().unwrap();
    std::fs::remove_file(path).unwrap();
}

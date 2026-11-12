use super::*;

#[test]
fn ninety_day_cutoff_equality_reads_and_late_delivery_never_resurrect() {
    let mut c = db();
    let r = seed(&c, 1);
    report(&mut c, &r, 300).unwrap();
    let cutoff = 200 + JOB_METADATA_RETENTION_MS;
    assert_eq!(expire(&mut c, cutoff - 1).unwrap(), 0);
    assert!(matches!(
        summary(&mut c, &r.job_id, cutoff - 1).unwrap().accounting,
        SummaryAccounting::Recorded { .. }
    ));
    let expired = summary(&mut c, &r.job_id, cutoff).unwrap();
    assert_eq!(expired.accounting, SummaryAccounting::Expired {});
    assert_eq!(expired.queue_wait, QueueWait::unknown());
    assert!(payload(&c, &r.job_id).is_none());
    assert_eq!(report(&mut c, &r, cutoff), Err(MeterError::Unavailable));
    assert_eq!(report(&mut c, &r, cutoff + 1), Err(MeterError::Unavailable));
    assert_eq!(expire(&mut c, cutoff + 1).unwrap(), 0);
    // Expiry proof remains durable even if a wall clock moves backwards.
    assert_eq!(
        summary(&mut c, &r.job_id, 300).unwrap().accounting,
        SummaryAccounting::Expired {}
    );
    let delayed = seed(&c, 2);
    assert_eq!(
        report(&mut c, &delayed, cutoff),
        Err(MeterError::Unavailable)
    );
    assert!(payload(&c, &delayed.job_id).is_none());
}

#[test]
fn cleanup_is_bounded_and_does_not_delete_lifecycle_or_root_dependencies() {
    let mut c = db();
    let cutoff = 200 + JOB_METADATA_RETENTION_MS;
    for n in 1..=129 {
        seed(&c, n);
    }
    c.execute(
        "UPDATE job_meter_summaries SET relation='included_in_root',root_job_id=?1 WHERE job_id=?2",
        params![fixture(1).job_id, fixture(2).job_id],
    )
    .unwrap();
    assert_eq!(expire(&mut c, cutoff).unwrap(), MAX_EXPIRY_BATCH);
    assert_eq!(expire(&mut c, cutoff).unwrap(), 1);
    assert_eq!(expire(&mut c, cutoff).unwrap(), 0);
    assert_eq!(
        c.query_row("SELECT count(*) FROM job_lifecycle", [], |r| r
            .get::<_, u32>(0))
            .unwrap(),
        129
    );
    assert_eq!(
        c.query_row("SELECT count(*) FROM jobs", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        129
    );
    assert_eq!(
        binding(&c, &fixture(2).job_id).unwrap().unwrap().root,
        Some(fixture(1).job_id)
    );
    assert!(c
        .execute("DELETE FROM jobs WHERE id=?1", [&fixture(1).job_id])
        .is_err());
    assert!(c
        .execute(
            "UPDATE job_meter_summaries SET root_job_id=?1 WHERE job_id=?2",
            params![fixture(999).job_id, fixture(2).job_id]
        )
        .is_err());
    assert!(c
        .query_row("PRAGMA foreign_key_check", [], |_| Ok(()))
        .optional()
        .unwrap()
        .is_none());
}

#[test]
fn active_job_permit_traffic_verified_execution_and_lease_protect_payloads() {
    let mut c = db();
    let cutoff = 200 + JOB_METADATA_RETENTION_MS;
    let r = seed(&c, 1);
    report(&mut c, &r, 300).unwrap();
    c.execute("UPDATE jobs SET state='RUNNING'", []).unwrap();
    assert_eq!(expire(&mut c, cutoff).unwrap(), 0);
    assert!(matches!(
        summary(&mut c, &r.job_id, cutoff).unwrap().accounting,
        SummaryAccounting::Recorded { .. }
    ));
    c.execute("UPDATE jobs SET state='OUTCOME_UNKNOWN'", [])
        .unwrap();
    assert_eq!(expire(&mut c, cutoff).unwrap(), 0);
    c.execute("UPDATE jobs SET state='SUCCEEDED'", []).unwrap();
    c.execute(
        "INSERT INTO permits(id,job_id,state,reserved_at) VALUES('p',?1,'ACTIVE',100)",
        [&r.job_id],
    )
    .unwrap();
    assert_eq!(expire(&mut c, cutoff).unwrap(), 0);
    c.execute("UPDATE permits SET state='RELEASED'", [])
        .unwrap();
    c.execute(
        "INSERT INTO traffic_entries(job_id,state,queued_at_ms) VALUES(?1,'QUEUED',100)",
        [&r.job_id],
    )
    .unwrap();
    assert_eq!(expire(&mut c, cutoff).unwrap(), 0);
    c.execute("UPDATE traffic_entries SET state='COMPLETED'", [])
        .unwrap();
    c.execute_batch("INSERT INTO vendors VALUES('v','v',NULL,100);
        INSERT INTO agent_instances(id,vendor_id,state,created_at) VALUES('i','v','ACTIVE',100);
        INSERT INTO agent_sessions(id,instance_id,started_at,state) VALUES('s','i',100,'ACTIVE');
        INSERT INTO resource_objects(id,resource_type,resource_identity,ownership_level,state)
          VALUES('o','temp','o','owned','ACTIVE');
        INSERT INTO resource_leases(id,resource_id,session_id,acquired_at,state) VALUES('l','o','s',100,'ACTIVE');
        UPDATE jobs SET session_id='s';").unwrap();
    assert_eq!(expire(&mut c, cutoff).unwrap(), 0);
    c.execute("UPDATE resource_leases SET state='RELEASED'", [])
        .unwrap();
    assert_eq!(expire(&mut c, cutoff).unwrap(), 1);
    assert!(payload(&c, &r.job_id).is_none());
}

#[test]
fn active_descendant_protects_root_and_root_protects_finished_nested_record() {
    let mut c = db();
    let root = seed(&c, 1);
    let nested = seed(&c, 2);
    c.execute(
        "UPDATE job_meter_summaries SET relation='included_in_root',root_job_id=?1 WHERE job_id=?2",
        params![root.job_id, nested.job_id],
    )
    .unwrap();
    c.execute(
        "UPDATE jobs SET state='RUNNING' WHERE id=?1",
        [&nested.job_id],
    )
    .unwrap();
    assert_eq!(expire(&mut c, 200 + JOB_METADATA_RETENTION_MS).unwrap(), 0);
    c.execute(
        "UPDATE jobs SET state='SUCCEEDED' WHERE id=?1",
        [&nested.job_id],
    )
    .unwrap();
    c.execute(
        "UPDATE jobs SET state='RUNNING' WHERE id=?1",
        [&root.job_id],
    )
    .unwrap();
    assert_eq!(expire(&mut c, 200 + JOB_METADATA_RETENTION_MS).unwrap(), 0);
    c.execute("UPDATE jobs SET state='SUCCEEDED'", []).unwrap();
    assert_eq!(expire(&mut c, 200 + JOB_METADATA_RETENTION_MS).unwrap(), 2);
}

#[test]
fn first_interruption_anchor_survives_recovery_and_no_ttl_input_exists() {
    let c = db();
    let r = seed(&c, 1);
    c.execute("UPDATE job_meter_summaries SET terminal_at_ms=NULL", [])
        .unwrap();
    let mut job = crate::jobs::get(&c, &JobId::try_from(r.job_id.clone()).unwrap())
        .unwrap()
        .unwrap();
    job.outcome = None;
    job.completed_at_ms = None;
    job.phase = Phase::OutcomeUnknown;
    job.observation.status = ObservationStatus::Missing;
    job.updated_at_ms = 250;
    track(&c, &job, false).unwrap();
    assert_eq!(binding(&c, &r.job_id).unwrap().unwrap().terminal, Some(250));
    job.completed_at_ms = Some(900);
    track(&c, &job, false).unwrap();
    assert_eq!(binding(&c, &r.job_id).unwrap().unwrap().terminal, Some(250));
}

#[test]
fn expiry_transaction_failure_preserves_all_payloads_then_recovers() {
    let mut c = db();
    for n in 1..=3 {
        let r = seed(&c, n);
        report(&mut c, &r, 300).unwrap();
    }
    c.execute_batch(
        "CREATE TRIGGER inject_expiry_failure BEFORE UPDATE OF expired_at_ms
        ON job_meter_summaries WHEN NEW.expired_at_ms IS NOT NULL
        BEGIN SELECT RAISE(ABORT,'injected'); END;",
    )
    .unwrap();
    assert_eq!(
        expire(&mut c, 200 + JOB_METADATA_RETENTION_MS),
        Err(MeterError::Busy)
    );
    assert!(c.is_autocommit());
    for n in 1..=3 {
        assert!(payload(&c, &fixture(n).job_id).is_some());
    }
    c.execute_batch("DROP TRIGGER inject_expiry_failure")
        .unwrap();
    assert_eq!(expire(&mut c, 200 + JOB_METADATA_RETENTION_MS).unwrap(), 3);
}

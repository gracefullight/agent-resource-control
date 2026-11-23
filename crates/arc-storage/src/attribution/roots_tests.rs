//! The Proven roots read: RUNNING managed jobs with verified children and
//! active leases only, failing closed over the bound.
use super::*;

fn database() -> Connection {
    let mut connection = Connection::open_in_memory().unwrap();
    crate::migration::migrate(&mut connection, crate::MIGRATIONS).unwrap();
    connection
}

#[test]
fn only_running_jobs_with_verified_children_and_active_leases_are_roots() {
    let db = database();
    let boot = "00000000-0000-4000-8000-000000000000";
    let rows = [
        ("a", "RUNNING", Some("claude-code"), Some(1)),
        ("b", "RUNNING", None, Some(1)),        // released permit
        ("c", "RUNNING", Some("cursor"), None), // ownership never verified
        ("d", "SUCCEEDED", Some("cursor"), Some(1)),
        ("e", "RUNNING", Some("cursor"), Some(1)), // no permit
        ("f", "RUNNING", Some("cursor"), Some(1)), // stale generation
    ];
    for (i, (id, state, vendor, verified)) in rows.into_iter().enumerate() {
        db.execute(
            "INSERT INTO jobs(id,job_class,locality,enforcement,state,created_at,vendor)
             VALUES(?1,'BUILD','LOCAL','MANAGED',?2,0,?3)",
            params![id, state, vendor],
        )
        .unwrap();
        db.execute(
            "INSERT INTO job_lifecycle(job_id,registered_at_ms,observer_pid,observer_start_token,
               observer_boot_id,observer_verified_at_ms,child_pid,child_start_token,child_boot_id,
               child_verified_at_ms,classification_reason,classification_uncertainty,
               observation_target,observation_status,observation_at_ms,observation_reason,
               revision,updated_at_ms,reason)
             VALUES(?1,0,1,'1',?2,0,?3,'7',?2,?4,'r','NONE','CHILD','VERIFIED_MATCH',0,'r',1,0,'r')",
            params![id, boot, 100 + i as i64, verified],
        )
        .unwrap();
        let context = format!("{i:064x}");
        db.execute(
            "INSERT INTO traffic_entries(job_id,state,queued_at_ms,granted_at_ms,generation,context)
             VALUES(?1,'RUNNING',0,1,1,?2)",
            params![id, context],
        )
        .unwrap();
        if id != "c" && id != "e" {
            db.execute(
                "INSERT INTO permits(id,job_id,state,reserved_at,generation,released_at)
                 VALUES(?1,?1,?2,1,?4,?3)",
                params![
                    id,
                    if id == "b" { "RELEASED" } else { "ACTIVE" },
                    if id == "b" { Some(2) } else { None },
                    if id == "f" { 2 } else { 1 }
                ],
            )
            .unwrap();
        }
    }
    let roots = managed_roots(&db).unwrap();
    let found: Vec<_> = roots
        .iter()
        .map(|root| (root.identity.pid, root.vendor.as_str()))
        .collect();
    assert_eq!(found, vec![(100, "claude-code")]);
    assert_eq!(roots[0].identity.start_token, "7");
    db.execute(
        "UPDATE traffic_entries SET root_job_id='a',context=NULL WHERE job_id='e'",
        [],
    )
    .unwrap();
    assert_eq!(
        managed_roots(&db).unwrap().len(),
        2,
        "nested job shares the root lease"
    );
    db.execute(
        "UPDATE permits SET state='RELEASED',released_at=2 WHERE job_id='a'",
        [],
    )
    .unwrap();
    assert!(
        managed_roots(&db).unwrap().is_empty(),
        "released root lease proves neither job"
    );
}

#[test]
fn root_limit_fails_closed_instead_of_omitting_managed_jobs() {
    let mut db = database();
    let tx = db.transaction().unwrap();
    for i in 0..=MAX_MANAGED_ROOTS {
        let id = format!("job{i}");
        let context = format!("{i:064x}");
        tx.execute("INSERT INTO jobs(id,job_class,locality,enforcement,state,created_at) VALUES(?1,'BUILD','LOCAL','SCHEDULED','RUNNING',0)", [&id]).unwrap();
        tx.execute("INSERT INTO job_lifecycle(job_id,registered_at_ms,observer_pid,observer_start_token,observer_boot_id,observer_verified_at_ms,child_pid,child_start_token,child_boot_id,child_verified_at_ms,classification_reason,classification_uncertainty,observation_target,observation_status,observation_at_ms,observation_reason,revision,updated_at_ms,reason) VALUES(?1,0,1,'1','00000000-0000-4000-8000-000000000000',0,2,'2','00000000-0000-4000-8000-000000000000',1,'r','NONE','CHILD','VERIFIED_MATCH',1,'r',1,1,'r')", [&id]).unwrap();
        tx.execute("INSERT INTO traffic_entries(job_id,state,queued_at_ms,granted_at_ms,generation,context) VALUES(?1,'RUNNING',0,1,1,?2)", params![id, context]).unwrap();
        tx.execute("INSERT INTO permits(id,job_id,state,reserved_at,generation) VALUES(?1,?1,'ACTIVE',1,1)", [&id]).unwrap();
    }
    tx.commit().unwrap();
    assert_eq!(managed_roots(&db), Err(JobError::IdentityUnavailable));
}

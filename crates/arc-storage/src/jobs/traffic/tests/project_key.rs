//! Seam for project fair queuing (AT-TRF-08): the stored key, the order of
//! keyless entries, the fairness reason codec and the version-7 migration.
use super::*;
use crate::{assess_schema, migration::migrate, MIGRATIONS};
use arc_ipc::traffic::fairness::ProjectKey;
use arc_policy::{
    evaluator::MissingObservation, DecisionKind, DecisionReason, FairnessReason, NegotiatedReason,
};

fn key(n: u32) -> ProjectKey {
    ProjectKey::try_from(format!("{n:064x}")).unwrap()
}

fn add_with(c: &mut Connection, n: u32, project: Option<ProjectKey>, now: u64) -> Reply {
    let request = Request::Enqueue {
        job_id: id(n),
        observer: identity(),
        classification: classification("BUILD"),
        lineage_context: None,
        project,
        event: None,
        usage: None,
    };
    enqueue(
        c,
        &request,
        &OwnerEvidence::gather(&identity()),
        &Policy::default(),
        &context(n),
        None,
        now,
    )
    .unwrap()
}

fn stored_key(c: &Connection, n: u32) -> Option<String> {
    c.query_row(
        "SELECT project_key FROM traffic_entries WHERE job_id=?1",
        [id(n).as_str()],
        |row| row.get(0),
    )
    .unwrap()
}

#[test]
fn null_project_keys_keep_sequence_order() {
    let db = Database::new();
    let mut c = db.open();
    let policy = Policy::default();
    let mut holder = add(&mut c, 1, "BUILD", 100, &policy);
    for n in 2..=5 {
        assert!(
            matches!(
                add(&mut c, n, "BUILD", 100 + u64::from(n), &policy),
                Reply::Queued { .. }
            ),
            "{n}"
        );
        assert_eq!(stored_key(&c, n), None);
    }
    // Every keyless waiter is granted in the order it was queued, whichever
    // waiter polls first after a release.
    for next in 2..=5 {
        complete_job(
            &mut c,
            next - 1,
            generation(&holder),
            Outcome::Exited { code: 0 },
            200 + u64::from(next) * 10,
        );
        for later in (next + 1..=5).rev() {
            assert!(
                matches!(
                    poll_job(&mut c, later, 201 + u64::from(next) * 10, &policy),
                    Reply::Queued { .. }
                ),
                "{later} overtook {next}"
            );
        }
        holder = poll_job(&mut c, next, 202 + u64::from(next) * 10, &policy);
        assert!(
            matches!(holder, Reply::Granted { .. }),
            "{next}: {holder:?}"
        );
    }
    assert_eq!(permit_counts(&c), (5, 1));
}

#[test]
fn enqueue_stores_only_the_opaque_project_key_and_rejects_any_other_text() {
    let db = Database::new();
    let mut c = db.open();
    assert!(matches!(
        add_with(&mut c, 1, Some(key(7)), 100),
        Reply::Granted { .. }
    ));
    assert!(matches!(
        add_with(&mut c, 2, None, 101),
        Reply::Queued { .. }
    ));
    assert_eq!(stored_key(&c, 1), Some(format!("{:064x}", 7)));
    assert_eq!(stored_key(&c, 2), None);
    // A retried enqueue is the same entry; the key is not rewritten.
    assert!(matches!(
        add_with(&mut c, 1, Some(key(7)), 102),
        Reply::Granted { .. }
    ));
    assert_eq!(stored_key(&c, 1), Some(format!("{:064x}", 7)));
    for invalid in ["/Users/someone/project", "ABCDEF", ""] {
        assert!(
            c.execute(
                "UPDATE traffic_entries SET project_key=?1 WHERE job_id=?2",
                [invalid, id(2).as_str()],
            )
            .is_err(),
            "{invalid}"
        );
    }
}

#[test]
fn fairness_reason_is_stored_raw_and_read_as_admitted_by_existing_readers() {
    let db = Database::new();
    let mut c = db.open();
    let granted = add(&mut c, 1, "BUILD", 100, &Policy::default());
    let generation = generation(&granted);
    crate::policy::transaction(&mut c, &MissingObservation, |tx, effective| {
        crate::policy::fairness::record(
            tx,
            &id(1),
            effective,
            FairnessReason::ProjectFairTurn,
            generation,
            101,
        )
    })
    .unwrap();
    let raw: (String, String) = c
        .query_row(
            "SELECT decision,reason_code FROM policy_decisions WHERE job_id=?1 ORDER BY rowid DESC LIMIT 1",
            [id(1).as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(raw, ("admit".to_owned(), "project_fair_turn".to_owned()));
    assert_eq!(
        crate::policy::fairness::negotiated(&c, &id(1), 102).unwrap(),
        Some(NegotiatedReason::Fairness(FairnessReason::ProjectFairTurn))
    );
    // The v1 diagnostics read validates and serializes the projection only.
    let diagnostic = diagnostics(&mut c, &id(1), &Policy::default(), 103).unwrap();
    let decision = diagnostic.admission.unwrap().last_decision.unwrap();
    assert_eq!(
        (decision.kind, decision.reason, decision.permit_generation),
        (
            DecisionKind::Admit,
            DecisionReason::Admitted,
            Some(generation)
        )
    );
    assert!(!serde_json::to_string(&decision).unwrap().contains("fair"));
    // A job with an ordinary grant reads back its recorded reason unchanged.
    complete_job(&mut c, 1, generation, Outcome::Exited { code: 0 }, 104);
    add(&mut c, 2, "BUILD", 105, &Policy::default());
    assert_eq!(
        crate::policy::fairness::negotiated(&c, &id(2), 106).unwrap(),
        Some(NegotiatedReason::Recorded(DecisionReason::Admitted))
    );
    assert_eq!(
        crate::policy::fairness::negotiated(&c, &id(9), 106).unwrap(),
        None
    );
}

#[test]
fn version_6_database_migrates_to_project_key_with_rows_intact() {
    let db = Database::new();
    let mut c = crate::open_connection(&db.0).unwrap();
    migrate(&mut c, &MIGRATIONS[..6]).unwrap();
    assert_eq!(assess_schema(&c).unwrap().applied_version, Some(6));
    for n in 1..=2u32 {
        c.execute(
            "INSERT INTO jobs (id,job_class,locality,enforcement,state,created_at) VALUES (?1,'BUILD','HOST_LOCAL','SCHEDULED','NEW',5)",
            [id(n).as_str()],
        )
        .unwrap();
        c.execute(
            "INSERT INTO traffic_entries(job_id,state,queued_at_ms) VALUES (?1,'QUEUED',?2)",
            (id(n).as_str(), 10 + i64::from(n)),
        )
        .unwrap();
    }
    let rows = |c: &Connection| -> Vec<(i64, String, String, i64)> {
        c.prepare(
            "SELECT sequence,job_id,state,queued_at_ms FROM traffic_entries ORDER BY sequence",
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
    assert_eq!(after_history[..6], before_history[..]);
    assert_eq!(after_history[6].0, "0007_traffic_project");
    let keys: Vec<Option<String>> = c
        .prepare("SELECT project_key FROM traffic_entries ORDER BY sequence")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(keys, [None, None]);
    let integrity: String = c
        .query_row("PRAGMA integrity_check", [], |r| r.get(0))
        .unwrap();
    assert_eq!(integrity, "ok");
    // Reopening applies nothing further.
    drop(c);
    assert_eq!(history(&db.open()), after_history);
}

//! The storage decision for a redelivered vendor hook event (AT-TRF-09,
//! ADR-014): one job per recorded key, decided in the admission transaction.
use super::*;
use crate::hook_event::{duplicates, DUPLICATE_EVENT, DUPLICATE_PREFIX};
use arc_ipc::{governor::GovernorJobResult, traffic::hook_event::EventKey};
use arc_policy::evaluator::MissingObservation;

fn key(n: u32) -> EventKey {
    EventKey::try_from(format!("{n:064x}")).unwrap()
}

fn keyed(n: u32, class: &str, event: Option<EventKey>) -> Request {
    Request::Enqueue {
        job_id: id(n),
        observer: identity(),
        classification: classification(class),
        lineage_context: None,
        project: None,
        event,
    }
}

/// The current client's negotiated route.
fn negotiated(c: &mut Connection, n: u32, event: Option<EventKey>, now: u64) -> Reply {
    let reply = governor::enqueue(
        c,
        &keyed(n, "BUILD", event),
        &OwnerEvidence::gather(&identity()),
        &Policy::default(),
        &context(n),
        None,
        now,
        &MissingObservation,
    )
    .unwrap();
    match reply {
        GovernorJobResult::Traffic { result } => result,
        other => panic!("{other:?}"),
    }
}

fn stored(c: &Connection) -> Vec<(String, Option<String>)> {
    c.prepare("SELECT job_id,event_key FROM job_lifecycle ORDER BY job_id")
        .unwrap()
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

/// Rows of every table an admitted job occupies: jobs, lifecycle, queue,
/// negotiated disposition, permits, tombstones and recorded decisions.
fn rows(c: &Connection) -> [i64; 7] {
    [
        "jobs",
        "job_lifecycle",
        "traffic_entries",
        "governor_jobs",
        "permits",
        "traffic_tombstones",
        "policy_decisions",
    ]
    .map(|table| {
        c.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
            .unwrap()
    })
}

fn audit(c: &Connection) -> Vec<(String, String, Option<String>, Option<String>)> {
    c.prepare(
        "SELECT id,severity,entity_id,payload_redacted_json FROM audit_events \
         WHERE event_type=?1 ORDER BY rowid",
    )
    .unwrap()
    .query_map([DUPLICATE_EVENT], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
    })
    .unwrap()
    .map(Result::unwrap)
    .collect()
}

fn duplicate_row(existing: u32, delivery: u32) -> (String, String, Option<String>, Option<String>) {
    (
        format!(
            "{DUPLICATE_PREFIX}{}-{}",
            id(existing).as_str(),
            id(delivery).as_str()
        ),
        "INFO".into(),
        Some(id(existing).as_str().to_owned()),
        None,
    )
}

#[test]
fn a_recorded_key_answers_a_later_registration_as_a_duplicate_delivery() {
    let db = Database::new();
    let mut c = db.open();
    let first = negotiated(&mut c, 1, Some(key(7)), 100);
    let granted = generation(&first);
    assert_eq!(
        stored(&c),
        [(id(1).as_str().to_owned(), Some(key(7).as_str().to_owned()))]
    );
    let admitted = rows(&c);
    assert_eq!(admitted[..6], [1, 1, 1, 1, 1, 0]);
    assert_eq!(permit_counts(&c), (1, 1));
    assert_eq!(duplicates(&c, &id(1)), Ok(0));

    // The redelivery: no job row, queue entry, disposition, reservation,
    // tombstone or decision, while the first delivery still holds its permit.
    let duplicate = Reply::DuplicateDelivery { job_id: id(1) };
    assert_eq!(negotiated(&mut c, 2, Some(key(7)), 110), duplicate);
    assert_eq!(rows(&c), admitted);
    assert_eq!(permit_counts(&c), (1, 1));
    assert_eq!(audit(&c), [duplicate_row(1, 2)]);
    assert!(get(&c, &id(2)).unwrap().is_none());
    // A registration retried after a lost reply is still one delivery.
    assert_eq!(negotiated(&mut c, 2, Some(key(7)), 120), duplicate);
    assert_eq!(audit(&c), [duplicate_row(1, 2)]);
    // The first delivery, retried, is itself and keeps its grant.
    assert_eq!(
        generation(&negotiated(&mut c, 1, Some(key(7)), 130)),
        granted
    );
    assert_eq!(audit(&c).len(), 1);
    // The older route decides in the same transaction.
    assert_eq!(
        enqueue(
            &mut c,
            &keyed(3, "BUILD", Some(key(7))),
            &OwnerEvidence::gather(&identity()),
            &Policy::default(),
            &context(3),
            None,
            140,
        ),
        Ok(duplicate.clone())
    );
    assert_eq!(audit(&c), [duplicate_row(1, 2), duplicate_row(1, 3)]);

    // The key lives with its job row: it stays recorded after the release.
    complete_job(&mut c, 1, granted, Outcome::Exited { code: 0 }, 200);
    assert_eq!(permit_counts(&c), (1, 0));
    assert_eq!(negotiated(&mut c, 4, Some(key(7)), 210), duplicate);
    assert_eq!(duplicates(&c, &id(1)), Ok(3));
    assert_eq!(rows(&c)[..6], [1, 1, 1, 1, 1, 0]);
    assert_eq!(permit_counts(&c), (1, 0));
    // Nothing recorded names the key, a command or a path.
    for (row_id, _, _, payload) in audit(&c) {
        assert!(!row_id.contains(key(7).as_str()), "{row_id}");
        assert_eq!(payload, None);
    }
}

#[test]
fn distinct_and_absent_keys_admit_as_before() {
    let db = Database::new();
    let mut c = db.open();
    assert!(matches!(
        negotiated(&mut c, 1, Some(key(1)), 100),
        Reply::Granted { .. }
    ));
    // The Build window is 1: a different event and a keyless job both queue.
    for (n, event) in [(2, Some(key(2))), (3, None), (4, None)] {
        assert!(
            matches!(
                negotiated(&mut c, n, event, 100 + u64::from(n)),
                Reply::Queued { .. }
            ),
            "{n}"
        );
    }
    assert_eq!(
        stored(&c),
        [
            (id(1).as_str().to_owned(), Some(key(1).as_str().to_owned())),
            (id(2).as_str().to_owned(), Some(key(2).as_str().to_owned())),
            (id(3).as_str().to_owned(), None),
            (id(4).as_str().to_owned(), None),
        ]
    );
    assert_eq!(rows(&c)[..5], [4, 4, 4, 4, 1]);
    assert!(audit(&c).is_empty());
}

#[test]
fn a_class_that_is_never_admitted_records_no_key_and_no_duplicate() {
    let db = Database::new();
    let mut c = db.open();
    for n in 1..=2 {
        let reply = enqueue(
            &mut c,
            &keyed(n, "REMOTE_EXEC", Some(key(9))),
            &OwnerEvidence::gather(&identity()),
            &Policy::default(),
            &context(n),
            None,
            100,
        )
        .unwrap();
        assert!(matches!(reply, Reply::Bypassed { .. }), "{reply:?}");
    }
    assert_eq!(rows(&c), [0; 7]);
    assert!(audit(&c).is_empty());
}

#[test]
fn an_unverified_owner_is_refused_before_any_duplicate_is_recorded() {
    let db = Database::new();
    let mut c = db.open();
    negotiated(&mut c, 1, Some(key(5)), 100);
    let refused = governor::enqueue(
        &mut c,
        &keyed(2, "BUILD", Some(key(5))),
        &OwnerEvidence::simulated(&identity(), IdentityStatus::Indeterminate),
        &Policy::default(),
        &context(2),
        None,
        110,
        &MissingObservation,
    );
    assert_eq!(refused, Err(JobError::IdentityUnavailable));
    assert!(audit(&c).is_empty());
}

#[test]
fn the_decision_is_only_made_inside_a_transaction() {
    let db = Database::new();
    let mut c = db.open();
    negotiated(&mut c, 1, Some(key(3)), 100);
    let found = key(3);
    assert_eq!(
        crate::hook_event::duplicate(&c, &id(2), Some(&found), 110),
        Err(JobError::InvalidTransition)
    );
    assert_eq!(
        crate::hook_event::record(&c, &id(1), Some(&found)),
        Err(JobError::InvalidTransition)
    );
    // Without a key neither function reads or writes anything.
    assert_eq!(
        crate::hook_event::duplicate(&c, &id(2), None, 110),
        Ok(None)
    );
    assert_eq!(crate::hook_event::record(&c, &id(2), None), Ok(()));
    let tx = c.transaction().unwrap();
    // A key no job recorded is not a duplicate; a job has one key for life.
    assert_eq!(
        crate::hook_event::duplicate(&tx, &id(2), Some(&key(4)), 110),
        Ok(None)
    );
    assert_eq!(
        crate::hook_event::record(&tx, &id(1), Some(&key(4))),
        Err(JobError::StorageFailure)
    );
    assert_eq!(
        crate::hook_event::duplicate(&tx, &id(2), Some(&found), u64::MAX),
        Err(JobError::InvalidRequest)
    );
    drop(tx);
    assert!(audit(&c).is_empty());
}

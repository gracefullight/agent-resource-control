use super::*;

const TABLES: &str = "CREATE TABLE traffic_entries(job_id TEXT, granted_at_ms INTEGER);
    CREATE TABLE governor_jobs(job_id TEXT, release_id TEXT);
    CREATE TABLE job_lifecycle(job_id TEXT, outcome_kind TEXT);";

fn job(c: char) -> JobId {
    JobId::try_from(c.to_string().repeat(32)).unwrap()
}

const NOTHING: Due = Due {
    registration: false,
    start: false,
    end: false,
};

#[test]
fn only_the_call_that_commits_a_mark_is_due_its_reading() {
    let db = rusqlite::Connection::open_in_memory().unwrap();
    db.execute_batch(TABLES).unwrap();
    let a = job('a');
    let none = marks(&db, &a);
    assert_eq!(
        none,
        Some(Marks {
            registered: false,
            started: false,
            ended: false
        })
    );
    db.execute("INSERT INTO traffic_entries VALUES(?1,NULL)", [a.as_str()])
        .unwrap();
    db.execute("INSERT INTO job_lifecycle VALUES(?1,NULL)", [a.as_str()])
        .unwrap();
    let queued = marks(&db, &a);
    assert_eq!(
        transition(none, queued),
        Due {
            registration: true,
            ..NOTHING
        },
        "queued is a registration, not a grant"
    );
    db.execute("UPDATE traffic_entries SET granted_at_ms=5", [])
        .unwrap();
    let granted = marks(&db, &a);
    assert_eq!(
        transition(queued, granted),
        Due {
            start: true,
            ..NOTHING
        }
    );
    // A replayed poll or grant finds the mark already set.
    assert_eq!(transition(granted, granted), NOTHING);
    db.execute("UPDATE job_lifecycle SET outcome_kind='EXITED'", [])
        .unwrap();
    let ended = marks(&db, &a);
    assert_eq!(
        transition(granted, ended),
        Due {
            end: true,
            ..NOTHING
        }
    );
    // A replayed completion, whether or not the original stored its reading.
    assert_eq!(transition(ended, ended), NOTHING);

    // An unreserved Governor release is a start.
    let b = job('b');
    let before = marks(&db, &b);
    db.execute("INSERT INTO governor_jobs VALUES(?1,'r')", [b.as_str()])
        .unwrap();
    assert_eq!(
        transition(before, marks(&db, &b)),
        Due {
            start: true,
            ..NOTHING
        }
    );
    // Unreadable marks take no reading.
    assert_eq!(transition(None, ended), NOTHING);
    assert_eq!(transition(granted, None), NOTHING);
    let broken = rusqlite::Connection::open_in_memory().unwrap();
    assert_eq!(marks(&broken, &a), None);
}

/// The writer binds a reading taken before the call to the commit that made
/// it due; an outcome committed without one is bound unavailable at once, so
/// a later reading can never stand in for it.
#[test]
fn readings_are_bound_to_the_commit_that_made_them_due() {
    let db = rusqlite::Connection::open_in_memory().unwrap();
    db.execute_batch(
        "CREATE TABLE jobs(id TEXT PRIMARY KEY,
            footprint_start_bytes INTEGER, footprint_start_completeness TEXT,
            footprint_end_bytes INTEGER, footprint_end_completeness TEXT);",
    )
    .unwrap();
    for c in ['a', 'b', 'c'] {
        db.execute("INSERT INTO jobs(id) VALUES(?1)", [job(c).as_str()])
            .unwrap();
    }
    let read = |c: char| -> (Option<i64>, Option<String>, Option<i64>, Option<String>) {
        db.query_row(
            "SELECT footprint_start_bytes,footprint_start_completeness,
                    footprint_end_bytes,footprint_end_completeness FROM jobs WHERE id=?1",
            [job(c).as_str()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .unwrap()
    };
    let ended = Due {
        end: true,
        ..NOTHING
    };
    let measured = Bound {
        registration: None,
        end: Some(Reading::Complete(4096)),
    };
    assert_eq!(bind(&db, &job('a'), ended, measured), None);
    assert_eq!(read('a'), (None, None, Some(4096), Some("complete".into())));
    // A replay is due nothing; its later reading is discarded.
    let later = Bound {
        registration: None,
        end: Some(Reading::Complete(8192)),
    };
    assert_eq!(bind(&db, &job('a'), NOTHING, later), None);
    assert_eq!(read('a').2, Some(4096));
    // An outcome committed with no reading is bound unavailable.
    assert_eq!(bind(&db, &job('b'), ended, Bound::default()), None);
    assert_eq!(read('b'), (None, None, None, Some("unavailable".into())));
    assert_eq!(bind(&db, &job('b'), ended, later), None);
    assert_eq!(read('b').3.as_deref(), Some("unavailable"));
    // A registration stores its start; a grant is left to the caller.
    let registered = Due {
        registration: true,
        ..NOTHING
    };
    let start = Bound {
        registration: Some(Reading::Partial(10)),
        end: None,
    };
    assert_eq!(bind(&db, &job('c'), registered, start), None);
    assert_eq!(read('c'), (Some(10), Some("partial".into()), None, None));
    let granted = Due {
        start: true,
        ..NOTHING
    };
    assert_eq!(
        bind(&db, &job('c'), granted, Bound::default()),
        Some(Point::Start)
    );
    let granted_and_ended = Due {
        start: true,
        end: true,
        ..NOTHING
    };
    assert_eq!(bind(&db, &job('c'), granted_and_ended, measured), None);
}

#[test]
fn only_enqueue_poll_and_complete_name_a_footprint_job() {
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let id = job('a');
    let complete = JobRequest::Complete {
        job_id: id.clone(),
        observer: observer.clone(),
        outcome: arc_ipc::jobs::Outcome::Exited { code: 0 },
        permit_generation: 1,
    };
    assert_eq!(traffic_job(&complete).map(|(j, _)| j), Some(&id));
    let poll = JobRequest::Poll {
        job_id: id.clone(),
        observer: observer.clone(),
    };
    assert_eq!(traffic_job(&poll).map(|(j, _)| j), Some(&id));
    let cancel = JobRequest::Cancel {
        job_id: id.clone(),
        observer,
        permit_generation: None,
    };
    assert!(traffic_job(&cancel).is_none());
    assert!(traffic_job(&JobRequest::Status { job_id: id }).is_none());
}

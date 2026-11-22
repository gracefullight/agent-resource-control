//! A scheduled job driven over the daemon's socket by this test process.
//!
//! The wrapper's requests cannot pass through a proxy: the daemon binds a
//! Traffic request to the process that opened the socket. So the two wire
//! faults a permit must survive are applied here, by a real client of the
//! real daemon: a complete request whose reply is never read, and the same
//! request repeated. The job holds a real permit beside the wrappers' jobs.
use crate::{journey::eventually, runtime::Fixture};
use arc_ipc::{
    jobs::{JobId, Outcome, ProcessIdentity, SpawnReason},
    traffic::{JobRequest, JobResult, OPERATION_TRAFFIC_JOB},
};
use std::os::unix::net::UnixStream;

fn wire(request: &JobRequest) -> arc_ipc::Request {
    let mut wire = arc_ipc::Request::new("nfr-009", OPERATION_TRAFFIC_JOB);
    wire.payload = serde_json::to_value(request).unwrap();
    wire
}

fn exchange(f: &Fixture, request: &JobRequest) -> JobResult {
    let response = arc_ipc::exchange(&mut f.connect(), &wire(request)).unwrap();
    assert_eq!(response.status, arc_ipc::STATUS_OK, "{response:?}");
    serde_json::from_value(response.result.unwrap()).unwrap()
}

/// Send one complete request and never read its reply. The caller drops the
/// connection once it has read the commit from SQLite.
fn unanswered(f: &Fixture, request: &JobRequest) -> UnixStream {
    let mut stream = f.connect();
    arc_ipc::write_message(&mut stream, &wire(request)).unwrap();
    stream
}

fn states(db: &rusqlite::Connection, id: &JobId) -> (Option<String>, Option<String>) {
    db.query_row(
        "SELECT (SELECT state FROM traffic_entries WHERE job_id=?1),\
         (SELECT state FROM permits WHERE job_id=?1)",
        [id.as_str()],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )
    .unwrap()
}

/// Enqueue job `number` of the class of `cargo <subcommand>`. With
/// `lose_replies` every request is first sent once without its reply being
/// read; every request is then repeated `repeats` times and must get the same
/// answer. A granted job is completed and its permit released once; a queued
/// job is cancelled and never gets a permit. Returns whether it was granted.
pub fn job(
    f: &Fixture,
    db: &rusqlite::Connection,
    number: u128,
    subcommand: &str,
    lose_replies: bool,
    repeats: usize,
    observe: &mut dyn FnMut(&str),
) -> bool {
    let (class, reason) = match subcommand {
        "build" => ("BUILD", "CARGO_BUILD"),
        "test" => ("TEST", "CARGO_TEST"),
        _ => ("LINT", "CARGO_CLIPPY"),
    };
    let id = JobId::try_from(format!("{:032x}", (0xf0u128 << 120) + number)).unwrap();
    let observer = ProcessIdentity::capture(std::process::id()).unwrap();
    let enqueue = JobRequest::Enqueue {
        job_id: id.clone(),
        observer: observer.clone(),
        classification: serde_json::from_value(serde_json::json!({"classification":class,
            "locality":"HOST_LOCAL","reason":reason,"uncertainty":"NONE"}))
        .unwrap(),
        lineage_context: None,
        project: None,
        event: None,
        usage: None,
    };
    let send = |request: &JobRequest, committed: &dyn Fn() -> bool, label: &str| {
        if lose_replies {
            let lost = unanswered(f, request);
            eventually(label, committed);
            drop(lost);
        }
        let first = exchange(f, request);
        for _ in 0..repeats {
            assert_eq!(exchange(f, request), first, "{label}: a repeated request");
        }
        first
    };
    let first = send(
        &enqueue,
        &|| states(db, &id).0.is_some(),
        "the unanswered enqueue committed",
    );
    observe("protocol job enqueued");
    match first {
        JobResult::Granted {
            permit_generation, ..
        } => {
            assert_eq!(
                states(db, &id),
                (Some("RESERVED".to_owned()), Some("ACTIVE".to_owned()))
            );
            let complete = JobRequest::Complete {
                job_id: id.clone(),
                observer,
                permit_generation,
                outcome: Outcome::SpawnFailed {
                    reason: SpawnReason::NotFound,
                    os_code: Some(libc::ENOENT),
                },
            };
            let done = send(
                &complete,
                &|| states(db, &id).1.as_deref() == Some("RELEASED"),
                "the unanswered completion committed",
            );
            assert!(matches!(done, JobResult::Completed { .. }), "{done:?}");
            observe("protocol job completed");
            assert_eq!(
                states(db, &id),
                (Some("COMPLETED".to_owned()), Some("RELEASED".to_owned()))
            );
            true
        }
        JobResult::Queued { .. } => {
            let cancel = JobRequest::Cancel {
                job_id: id.clone(),
                observer,
                permit_generation: None,
            };
            let cancelled = send(
                &cancel,
                &|| states(db, &id).0.as_deref() == Some("CANCELLED"),
                "the unanswered cancellation committed",
            );
            assert_eq!(cancelled, JobResult::Cancelled {});
            // A late copy of the enqueue cannot bring the job back.
            assert_eq!(exchange(f, &enqueue), JobResult::Cancelled {});
            observe("protocol job cancelled");
            assert_eq!(states(db, &id), (Some("CANCELLED".to_owned()), None));
            false
        }
        other => panic!("protocol job {number}: {other:?}"),
    }
}

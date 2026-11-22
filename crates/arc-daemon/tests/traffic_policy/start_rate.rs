//! DEC-013 item 4 and POL-001 item 2 on the path a user's command takes.
use super::fixture::*;
use crate::{
    journey::{eventually, queue_state, starts, Sandbox},
    runtime::{child_watchdog::Owned, Fixture},
};
use std::process::Stdio;

const CLASSES: [&str; 4] = ["BUILD", "TEST", "LINT", "TYPECHECK"];

/// The `reserved_at` of every Build permit, oldest first, with its state and
/// generation.
fn build_permits(db: &rusqlite::Connection) -> Vec<(String, i64, String, i64)> {
    db.prepare(
        "SELECT p.job_id,p.reserved_at,p.state,p.generation FROM permits p JOIN jobs j
         ON j.id=p.job_id WHERE j.job_class='BUILD' ORDER BY p.reserved_at,p.rowid",
    )
    .unwrap()
    .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
    .unwrap()
    .map(Result::unwrap)
    .collect()
}

pub fn bucket_queues_then_grants_without_lost_or_duplicated_permit() {
    let f = Fixture::new();
    let s = Sandbox::new(&f);
    s.stub("cargo");
    let project = s.project("project");
    let marker = f.root.join("starts");

    // The conservative defaults (POL-001), read from a daemon with no
    // start-rate configuration through the built CLI.
    let daemon_default = daemon(&f, &[]);
    run(&f, &s, &project, "test", &marker);
    let db = database(&f);
    let tested: String = db
        .query_row("SELECT id FROM jobs WHERE job_class='TEST'", [], |r| {
            r.get(0)
        })
        .unwrap();
    let document = diagnostics_json(&f, &s, &tested);
    let policy = &document["decision_history"]["policy"];
    assert_eq!(policy["start_rate_control"], true, "{document}");
    for (index, name) in CLASSES.iter().enumerate() {
        assert_default_class(&policy["classes"][index], name);
    }
    drop(db);
    stop(&f, daemon_default);

    // A tighter Build bucket through the startup Traffic configuration: one
    // token, refilled every 10 s. Other classes keep their defaults.
    let tight = daemon(&f, &[(START_RATE, "build=6"), (START_BURST, "build=1")]);
    let db = database(&f);
    run(&f, &s, &project, "build", &marker);
    let first = build_permits(&db);
    assert_eq!(first.len(), 1, "{first:?}");
    let token_at = first[0].1 + 10_000;

    // The next build finds the Build window free but no token: QUEUED with
    // the start-rate reason recorded, no permit, no child.
    let mut held = cargo(&f, &s, &project, "build", &marker);
    let mut held = Owned::spawn(held.stderr(Stdio::null())).unwrap();
    let pid = held.child.id();
    let mut job = None;
    eventually("the start-rate hold", || {
        if let Some(status) = held.child.try_wait().unwrap() {
            panic!("the held build ended before it was observed queued: {status:?}");
        }
        if queue_state(&db, pid).as_deref() != Some("QUEUED") {
            return false;
        }
        let id = job_of(&db, pid);
        let held = decisions(&db, &id)
            .last()
            .is_some_and(|(kind, reason, _, key)| {
                (kind.as_str(), reason.as_str()) == ("wait", "fifo")
                    && key.starts_with("[\"start_rate\"")
            });
        job = Some(id);
        held
    });
    let job = job.unwrap();
    let active: i64 = db
        .query_row(
            "SELECT count(*) FROM permits WHERE state='ACTIVE'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(
        active, 0,
        "the window is free: only the bucket holds the job"
    );
    assert_eq!(build_permits(&db).len(), 1, "a held job has no permit");
    assert_eq!(starts(&marker), 2, "the held build's child has not started");

    // v1 readers see the projection; the negotiated read the raw reason,
    // the queue position, the token time and the configured settings.
    let document = diagnostics_json(&f, &s, &job);
    let last = &document["admission"]["last_decision"];
    assert_eq!(
        (&last["kind"], &last["reason"]),
        (&"wait".into(), &"fifo".into())
    );
    let history = &document["decision_history"];
    assert_eq!(
        history["decisions"].as_array().unwrap().last().unwrap()["reason"],
        "start_rate"
    );
    assert_eq!(
        history["queue_position"],
        serde_json::json!([1, 1]),
        "{document}"
    );
    assert_eq!(history["expected_schedule_ms"], token_at, "{document}");
    let build = &history["policy"]["classes"][0];
    assert_eq!(
        (&build["start_tokens_per_minute"], &build["start_burst"]),
        (&6.into(), &1.into())
    );
    for (index, name) in CLASSES.iter().enumerate().skip(1) {
        assert_default_class(&history["policy"]["classes"][index], name);
    }
    let text = diagnostics(&f, &s, &job, false);
    for line in [
        "Last recorded decision: fifo\n".to_owned(),
        "wait start_rate (waiting for a start-rate token)\n".to_owned(),
        "Queue position: 1 of 1\n".to_owned(),
        format!("Expected schedule: start-rate token at {token_at} (Unix ms)\n"),
    ] {
        assert!(text.contains(&line), "{line:?} in {text}");
    }
    assert_eq!(
        queue_state(&db, pid).as_deref(),
        Some("QUEUED"),
        "read while held"
    );

    // The waiter's own poll is granted once the token is due: one permit,
    // one start, and the grant follows the token time in daemon state.
    assert!(held.wait("held build").unwrap().success());
    eventually("the release", || {
        build_permits(&db)
            .iter()
            .all(|(_, _, state, _)| state == "RELEASED")
    });
    let permits = build_permits(&db);
    assert_eq!(permits.len(), 2, "{permits:?}");
    assert_eq!(permits[1].0, job);
    assert!(
        permits[1].1 >= token_at,
        "granted before its token: {permits:?}"
    );
    assert_eq!(starts(&marker), 3, "each build ran its child exactly once");
    let recorded = decisions(&db, &job);
    assert!(recorded[0].3.starts_with("[\"start_rate\""), "{recorded:?}");
    assert!(
        recorded
            .iter()
            .any(|(kind, reason, _, _)| kind == "admit" && reason == "admitted"),
        "{recorded:?}"
    );
    assert_eq!(
        recorded
            .iter()
            .filter(|(_, _, _, key)| key.starts_with("[\"start_rate\""))
            .count(),
        1,
        "repeated polls of one hold record it once: {recorded:?}"
    );
    let state: String = db
        .query_row(
            "SELECT state FROM traffic_entries WHERE job_id=?1",
            [&job],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(state, "COMPLETED");
    drop(db);
    stop(&f, tight);
}

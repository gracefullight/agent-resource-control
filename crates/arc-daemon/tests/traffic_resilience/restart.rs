//! One scheduled job, held RUNNING with its ACTIVE permit, across a restart
//! of the daemon (TRF-009 item 4, AT-TRF-10 item 3).
use crate::{
    fixture::{alive, World},
    support,
};
use arc_storage::{assess_schema, SchemaStatus, MIGRATIONS};

/// Everything the daemon holds for one admitted job: the permit, the queue
/// entry and the child identity (PID and start time).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Held {
    permit: String,
    permit_state: String,
    permit_generation: i64,
    reserved_at: i64,
    released_at: Option<i64>,
    entry_state: String,
    entry_generation: i64,
    granted_at_ms: i64,
    child_pid: Option<u32>,
    child_start_token: Option<String>,
    outcome: Option<String>,
    exit_code: Option<i64>,
}

fn held(db: &rusqlite::Connection, job: &str) -> Held {
    db.query_row(
        "SELECT p.id,p.state,p.generation,p.reserved_at,p.released_at,t.state,t.generation,\
         t.granted_at_ms,l.child_pid,l.child_start_token,l.outcome_kind,j.exit_code \
         FROM permits p JOIN traffic_entries t ON t.job_id=p.job_id \
         JOIN job_lifecycle l ON l.job_id=p.job_id JOIN jobs j ON j.id=p.job_id WHERE p.job_id=?1",
        [job],
        |r| {
            Ok(Held {
                permit: r.get(0)?,
                permit_state: r.get(1)?,
                permit_generation: r.get(2)?,
                reserved_at: r.get(3)?,
                released_at: r.get(4)?,
                entry_state: r.get(5)?,
                entry_generation: r.get(6)?,
                granted_at_ms: r.get(7)?,
                child_pid: r.get(8)?,
                child_start_token: r.get(9)?,
                outcome: r.get(10)?,
                exit_code: r.get(11)?,
            })
        },
    )
    .unwrap()
}

/// (jobs, permits, ACTIVE permits) in the whole database.
fn totals(db: &rusqlite::Connection) -> (i64, i64, i64) {
    db.query_row(
        "SELECT (SELECT count(*) FROM job_lifecycle),(SELECT count(*) FROM permits),\
         (SELECT count(*) FROM permits WHERE state='ACTIVE')",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )
    .unwrap()
}

enum Restart {
    /// SIGKILL: no shutdown path runs.
    ForcedKill,
    /// A normal stop, then the database is put back to an earlier schema
    /// version, so the current daemon binary migrates it when it starts.
    SchemaStepBack,
}

/// A real daemon admits `arcctl exec -- cargo build`; its stub child is alive
/// at a barrier and the job holds the one Build permit. The daemon is then
/// restarted. Afterwards the same permit is ACTIVE, the job is RUNNING with
/// the same child PID and start time, which the daemon rechecks against the
/// live process, and the stub was started once. When the child exits the
/// permit is released once and nothing is replayed.
fn scheduled_job_keeps_its_permit_and_child_across(restart: Restart) {
    let _serial = support::serial();
    let mut w = World::new();
    let mut job = w.start("build");
    let db = w.db();
    let child = job.running(&db);
    let id = job.id(&db).unwrap();
    let before = held(&db, &id);
    assert_eq!(
        (before.permit_state.as_str(), before.entry_state.as_str()),
        ("ACTIVE", "RUNNING")
    );
    assert_eq!(before.released_at, None);
    assert_eq!(before.permit_generation, before.entry_generation);
    assert_eq!(
        (before.child_pid, before.child_start_token.as_deref()),
        (Some(child.pid), Some(child.start_token.as_str()))
    );
    assert_eq!(totals(&db), (1, 1, 1));
    assert_eq!(job.starts(), 1);
    drop(db);

    let current = MIGRATIONS.last().unwrap().version;
    match restart {
        Restart::ForcedKill => w.kill(),
        Restart::SchemaStepBack => {
            // One version back keeps every value of the held job: only the
            // newest migration's column and index are gone.
            w.stop_and_step_back(current - 1);
            let db = w.db();
            let schema = assess_schema(&db).unwrap();
            assert_eq!(
                (schema.status, schema.applied_version),
                (SchemaStatus::UpgradeRequired, Some(current - 1))
            );
            // The earlier schema still holds the same ACTIVE permit.
            assert_eq!(held(&db, &id), before);
        }
    }
    // Neither the wrapper nor its child noticed the daemon going away.
    assert!(job.wrapper.child.try_wait().unwrap().is_none());
    assert!(alive(&child));
    w.boot();

    let db = w.db();
    let schema = assess_schema(&db).unwrap();
    assert_eq!(
        (
            schema.status,
            schema.applied_version,
            schema.history_verified
        ),
        (SchemaStatus::Compatible, Some(current), true)
    );
    let doctor = w.arcctl(&["doctor"]);
    assert!(doctor.status.success(), "{doctor:?}");
    assert!(
        String::from_utf8_lossy(&doctor.stdout).contains(&format!(
            "Schema compatible. Version {current}. Migration history verified."
        )),
        "{doctor:?}"
    );
    // Startup reconciliation alone leaves the permit and the job as they were.
    assert_eq!(held(&db, &id), before, "the restart changed the held job");
    assert_eq!(totals(&db), (1, 1, 1));
    // The daemon's own answer: it rechecks the recorded PID and start time
    // against the live process before it calls the job running.
    let status = w.status(&id);
    assert!(
        status.starts_with(&format!("Job: {id}\nState: running\n")),
        "{status}"
    );
    assert!(alive(&child));
    assert_eq!(
        held(&db, &id),
        before,
        "the status read changed the held job"
    );
    assert_eq!(job.starts(), 1, "the restart replayed the command");
    assert!(job.wrapper.child.try_wait().unwrap().is_none());

    // The child exits: one release, and the recorded outcome is its own.
    job.release();
    let exit = job.wrapper.wait("held build").unwrap();
    assert!(exit.success(), "{exit:?}: {}", job.diagnostics());
    let after = held(&db, &id);
    assert_eq!(
        (after.permit_state.as_str(), after.entry_state.as_str()),
        ("RELEASED", "COMPLETED")
    );
    let released_at = after.released_at.expect("a release time");
    assert_eq!(
        (after.outcome.as_deref(), after.exit_code),
        (Some("EXITED"), Some(0))
    );
    assert_eq!(
        Held {
            permit_state: before.permit_state.clone(),
            entry_state: before.entry_state.clone(),
            released_at: None,
            outcome: None,
            exit_code: None,
            ..after.clone()
        },
        before,
        "the release changed more than the states and the outcome"
    );
    assert_eq!(totals(&db), (1, 1, 0));
    let status = w.status(&id);
    assert!(
        status.starts_with(&format!("Job: {id}\nState: succeeded\nExit code: 0\n")),
        "{status}"
    );
    // A second restart finds nothing to release again and replays nothing.
    w.stop();
    w.boot();
    assert_eq!(held(&db, &id), after);
    assert_eq!(after.released_at, Some(released_at));
    assert_eq!(totals(&db), (1, 1, 0));
    assert_eq!(job.starts(), 1);
    drop(db);
    w.finish();
}

#[test]
fn trf_009_schema_migrating_restart_keeps_the_active_permit_and_running_job() {
    scheduled_job_keeps_its_permit_and_child_across(Restart::SchemaStepBack);
}

#[test]
fn at_trf_10_forced_daemon_kill_keeps_one_active_permit_and_one_start() {
    scheduled_job_keeps_its_permit_and_child_across(Restart::ForcedKill);
}

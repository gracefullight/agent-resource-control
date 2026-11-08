use super::{claimed, get, id, mutate, recent, register, Database, JobError, StatusResult};
use crate::jobs::reconcile::RECENT_SQL;
use rusqlite::{params, Connection, StatementStatus};

fn recent_steps(c: &Connection) -> i32 {
    let mut stmt = c.prepare(RECENT_SQL).unwrap();
    let mut rows = stmt.query([]).unwrap();
    while rows.next().unwrap().is_some() {}
    drop(rows);
    stmt.get_status(StatementStatus::VmStep)
}
fn add_legacy(c: &Connection, start: u32) {
    c.execute(
        "WITH RECURSIVE n(i) AS (VALUES(0) UNION ALL SELECT i+1 FROM n WHERE i<4999)
        INSERT INTO jobs (id,job_class,locality,enforcement,state,created_at)
        SELECT printf('legacy-%d',i+?1),'UNKNOWN','UNKNOWN','OBSERVE','NEW',10000+i FROM n",
        [start],
    )
    .unwrap();
}
#[test]
fn recent_work_is_bounded_before_summary_join_despite_newer_legacy_jobs() {
    let db = Database::new();
    let mut c = db.open();
    mutate(&mut c, &register(1), 100);
    let one_steps = recent_steps(&c);
    add_legacy(&c, 0);
    assert_eq!(
        recent_steps(&c),
        one_steps,
        "legacy rows must not be scanned"
    );
    let StatusResult::Recent {
        jobs, truncated, ..
    } = recent(&c, 200).unwrap()
    else {
        panic!("expected recent");
    };
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].job_id, id(1));
    assert!(!truncated);
    for n in 2..=60 {
        mutate(&mut c, &register(n), 100 + u64::from(n / 3));
    }
    let bounded_steps = recent_steps(&c);
    assert!(
        bounded_steps < 5000,
        "bounded query used {bounded_steps} VM steps"
    );
    add_legacy(&c, 5000);
    assert_eq!(recent_steps(&c), bounded_steps);
    let StatusResult::Recent {
        jobs,
        truncated,
        liveness_fresh,
        ..
    } = recent(&c, 200).unwrap()
    else {
        panic!("expected recent");
    };
    assert!(truncated);
    assert!(!liveness_fresh);
    let expected: Vec<_> = (11..=60).rev().map(id).collect();
    assert_eq!(
        jobs.iter().map(|j| j.job_id.clone()).collect::<Vec<_>>(),
        expected
    );
    assert!(jobs
        .windows(2)
        .all(|w| w[0].registered_at_ms >= w[1].registered_at_ms));
}

#[test]
fn exclusive_outcome_constraint_rejects_every_invalid_nullable_combination() {
    outcome_matrix(false);
}
#[test]
fn decoder_rejects_invalid_outcome_fields_even_when_sql_checks_are_bypassed() {
    outcome_matrix(true);
}
fn outcome_matrix(bypass_checks: bool) {
    let db = Database::new();
    let mut c = db.open();
    claimed(&mut c, 1);
    if bypass_checks {
        c.pragma_update(None, "ignore_check_constraints", true)
            .unwrap();
    }
    let mut accepted = 0;
    let mut rejected = 0;
    for kind in [None, Some("EXITED"), Some("SIGNALED"), Some("SPAWN_FAILED")] {
        for signal in [None, Some(15)] {
            for core in [None, Some(0)] {
                for spawn in [None, Some("OTHER")] {
                    for os_code in [None, Some(2)] {
                        let valid = match kind {
                            None | Some("EXITED") => {
                                signal.is_none()
                                    && core.is_none()
                                    && spawn.is_none()
                                    && os_code.is_none()
                            }
                            Some("SIGNALED") => {
                                signal.is_some()
                                    && core.is_some()
                                    && spawn.is_none()
                                    && os_code.is_none()
                            }
                            Some("SPAWN_FAILED") => {
                                signal.is_none() && core.is_none() && spawn.is_some()
                            }
                            _ => unreachable!(),
                        };
                        let tx = c.savepoint().unwrap();
                        let result = tx.execute("UPDATE job_lifecycle SET outcome_kind=?1, signal=?2,core_dumped=?3,spawn_reason=?4,os_code=?5",
                            params![kind, signal, core, spawn, os_code]);
                        if bypass_checks {
                            result.unwrap();
                            if !valid {
                                assert_eq!(
                                    get(&tx, &id(1)),
                                    Err(JobError::StorageFailure),
                                    "{kind:?}/{signal:?}/{core:?}/{spawn:?}/{os_code:?}"
                                );
                            }
                        } else {
                            assert_eq!(
                                result.is_ok(),
                                valid,
                                "{kind:?}/{signal:?}/{core:?}/{spawn:?}/{os_code:?}"
                            );
                        }
                        if valid {
                            accepted += 1;
                        } else {
                            rejected += 1;
                        }
                        // Savepoint Drop rolls back every tuple, including accepted ones.
                    }
                }
            }
        }
    }
    assert_eq!((accepted, rejected), (5, 59));
}

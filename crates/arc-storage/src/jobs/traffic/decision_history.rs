//! Every recorded decision of one job in recorded order, plus its current
//! queue position or expected schedule (lane C, TRF-015). One read
//! transaction; raw reasons, including `start_rate` and fairness reasons.
//!
//! The expected schedule is derived only for a QUEUED job whose last recorded
//! decision is a start-rate hold: the time its class's next token is due,
//! from the same recorded starts admission reads. Any other wait has no
//! derivable time, so none is given.
use super::start_rate;
use crate::jobs::{JobError, JobResult};
use arc_ipc::{
    jobs::JobId,
    traffic::decision_history::{ClassSettings, DecisionHistory, HistoryEntry, PolicySettings},
};
use arc_traffic::{start_rate::CLASSES, Policy, TrafficClass};
use rusqlite::{Connection, OptionalExtension};

fn class_of(tag: &str) -> Option<TrafficClass> {
    serde_json::from_value(serde_json::Value::String(tag.to_owned())).ok()
}

fn settings(connection: &Connection, policy: &Policy) -> JobResult<PolicySettings> {
    Ok(PolicySettings {
        start_rate_control: start_rate::state(connection)?.enabled,
        classes: CLASSES
            .into_iter()
            .map(|class| {
                let (tag, limits) = super::store::policy_class(class, policy);
                let rate = policy.start_rate.class(class);
                ClassSettings {
                    class: tag.to_owned(),
                    window: limits.window,
                    cpu_millis_estimate: limits.reservation.cpu_millis,
                    memory_bytes_estimate: limits.reservation.memory_bytes,
                    start_tokens_per_minute: rate.tokens_per_minute,
                    start_burst: rate.burst,
                }
            })
            .collect(),
    })
}

pub fn read(
    connection: &mut Connection,
    job_id: &JobId,
    policy: &Policy,
    now_ms: u64,
) -> JobResult<DecisionHistory> {
    let tx = connection.transaction()?;
    let mut decisions = Vec::new();
    {
        let mut statement = tx.prepare(
            "SELECT decision,reason_code,coalesce(recorded_at_ms,created_at),transition_key
             FROM policy_decisions WHERE job_id=?1 ORDER BY rowid LIMIT ?2",
        )?;
        let rows = statement.query_map(
            (job_id.as_str(), arc_policy::MAX_JOB_DECISIONS as i64),
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            },
        )?;
        for (index, row) in rows.enumerate() {
            let (kind, reason, at, key) = row?;
            let held = key.is_some_and(|key| key.starts_with(start_rate::MARKER));
            decisions.push(HistoryEntry {
                index: index as u64 + 1,
                decided_at_ms: u64::try_from(at).map_err(|_| JobError::StorageFailure)?,
                kind,
                reason: if held {
                    start_rate::REASON.to_owned()
                } else {
                    reason
                },
            });
        }
    }
    let queued: Option<(i64, String)> = tx
        .query_row(
            "SELECT t.sequence,j.job_class FROM traffic_entries t JOIN jobs j ON j.id=t.job_id
             WHERE t.job_id=?1 AND t.state='QUEUED'",
            [job_id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let (mut queue_position, mut expected_schedule_ms) = (None, None);
    if let Some((sequence, class)) = queued {
        let (position, of): (i64, i64) = tx.query_row(
            "SELECT count(*) FILTER (WHERE sequence<=?1), count(*) FROM traffic_entries
             WHERE state='QUEUED'",
            [sequence],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        queue_position = Some((position as u64, of as u64));
        let held = decisions
            .last()
            .is_some_and(|last| last.reason == start_rate::REASON);
        if let (true, Some(class)) = (held, class_of(&class)) {
            if start_rate::state(&tx)?.enabled {
                expected_schedule_ms =
                    start_rate::next_token(&tx, class, policy.start_rate.class(class), now_ms)?;
            }
        }
    }
    let policy = settings(&tx, policy)?;
    tx.commit()?;
    Ok(DecisionHistory {
        schema_version: arc_ipc::traffic::decision_history::SCHEMA_VERSION,
        job_id: job_id.clone(),
        decisions,
        queue_position,
        expected_schedule_ms,
        policy: Some(policy),
    })
}

use super::*;

// Latest decisions for live work and referenced reservations remain protected.
// Terminal anchors are lifecycle times, never renewed by reads/polls.
pub(super) fn terminal_at(connection: &Connection, id: &JobId) -> JobResult<Option<u64>> {
    let terminal:Option<i64>=connection.query_row(
        "SELECT COALESCE((SELECT created_at FROM audit_events WHERE id='traffic-policy-terminal-'||?1),finished_at) FROM jobs WHERE id=?1",
        [id.as_str()],|r|r.get(0)).optional()?.flatten();
    terminal
        .map(|value| u64::try_from(value).map_err(|_| E::StorageFailure))
        .transpose()
}
pub(super) fn job_expired(connection: &Connection, id: &JobId, now: u64) -> JobResult<bool> {
    let terminal = terminal_at(connection, id)?;
    if terminal.is_none_or(|t| {
        now.checked_sub(t)
            .is_none_or(|age| age < DECISION_RETENTION_MS)
    }) {
        return Ok(false);
    }
    let protected: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM jobs WHERE id=?1 AND state IN ('REGISTERED','LAUNCHING','RUNNING')
             AND NOT EXISTS(SELECT 1 FROM traffic_entries t WHERE t.job_id=?1 AND t.state IN ('CANCELLED','ABANDONED')))
         OR EXISTS(SELECT 1 FROM traffic_entries t WHERE (t.job_id=?1 OR t.root_job_id=?1)
             AND t.state IN ('QUEUED','RESERVED','RUNNING','RETAINED'))
         OR EXISTS(SELECT 1 FROM permits p WHERE p.state='ACTIVE' AND
             (p.job_id=?1 OR p.job_id=(SELECT root_job_id FROM traffic_entries WHERE job_id=?1)))
         OR EXISTS(SELECT 1 FROM jobs j JOIN resource_leases l ON l.session_id=j.session_id
             WHERE j.id=?1 AND l.state='ACTIVE')
         OR EXISTS(SELECT 1 FROM jobs j JOIN job_lifecycle l ON l.job_id=j.id
             WHERE j.id=?1 AND j.state='OUTCOME_UNKNOWN' AND l.observation_status NOT IN ('MISSING','MISMATCH'))",
        [id.as_str()],|r|r.get(0))?;
    Ok(!protected)
}

/// One bounded batch on the maintained writer; no new worker or TTL authority.
/// Remove expired decisions; reads derive expiry from the terminal anchor.
const EXPIRY_SQL: &str = "DELETE FROM policy_decisions WHERE id IN (SELECT d.id FROM policy_decisions d
         WHERE d.expires_at_ms<=?1 AND (d.job_id IS NULL
          OR d.rowid!=(SELECT MAX(latest.rowid) FROM policy_decisions latest WHERE latest.job_id=d.job_id)
          OR (NOT EXISTS(SELECT 1 FROM jobs j WHERE j.id=d.job_id AND j.state IN ('REGISTERED','LAUNCHING','RUNNING')
              AND NOT EXISTS(SELECT 1 FROM traffic_entries t WHERE t.job_id=j.id AND t.state IN ('CANCELLED','ABANDONED')))
           AND NOT EXISTS(SELECT 1 FROM traffic_entries t WHERE (t.job_id=d.job_id OR t.root_job_id=d.job_id) AND t.state IN ('QUEUED','RESERVED','RUNNING','RETAINED'))
           AND NOT EXISTS(SELECT 1 FROM permits p WHERE p.state='ACTIVE' AND (p.job_id=d.job_id OR p.job_id=(SELECT root_job_id FROM traffic_entries WHERE job_id=d.job_id)))
           AND NOT EXISTS(SELECT 1 FROM jobs j JOIN resource_leases l ON l.session_id=j.session_id WHERE j.id=d.job_id AND l.state='ACTIVE')
           AND NOT EXISTS(SELECT 1 FROM jobs j JOIN job_lifecycle l ON l.job_id=j.id WHERE j.id=d.job_id AND j.state='OUTCOME_UNKNOWN' AND l.observation_status NOT IN ('MISSING','MISMATCH'))))
         ORDER BY d.expires_at_ms,d.id LIMIT ?2)";

pub fn expire(connection: &mut Connection, now: u64) -> JobResult<usize> {
    valid_now(now)?;
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let changed = tx.execute(EXPIRY_SQL, params![now as i64, MAX_EXPIRY_BATCH as i64])?;
    tx.commit()?;
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_bounded_expiry_statement_uses_the_registered_expiry_index() {
        let c = Connection::open_in_memory().unwrap();
        for migration in crate::MIGRATIONS {
            c.execute_batch(migration.sql).unwrap();
        }
        let mut statement = c
            .prepare(&format!("EXPLAIN QUERY PLAN {EXPIRY_SQL}"))
            .unwrap();
        let plan: Vec<String> = statement
            .query_map(
                params![DECISION_RETENTION_MS as i64, MAX_EXPIRY_BATCH as i64],
                |r| r.get(3),
            )
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert!(
            plan.iter()
                .any(|step| step.contains("idx_policy_decisions_expiry")),
            "{plan:?}"
        );
    }
}

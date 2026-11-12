use super::{valid_now, Binding, MeterResult};
use arc_core::job_meter::JOB_METADATA_RETENTION_MS;
use rusqlite::{params, Connection, TransactionBehavior};

pub const MAX_EXPIRY_BATCH: usize = 128;

// Dependent reports share their job lifecycle. Retention clears payloads only:
// lifecycle, Traffic, leases, permits and root references remain owned elsewhere.
fn protected(connection: &Connection, id: &str) -> MeterResult<bool> {
    Ok(connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM job_meter_summaries m WHERE m.job_id=?1 AND (
          EXISTS(SELECT 1 FROM jobs j WHERE j.id IN (m.job_id,m.root_job_id)
            AND j.state IN ('REGISTERED','LAUNCHING','RUNNING'))
          OR EXISTS(SELECT 1 FROM permits p WHERE p.job_id IN (m.job_id,m.root_job_id) AND p.state='ACTIVE')
          OR EXISTS(SELECT 1 FROM traffic_entries t WHERE
            (t.job_id IN (m.job_id,m.root_job_id) OR t.root_job_id=m.job_id)
            AND t.state IN ('QUEUED','RESERVED','RUNNING','RETAINED'))
          OR EXISTS(SELECT 1 FROM job_meter_summaries d JOIN jobs j ON j.id=d.job_id
            WHERE d.root_job_id=m.job_id AND j.state IN ('REGISTERED','LAUNCHING','RUNNING'))
          OR EXISTS(SELECT 1 FROM jobs j JOIN resource_leases l ON l.session_id=j.session_id
            WHERE j.id IN (m.job_id,m.root_job_id) AND l.state='ACTIVE')
          OR EXISTS(SELECT 1 FROM job_lifecycle l JOIN jobs j ON j.id=l.job_id
            WHERE j.id IN (m.job_id,m.root_job_id) AND j.state='OUTCOME_UNKNOWN'
              AND l.observation_status NOT IN ('MISSING','MISMATCH'))))",
        [id], |row| row.get(0))?)
}
pub(super) fn is_expired(
    connection: &Connection,
    id: &str,
    binding: &Binding,
    now: u64,
) -> MeterResult<bool> {
    Ok(binding.expired
        || (binding.terminal.is_some_and(|anchor| {
            now.checked_sub(anchor)
                .is_some_and(|age| age >= JOB_METADATA_RETENTION_MS)
        }) && !protected(connection, id)?))
}
pub(super) fn clear(connection: &Connection, id: &str, now: u64) -> MeterResult<()> {
    connection.execute(
        "UPDATE job_meter_summaries SET report_id=NULL,report_json=NULL,
        expired_at_ms=COALESCE(expired_at_ms,?2) WHERE job_id=?1",
        params![id, now as i64],
    )?;
    Ok(())
}

/// Opportunistic bounded maintenance on the existing writer. No TTL input,
/// background service or independent connection. Reads also enforce expiry.
pub fn expire(connection: &mut Connection, now: u64) -> MeterResult<usize> {
    valid_now(now)?;
    let Some(cutoff) = now.checked_sub(JOB_METADATA_RETENTION_MS) else {
        return Ok(0);
    };
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let changed = tx.execute(
        "UPDATE job_meter_summaries SET report_id=NULL,report_json=NULL,expired_at_ms=?2
         WHERE job_id IN (SELECT m.job_id FROM job_meter_summaries m
         WHERE m.expired_at_ms IS NULL AND m.terminal_at_ms<=?1
          AND NOT EXISTS(SELECT 1 FROM jobs j WHERE j.id IN (m.job_id,m.root_job_id)
            AND j.state IN ('REGISTERED','LAUNCHING','RUNNING'))
          AND NOT EXISTS(SELECT 1 FROM permits p WHERE p.job_id IN (m.job_id,m.root_job_id) AND p.state='ACTIVE')
          AND NOT EXISTS(SELECT 1 FROM traffic_entries t WHERE
            (t.job_id IN (m.job_id,m.root_job_id) OR t.root_job_id=m.job_id)
            AND t.state IN ('QUEUED','RESERVED','RUNNING','RETAINED'))
          AND NOT EXISTS(SELECT 1 FROM job_meter_summaries d JOIN jobs j ON j.id=d.job_id
            WHERE d.root_job_id=m.job_id AND j.state IN ('REGISTERED','LAUNCHING','RUNNING'))
          AND NOT EXISTS(SELECT 1 FROM jobs j JOIN resource_leases l ON l.session_id=j.session_id
            WHERE j.id IN (m.job_id,m.root_job_id) AND l.state='ACTIVE')
          AND NOT EXISTS(SELECT 1 FROM job_lifecycle l JOIN jobs j ON j.id=l.job_id
            WHERE j.id IN (m.job_id,m.root_job_id) AND j.state='OUTCOME_UNKNOWN'
              AND l.observation_status NOT IN ('MISSING','MISMATCH'))
         ORDER BY m.terminal_at_ms,m.job_id LIMIT ?3)",
        params![cutoff as i64, now as i64, MAX_EXPIRY_BATCH as i64],
    )?;
    tx.commit()?;
    Ok(changed)
}

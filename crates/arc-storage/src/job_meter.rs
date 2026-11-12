//! Job-keyed terminal metadata, always called by the daemon's single writer.
//! Policy lives here, not in SQL triggers. Unknown placeholders have no report
//! identity. JSON is the bounded, immutable wire document, not arbitrary input.
mod lineage;
mod retention;
#[cfg(test)]
mod tests;

use arc_core::job_meter::*;
use arc_ipc::{
    jobs::{JobId, JobSnapshot, ObservationStatus, Phase, MAX_TIMESTAMP},
    meter::{matches_terminal_outcome, ReportDisposition, ReportReceipt},
};
pub use lineage::LineageEvidence;
pub use retention::{expire, MAX_EXPIRY_BATCH};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use std::{error::Error, fmt};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeterError {
    InvalidRequest,
    Conflict,
    IdentityMismatch,
    NotTerminal,
    Unavailable,
    Busy,
}
impl MeterError {
    pub fn code(self) -> &'static str {
        match self {
            Self::InvalidRequest => arc_ipc::ERROR_INVALID_REQUEST,
            Self::Conflict => arc_ipc::meter::ERROR_METER_REPORT_CONFLICT,
            Self::IdentityMismatch => arc_ipc::meter::ERROR_METER_IDENTITY_MISMATCH,
            Self::NotTerminal => arc_ipc::meter::ERROR_METER_NOT_TERMINAL,
            Self::Unavailable => arc_ipc::meter::ERROR_METER_UNAVAILABLE,
            Self::Busy => arc_ipc::ERROR_DB_BUSY,
        }
    }
}
impl fmt::Display for MeterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}
impl Error for MeterError {}
impl From<rusqlite::Error> for MeterError {
    fn from(_: rusqlite::Error) -> Self {
        Self::Busy
    }
}
impl From<crate::jobs::JobError> for MeterError {
    fn from(_: crate::jobs::JobError) -> Self {
        Self::Busy
    }
}
pub type MeterResult<T> = Result<T, MeterError>;

struct Binding {
    aware: bool,
    relation: String,
    root: Option<String>,
    child_bound: bool,
    terminal: Option<u64>,
    expired: bool,
    report_json: Option<String>,
}
fn binding(connection: &Connection, id: &str) -> MeterResult<Option<Binding>> {
    Ok(connection
        .query_row(
            "SELECT measurement_aware,relation,root_job_id,child_bound,terminal_at_ms,
         expired_at_ms IS NOT NULL,report_json FROM job_meter_summaries WHERE job_id=?1",
            [id],
            |row| {
                Ok(Binding {
                    aware: row.get(0)?,
                    relation: row.get(1)?,
                    root: row.get(2)?,
                    child_bound: row.get(3)?,
                    terminal: row.get::<_, Option<i64>>(4)?.map(|n| n as u64),
                    expired: row.get(5)?,
                    report_json: row.get(6)?,
                })
            },
        )
        .optional()?)
}
fn valid_now(now: u64) -> MeterResult<()> {
    if now > MAX_TIMESTAMP {
        Err(MeterError::InvalidRequest)
    } else {
        Ok(())
    }
}

/// Invoked in the lifecycle transition's existing IMMEDIATE transaction.
/// COALESCE preserves the first terminalization even if later recovery completes
/// an interrupted record. A future observation never renews retention.
pub(crate) fn track(connection: &Connection, job: &JobSnapshot, new: bool) -> rusqlite::Result<()> {
    let anchor = job.completed_at_ms.or_else(|| {
        (job.phase == Phase::OutcomeUnknown
            && matches!(
                job.observation.status,
                ObservationStatus::Missing | ObservationStatus::Mismatch
            ))
        .then_some(job.updated_at_ms)
    });
    connection.execute(
        "INSERT INTO job_meter_summaries(job_id,measurement_aware,terminal_at_ms)
         VALUES(?1,?2,?3) ON CONFLICT(job_id) DO UPDATE SET
         terminal_at_ms=COALESCE(job_meter_summaries.terminal_at_ms,excluded.terminal_at_ms)",
        params![job.job_id.as_str(), new, anchor.map(|n| n as i64)],
    )?;
    Ok(())
}

/// Bind native evidence during registration/attachment, never from a terminal
/// report's assertion. The caller gathers it before taking a database ticket.
pub(crate) fn bind(
    connection: &Connection,
    job: &JobSnapshot,
    proof: &LineageEvidence,
) -> crate::jobs::JobResult<()> {
    lineage::bind(connection, job, proof)
}

pub fn report(
    connection: &mut Connection,
    report: &JobMeterReport,
    now: u64,
) -> MeterResult<ReportReceipt> {
    valid_now(now)?;
    if !report.is_valid() {
        return Err(MeterError::InvalidRequest);
    }
    let json = serde_json::to_string(report).map_err(|_| MeterError::InvalidRequest)?;
    if json.len() > MAX_JOB_METER_BYTES {
        return Err(MeterError::InvalidRequest);
    }
    let id = JobId::try_from(report.job_id.clone()).map_err(|_| MeterError::InvalidRequest)?;
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let job = crate::jobs::get(&tx, &id)?.ok_or(MeterError::IdentityMismatch)?;
    let b = binding(&tx, &report.job_id)?.ok_or(MeterError::IdentityMismatch)?;
    // Do not probe the reaped child. Only the previously verified durable tuple
    // and owned-child binding establish its identity at this boundary.
    if job.observer != report.observer
        || job.child.as_ref() != Some(&report.child)
        || job.child_verified_at_ms.is_none()
        || !b.child_bound
    {
        return Err(MeterError::IdentityMismatch);
    }
    let Some(outcome) = &job.outcome else {
        return Err(MeterError::NotTerminal);
    };
    if !matches_terminal_outcome(report, outcome) {
        return Err(MeterError::Conflict);
    }
    if retention::is_expired(&tx, &report.job_id, &b, now)? {
        return Err(MeterError::Unavailable);
    }
    if !lineage::matches(&b, &report.accounting_relation, &report.job_id) {
        return Err(MeterError::IdentityMismatch);
    }
    let disposition = if let Some(prior) = b.report_json {
        let prior = arc_ipc::meter::decode_report(
            &serde_json::from_str(&prior).map_err(|_| MeterError::Busy)?,
        )
        .map_err(|_| MeterError::Busy)?;
        if prior != *report {
            return Err(MeterError::Conflict);
        }
        ReportDisposition::Unchanged
    } else {
        if b.terminal.is_none() {
            return Err(MeterError::NotTerminal);
        }
        let reused: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM job_meter_summaries WHERE report_id=?1)",
            [&report.report_id],
            |r| r.get(0),
        )?;
        if reused {
            return Err(MeterError::Conflict);
        }
        tx.execute(
            "UPDATE job_meter_summaries SET report_id=?2,report_json=?3
            WHERE job_id=?1 AND report_id IS NULL AND expired_at_ms IS NULL",
            params![report.job_id, report.report_id, json],
        )?;
        ReportDisposition::Accepted
    };
    tx.commit()?;
    Ok(ReportReceipt {
        schema_version: JOB_METER_SCHEMA_VERSION,
        job_id: report.job_id.clone(),
        report_id: report.report_id.clone(),
        disposition,
    })
}

fn queue(connection: &Connection, id: &str, aware: bool) -> MeterResult<QueueWait> {
    // A pre-meter row's absence of scheduling data proves no measured zero.
    if !aware {
        return Ok(QueueWait::unknown());
    }
    let timestamps = connection
        .query_row(
            "SELECT queued_at_ms,granted_at_ms FROM traffic_entries WHERE job_id=?1",
            [id],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, Option<i64>>(1)?)),
        )
        .optional()?;
    Ok(match timestamps {
        Some((queued, reserved)) => QueueWait::from_wall_timestamps(
            queued.try_into().ok(),
            reserved.and_then(|n| n.try_into().ok()),
        ),
        None => QueueWait::lifecycle_only(),
    })
}

pub fn summary(connection: &mut Connection, id: &str, now: u64) -> MeterResult<JobMeterSummary> {
    valid_now(now)?;
    if !valid_id(id) {
        return Err(MeterError::InvalidRequest);
    }
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let mut queue_wait = QueueWait::unknown();
    let accounting = match binding(&tx, id)? {
        None => {
            let exists: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM jobs WHERE id=?1)",
                [id],
                |row| row.get(0),
            )?;
            if exists {
                SummaryAccounting::Unknown {
                    reason: unknown_reason(&tx, id, false)?,
                }
            } else {
                SummaryAccounting::Missing {}
            }
        }
        Some(b) if retention::is_expired(&tx, id, &b, now)? => {
            retention::clear(&tx, id, now)?;
            SummaryAccounting::Expired {}
        }
        Some(b) => {
            queue_wait = queue(&tx, id, b.aware)?;
            if let Some(json) = b.report_json {
                let report = arc_ipc::meter::decode_report(
                    &serde_json::from_str(&json).map_err(|_| MeterError::Busy)?,
                )
                .map_err(|_| MeterError::Busy)?;
                if report.job_id != id {
                    return Err(MeterError::Busy);
                }
                SummaryAccounting::Recorded {
                    report: Box::new(report),
                }
            } else {
                SummaryAccounting::Unknown {
                    reason: unknown_reason(&tx, id, b.aware)?,
                }
            }
        }
    };
    let result = JobMeterSummary {
        schema_version: JOB_METER_SCHEMA_VERSION,
        job_id: id.to_owned(),
        accounting,
        queue_wait,
    };
    if !result.is_valid() {
        return Err(MeterError::Busy);
    }
    tx.commit()?;
    Ok(result)
}

fn unknown_reason(connection: &Connection, id: &str, aware: bool) -> MeterResult<UnknownReason> {
    let (state, interrupted): (String, bool) = connection.query_row(
        "SELECT j.state,COALESCE(l.observation_status IN ('MISSING','MISMATCH'),0)
         FROM jobs j LEFT JOIN job_lifecycle l ON l.job_id=j.id WHERE j.id=?1",
        [id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    Ok(
        if interrupted
            && matches!(
                state.as_str(),
                "REGISTERED" | "LAUNCHING" | "RUNNING" | "OUTCOME_UNKNOWN"
            )
        {
            UnknownReason::Interrupted
        } else if matches!(
            state.as_str(),
            "REGISTERED" | "LAUNCHING" | "RUNNING" | "OUTCOME_UNKNOWN"
        ) {
            UnknownReason::NotTerminated
        } else if !aware {
            UnknownReason::Legacy
        } else {
            UnknownReason::Unavailable
        },
    )
}

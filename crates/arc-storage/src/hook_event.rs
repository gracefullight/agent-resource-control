//! Vendor hook event identity records (AT-TRF-09, ADR-014).
//!
//! The key of an event's first delivery is `job_lifecycle.event_key`
//! (migration `0008_hook_event`): unique among recorded keys, written in the
//! admission transaction that creates the job row and deleted with that row.
//! Each later delivery of a recorded key is one `audit_events` row on the
//! existing job, in the pattern of `crate::request`: no migration, and no
//! key, command, path or environment text.
//!
//! Both functions run inside the caller's admission transaction, so of two
//! registrations of one key exactly one creates a job row and the other is
//! answered as a duplicate; the partial UNIQUE index is the backstop.
use crate::jobs::{JobError, JobResult};
use arc_ipc::{
    jobs::{JobId, MAX_TIMESTAMP},
    traffic::hook_event::EventKey,
};
use rusqlite::{params, Connection, OptionalExtension};

/// `audit_events.event_type` of one duplicate delivery; `severity` is `INFO`
/// and `payload_redacted_json` is NULL.
pub const DUPLICATE_EVENT: &str = "HOOK_EVENT_DUPLICATE";
/// `audit_events.id` of a duplicate delivery is this prefix, the existing
/// job id, `-` and the registration id of the delivering run; `entity_id` is
/// the existing job id. A registration retried after a lost reply repeats
/// its id, so one delivery is recorded once.
pub const DUPLICATE_PREFIX: &str = "hook-event-duplicate-";

/// Record `key` on the job row this transaction just created. An absent key
/// leaves the column NULL, as for every client that sends none.
pub(crate) fn record(tx: &Connection, job: &JobId, key: Option<&EventKey>) -> JobResult<()> {
    let Some(key) = key else {
        return Ok(());
    };
    if tx.is_autocommit() {
        return Err(JobError::InvalidTransition);
    }
    let stored = tx.execute(
        "UPDATE job_lifecycle SET event_key=?2 WHERE job_id=?1 AND event_key IS NULL",
        params![job.as_str(), key.as_str()],
    )?;
    if stored == 1 {
        Ok(())
    } else {
        Err(JobError::StorageFailure)
    }
}

/// The job that already recorded `key`, after recording this registration
/// (`delivery`) as one duplicate delivery on it. `None` when the request
/// carries no key or the key is new: the caller then admits as before.
pub(crate) fn duplicate(
    tx: &Connection,
    delivery: &JobId,
    key: Option<&EventKey>,
    now: u64,
) -> JobResult<Option<JobId>> {
    let Some(key) = key else {
        return Ok(None);
    };
    if tx.is_autocommit() {
        return Err(JobError::InvalidTransition);
    }
    let existing: Option<String> = tx
        .query_row(
            "SELECT job_id FROM job_lifecycle WHERE event_key=?1",
            [key.as_str()],
            |row| row.get(0),
        )
        .optional()?;
    let Some(existing) = existing else {
        return Ok(None);
    };
    let existing = JobId::try_from(existing).map_err(|_| JobError::StorageFailure)?;
    if now > MAX_TIMESTAMP {
        return Err(JobError::InvalidRequest);
    }
    tx.execute(
        "INSERT OR IGNORE INTO audit_events(id,severity,event_type,entity_id,payload_redacted_json,created_at)
         VALUES(?1,'INFO',?2,?3,NULL,?4)",
        params![
            format!("{DUPLICATE_PREFIX}{}-{}", existing.as_str(), delivery.as_str()),
            DUPLICATE_EVENT,
            existing.as_str(),
            now as i64
        ],
    )?;
    Ok(Some(existing))
}

/// How many duplicate deliveries are recorded on `job`.
pub fn duplicates(connection: &Connection, job: &JobId) -> JobResult<u64> {
    let count: i64 = connection.query_row(
        "SELECT count(*) FROM audit_events WHERE event_type=?1 AND entity_id=?2",
        params![DUPLICATE_EVENT, job.as_str()],
        |row| row.get(0),
    )?;
    u64::try_from(count).map_err(|_| JobError::StorageFailure)
}

use super::{get, JobError, JobResult};
use arc_ipc::{
    jobs::{JobId, JobSnapshot, ProcessIdentity},
    traffic::{JobResult as Reply, LineageContext, Resources, StatusResult, WaitReason},
};
use arc_traffic::{Active, Policy, TrafficClass};
use rusqlite::{Connection, OptionalExtension};

#[derive(Clone)]
pub(super) struct Entry {
    pub sequence: u64,
    pub state: String,
    pub queued_at: u64,
    pub granted_at: Option<u64>,
    pub generation: Option<u64>,
    pub root: Option<JobId>,
    pub context: Option<LineageContext>,
    pub inherited: Option<LineageContext>,
}

pub(super) fn tombstone(
    connection: &Connection,
    id: &JobId,
    observer: &ProcessIdentity,
) -> JobResult<Option<Reply>> {
    let row: Option<(u32, String, String, String)> = connection
        .query_row(
            "SELECT observer_pid,observer_start_token,observer_boot_id,terminal_state FROM traffic_tombstones WHERE job_id=?1",
            [id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?;
    match row {
        None => Ok(None),
        Some((pid, token, boot, _))
            if pid != observer.pid || token != observer.start_token || boot != observer.boot_id =>
        {
            Err(JobError::Conflict)
        }
        Some((_, _, _, state)) => Ok(Some(terminal_reply(&state)?)),
    }
}

pub(super) fn put_tombstone(
    connection: &Connection,
    id: &JobId,
    observer: &ProcessIdentity,
    state: &str,
    now: u64,
) -> JobResult<Reply> {
    if let Some(existing) = tombstone(connection, id, observer)? {
        return Ok(existing);
    }
    let collision: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM jobs WHERE id=?1)",
        [id.as_str()],
        |row| row.get(0),
    )?;
    if collision {
        return Err(JobError::Conflict);
    }
    let reply = terminal_reply(state)?;
    connection.execute(
        "INSERT INTO traffic_tombstones VALUES (?1,?2,?3,?4,?5,?6)",
        rusqlite::params![
            id.as_str(),
            observer.pid,
            observer.start_token,
            observer.boot_id,
            state,
            now as i64
        ],
    )?;
    Ok(reply)
}

fn terminal_reply(state: &str) -> JobResult<Reply> {
    match state {
        "CANCELLED" => Ok(Reply::Cancelled {}),
        "ABANDONED" => Ok(Reply::Abandoned {}),
        _ => Err(JobError::StorageFailure),
    }
}

pub(super) fn entry(connection: &Connection, id: &JobId) -> JobResult<Option<Entry>> {
    let raw = connection.query_row(
        "SELECT sequence,state,queued_at_ms,granted_at_ms,generation,root_job_id,context,inherited_context FROM traffic_entries WHERE job_id=?1",
        [id.as_str()], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, i64>(2)?, row.get::<_, Option<i64>>(3)?, row.get::<_, Option<i64>>(4)?, row.get::<_, Option<String>>(5)?, row.get::<_, Option<String>>(6)?, row.get::<_, Option<String>>(7)?)),
    ).optional()?;
    raw.map(
        |(seq, state, queued, granted, generation, root, context, inherited)| {
            let convert = |n: i64| u64::try_from(n).map_err(|_| JobError::StorageFailure);
            Ok(Entry {
                sequence: convert(seq)?,
                state,
                queued_at: convert(queued)?,
                granted_at: granted.map(convert).transpose()?,
                generation: generation.map(convert).transpose()?,
                root: root
                    .map(|v| JobId::try_from(v).map_err(|_| JobError::StorageFailure))
                    .transpose()?,
                context: context
                    .map(|v| LineageContext::try_from(v).map_err(|_| JobError::StorageFailure))
                    .transpose()?,
                inherited: inherited
                    .map(|v| LineageContext::try_from(v).map_err(|_| JobError::StorageFailure))
                    .transpose()?,
            })
        },
    )
    .transpose()
}

pub(super) fn active(connection: &Connection) -> JobResult<Active> {
    let mut result = Active::default();
    // Include every active permit, not only rows created by this version. An
    // older active permit without exact estimates makes admission uncertain;
    // reject the transaction rather than silently spend its capacity twice.
    let mut stmt = connection.prepare("SELECT j.job_class,p.cpu_millis_est,p.memory_bytes_est FROM permits p JOIN jobs j ON j.id=p.job_id WHERE p.state='ACTIVE'")?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let class: String = row.get(0)?;
        let cpu = u64::try_from(row.get::<_, i64>(1)?).map_err(|_| JobError::StorageFailure)?;
        let memory = u64::try_from(row.get::<_, i64>(2)?).map_err(|_| JobError::StorageFailure)?;
        if cpu == 0 || memory == 0 {
            return Err(JobError::StorageFailure);
        }
        let count = match class.as_str() {
            "BUILD" => &mut result.build,
            "TEST" => &mut result.test,
            "LINT" => &mut result.lint,
            _ => return Err(JobError::StorageFailure),
        };
        *count = count.checked_add(1).ok_or(JobError::StorageFailure)?;
        result.resources.cpu_millis = result
            .resources
            .cpu_millis
            .checked_add(cpu)
            .ok_or(JobError::StorageFailure)?;
        result.resources.memory_bytes = result
            .resources
            .memory_bytes
            .checked_add(memory)
            .ok_or(JobError::StorageFailure)?;
    }
    Ok(result)
}

pub(super) fn reservation(connection: &Connection, id: &JobId, e: &Entry) -> JobResult<Resources> {
    let root = e.root.as_ref().unwrap_or(id);
    let (cpu, memory): (i64, i64) = connection.query_row(
        "SELECT cpu_millis_est,memory_bytes_est FROM permits WHERE job_id=?1 AND generation=?2",
        (
            root.as_str(),
            e.generation.ok_or(JobError::StorageFailure)? as i64,
        ),
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    Ok(Resources {
        cpu_millis: u64::try_from(cpu).map_err(|_| JobError::StorageFailure)?,
        memory_bytes: u64::try_from(memory).map_err(|_| JobError::StorageFailure)?,
    })
}

pub(super) fn result(
    connection: &Connection,
    id: &JobId,
    job: &JobSnapshot,
    e: &Entry,
    wait: WaitReason,
) -> JobResult<Reply> {
    Ok(match e.state.as_str() {
        "QUEUED" => Reply::Queued {
            sequence: e.sequence,
            queued_at_ms: e.queued_at,
            reason: wait,
        },
        "RESERVED" if e.root.is_some() => Reply::Reused {
            parent_job_id: e.root.clone().ok_or(JobError::StorageFailure)?,
            permit_generation: e.generation.ok_or(JobError::StorageFailure)?,
        },
        "RESERVED" => Reply::Granted {
            sequence: e.sequence,
            queued_at_ms: e.queued_at,
            granted_at_ms: e.granted_at.ok_or(JobError::StorageFailure)?,
            permit_generation: e.generation.ok_or(JobError::StorageFailure)?,
            reservation: reservation(connection, id, e)?,
            lineage_context: e.context.clone().ok_or(JobError::StorageFailure)?,
        },
        "RUNNING" => Reply::Running {},
        "RETAINED" => Reply::ReservationRetained {},
        // A completed historical entry cannot answer an owner retry without
        // its outcome. Reject the transition rather than retrying DB_BUSY.
        "COMPLETED"
            if job.outcome.is_none() && crate::jobs::codec::legacy_unknown(connection, id)? =>
        {
            return Err(JobError::InvalidTransition);
        }
        "COMPLETED" => Reply::Completed {
            queue_wait_ms: grant_time(connection, id, e)?
                .checked_sub(e.queued_at)
                .ok_or(JobError::StorageFailure)?,
            outcome: job.outcome.clone().ok_or(JobError::StorageFailure)?,
        },
        "CANCELLED" => Reply::Cancelled {},
        "ABANDONED" => Reply::Abandoned {},
        _ => return Err(JobError::StorageFailure),
    })
}

fn grant_time(connection: &Connection, id: &JobId, e: &Entry) -> JobResult<u64> {
    if let Some(at) = e.granted_at {
        return Ok(at);
    }
    Ok(connection.query_row(
        "SELECT unreserved_at_ms FROM governor_jobs WHERE job_id=?1 AND mode='UNRESERVED'",
        [id.as_str()],
        |r| r.get::<_, i64>(0),
    )? as u64)
}

/// A status never contains the opaque lineage context.
pub fn status(connection: &Connection, id: &JobId) -> JobResult<StatusResult> {
    let Some(e) = entry(connection, id)? else {
        let state: Option<String> = connection
            .query_row(
                "SELECT terminal_state FROM traffic_tombstones WHERE job_id=?1",
                [id.as_str()],
                |r| r.get(0),
            )
            .optional()?;
        return match state.as_deref() {
            Some("CANCELLED") => Ok(StatusResult::Cancelled {}),
            Some("ABANDONED") => Ok(StatusResult::Abandoned {}),
            None => Ok(StatusResult::NotFound {}),
            _ => Err(JobError::StorageFailure),
        };
    };
    let job = get(connection, id)?.ok_or(JobError::StorageFailure)?;
    Ok(match e.state.as_str() {
        "QUEUED" => StatusResult::Queued {
            sequence: e.sequence,
            queued_at_ms: e.queued_at,
        },
        "RESERVED" | "RETAINED" if e.generation.is_none() => StatusResult::Unconfirmed {},
        "RESERVED" | "RETAINED" => StatusResult::Reserved {
            sequence: e.sequence,
            queued_at_ms: e.queued_at,
            granted_at_ms: e.granted_at.ok_or(JobError::StorageFailure)?,
            reservation: reservation(connection, id, &e)?,
        },
        "RUNNING" => StatusResult::Running {
            queue_wait_ms: grant_time(connection, id, &e)?
                .checked_sub(e.queued_at)
                .ok_or(JobError::StorageFailure)?,
        },
        // Protocol-1 Traffic COMPLETED requires an outcome, so direct the
        // caller to the existing lifecycle status for this historical row.
        "COMPLETED"
            if job.outcome.is_none() && crate::jobs::codec::legacy_unknown(connection, id)? =>
        {
            return Err(JobError::UnsupportedStatus);
        }
        "COMPLETED" => StatusResult::Completed {
            queue_wait_ms: grant_time(connection, id, &e)?
                .checked_sub(e.queued_at)
                .ok_or(JobError::StorageFailure)?,
            outcome: job.outcome.ok_or(JobError::StorageFailure)?,
        },
        "CANCELLED" => StatusResult::Cancelled {},
        "ABANDONED" => StatusResult::Abandoned {},
        _ => return Err(JobError::StorageFailure),
    })
}

pub(super) fn policy_class(
    class: TrafficClass,
    policy: &Policy,
) -> (&'static str, arc_traffic::ClassPolicy) {
    (
        match class {
            TrafficClass::Build => "BUILD",
            TrafficClass::Test => "TEST",
            TrafficClass::Lint => "LINT",
        },
        policy.class(class),
    )
}

//! Start-rate admission under the existing windows and reservations (lane C,
//! DEC-013, POL-001). Called once from admission after the window and budget
//! decision admitted a QUEUED entry and before its permit is written. A
//! returned reply keeps the entry QUEUED; v1 replies project the wait onto
//! `wait`/`fifo` and the raw `start_rate` reason is recorded for
//! `decision_history` only.
//!
//! The bucket keeps no state of its own: the recorded starts of a class are
//! the `reserved_at` times of its permits, written in the same admission
//! transaction as each grant, so the bucket survives a restart and a held or
//! rolled-back grant never spends a token. A held entry is granted by the
//! next poll its waiter sends on its existing cadence once a token is due;
//! that poll re-runs this evaluation in its own admission transaction, so the
//! one permit is written exactly once.
//!
//! A held decision is stored as the existing pair `wait`/`fifo`, which every
//! v1 reader accepts. Its `transition_key` starts with [`MARKER`], which only
//! `decision_history` reads back as the raw reason `start_rate`.
//!
//! The switch `traffic.start_rate_control` is the `policy_states` row of that
//! key; a missing row is the default ON at revision 0.
use crate::jobs::{JobError, JobResult};
use arc_ipc::{
    governor::{
        GetGovernorSwitchRequest, GovernorConflictReason, GovernorMutationResult,
        GovernorRejectionReason, GovernorSwitchKey, GovernorSwitchReadResult, GovernorSwitchState,
        SetGovernorSwitchRequest, MAX_SAFE_INTEGER, START_RATE_SWITCH_KEY,
    },
    jobs::JobSnapshot,
    traffic::{JobResult as Reply, WaitReason},
};
use arc_policy::{EffectivePolicy, MAX_JOB_DECISIONS};
use arc_traffic::{start_rate::StartRate, Policy, TrafficClass};
use rusqlite::{params, Connection, OptionalExtension, Transaction};

/// Prefix of the `transition_key` of a recorded start-rate hold.
pub(crate) const MARKER: &str = "[\"start_rate\"";
/// Raw reason of a recorded start-rate hold, read only by `decision_history`.
pub const REASON: &str = "start_rate";
/// Only the most recent permits are read; older starts cannot be inside the
/// lookback of a bucket whose burst is at most `MAX_BURST` per class.
const MAX_RECENT_PERMITS: i64 = 4 * arc_traffic::start_rate::MAX_BURST as i64;

pub(crate) struct Candidate<'a> {
    pub job: &'a JobSnapshot,
    pub sequence: u64,
    pub queued_at_ms: u64,
    pub class: TrafficClass,
    pub policy: &'a Policy,
    pub effective: &'a EffectivePolicy,
    pub now_ms: u64,
}

/// `Some(reply)` holds the entry QUEUED; `None` lets the grant proceed.
pub(crate) fn hold(connection: &Connection, candidate: &Candidate<'_>) -> JobResult<Option<Reply>> {
    if !state(connection)?.enabled {
        return Ok(None);
    }
    let rate = candidate.policy.start_rate.class(candidate.class);
    match next_token(connection, candidate.class, rate, candidate.now_ms)? {
        Some(at) if at > candidate.now_ms => (),
        _ => return Ok(None),
    }
    record(connection, candidate)?;
    Ok(Some(Reply::Queued {
        sequence: candidate.sequence,
        queued_at_ms: candidate.queued_at_ms,
        reason: WaitReason::Fifo,
    }))
}

/// The earliest start time of the next token of `class`; `None` when the
/// class is unlimited. A time at or before `now_ms` means a token is free.
pub(crate) fn next_token(
    connection: &Connection,
    class: TrafficClass,
    rate: StartRate,
    now_ms: u64,
) -> JobResult<Option<u64>> {
    let Some(lookback) = rate.lookback_ms() else {
        return Ok(None);
    };
    let (tag, _) = super::store::policy_class(class, &Policy::default());
    let horizon = now_ms.saturating_sub(lookback);
    let mut statement = connection.prepare_cached(
        "SELECT p.reserved_at FROM permits p JOIN jobs j ON j.id=p.job_id
         WHERE p.rowid>(SELECT coalesce(max(rowid),0) FROM permits)-?1
           AND j.job_class=?2 AND p.reserved_at>?3 ORDER BY p.reserved_at,p.rowid",
    )?;
    let starts = statement
        .query_map(
            params![MAX_RECENT_PERMITS, tag, horizon.min(i64::MAX as u64) as i64],
            |row| row.get::<_, i64>(0),
        )?
        .map(|start| u64::try_from(start?).map_err(|_| JobError::StorageFailure))
        .collect::<JobResult<Vec<_>>>()?;
    Ok(rate.next_token_ms(&starts, now_ms))
}

fn transition_key(candidate: &Candidate<'_>) -> JobResult<String> {
    let policy = candidate.effective;
    let key = serde_json::to_string(&(
        REASON,
        &policy.policy_version,
        &policy.profile,
        policy.mode,
        policy.reason,
        candidate.class,
        policy.observation.availability,
        &policy.observation.sampler,
    ))
    .map_err(|_| JobError::StorageFailure)?;
    (key.starts_with(MARKER) && key.len() <= 512)
        .then_some(key)
        .ok_or(JobError::InvalidRequest)
}

/// Record one hold as `wait`/`fifo` with the start-rate transition key. A
/// repeated hold under unchanged conditions records nothing new, exactly as
/// `policy::record` de-duplicates a transition. The entry is QUEUED, so it
/// has no terminal anchor and its decision has no expiry yet.
fn record(connection: &Connection, candidate: &Candidate<'_>) -> JobResult<()> {
    let policy = candidate.effective;
    crate::policy::record_effective(connection, policy, candidate.now_ms)?;
    let key = transition_key(candidate)?;
    let job = candidate.job.job_id.as_str();
    let last: Option<String> = connection
        .query_row(
            "SELECT transition_key FROM policy_decisions WHERE job_id=?1 ORDER BY rowid DESC LIMIT 1",
            [job],
            |row| row.get(0),
        )
        .optional()?
        .flatten();
    if last.as_deref() == Some(key.as_str()) {
        return Ok(());
    }
    let availability = json(&policy.observation.availability)?;
    connection.execute(
        "INSERT INTO policy_decisions(id,job_id,policy_version,decision,reason_code,created_at,
         observation_availability,observation_age_ms,observation_json,transition_key,
         permit_generation,recorded_at_ms,expires_at_ms)
         VALUES(lower(hex(randomblob(16))),?1,?2,'wait','fifo',?3,?4,?5,?6,?7,NULL,?3,NULL)",
        params![
            job,
            policy.policy_version,
            candidate.now_ms as i64,
            availability.trim_matches('"'),
            policy.observation.age_ms.map(|n| n as i64),
            json(&policy.observation)?,
            key,
        ],
    )?;
    connection.execute(
        "DELETE FROM policy_decisions WHERE id IN (SELECT id FROM policy_decisions
         WHERE job_id=?1 ORDER BY rowid DESC LIMIT ?2 OFFSET ?3)",
        params![
            job,
            arc_policy::MAX_EXPIRY_BATCH as i64,
            MAX_JOB_DECISIONS as i64
        ],
    )?;
    Ok(())
}

fn json<T: serde::Serialize>(value: &T) -> JobResult<String> {
    serde_json::to_string(value).map_err(|_| JobError::StorageFailure)
}

/// The current switch; a missing row is the default ON at revision 0.
pub fn state(connection: &Connection) -> JobResult<GovernorSwitchState> {
    let raw: Option<(i64, i64, i64)> = connection
        .query_row(
            "SELECT enabled,policy_revision,updated_at FROM policy_states WHERE key=?1",
            [START_RATE_SWITCH_KEY],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let Some((enabled, revision, updated)) = raw else {
        return Ok(GovernorSwitchState {
            key: GovernorSwitchKey::StartRateControl,
            ..GovernorSwitchState::default()
        });
    };
    let state = GovernorSwitchState {
        key: GovernorSwitchKey::StartRateControl,
        enabled: match enabled {
            0 => false,
            1 => true,
            _ => return Err(JobError::StorageFailure),
        },
        revision: u64::try_from(revision).map_err(|_| JobError::StorageFailure)?,
        updated_at_ms: Some(u64::try_from(updated).map_err(|_| JobError::StorageFailure)?),
    };
    state
        .is_valid()
        .then_some(state)
        .ok_or(JobError::StorageFailure)
}

/// `None` for the Governor switch (handled by `governor::set_switch`). A
/// compare-and-set on the expected revision; no receipt is kept, so a lost
/// reply is resolved by reading the switch back (`policy.get_switch`).
pub(crate) fn set_switch(
    tx: &Transaction<'_>,
    request: &SetGovernorSwitchRequest,
    now_ms: u64,
) -> Option<JobResult<GovernorMutationResult>> {
    (request.key == GovernorSwitchKey::StartRateControl).then(|| {
        let before = state(tx)?;
        let id = request.mutation_id.clone();
        if before.revision != request.expected_revision {
            return Ok(GovernorMutationResult::Conflict {
                mutation_id: id,
                switch: before,
                reason: GovernorConflictReason::RevisionConflict,
            });
        }
        let changed = before.enabled != request.enabled;
        if !changed {
            return Ok(GovernorMutationResult::Committed {
                mutation_id: id,
                switch: before,
                changed,
                replayed: false,
            });
        }
        if before.revision == MAX_SAFE_INTEGER {
            return Ok(GovernorMutationResult::Rejected {
                mutation_id: id,
                switch: before,
                reason: GovernorRejectionReason::RevisionExhausted,
            });
        }
        let at = now_ms.max(before.updated_at_ms.unwrap_or(0));
        tx.execute(
            "INSERT INTO policy_states(key,enabled,policy_revision,policy_version,updated_at)
             VALUES(?1,?2,?3,'traffic.start_rate.v1',?4) ON CONFLICT(key) DO UPDATE SET
             enabled=excluded.enabled,policy_revision=excluded.policy_revision,
             policy_version=excluded.policy_version,updated_at=excluded.updated_at",
            params![
                START_RATE_SWITCH_KEY,
                request.enabled,
                (before.revision + 1) as i64,
                at as i64
            ],
        )?;
        let after = state(tx)?;
        crate::policy::change_audit::switch(tx, &before, &after, at)?;
        Ok(GovernorMutationResult::Committed {
            mutation_id: id,
            switch: after,
            changed,
            replayed: false,
        })
    })
}

/// `None` for the Governor switch (handled by `governor::read_switch`). No
/// receipt exists for this switch, so a mutation read-back reports none.
pub(crate) fn read_switch(
    connection: &Connection,
    query: &GetGovernorSwitchRequest,
) -> Option<JobResult<GovernorSwitchReadResult>> {
    (query.key == GovernorSwitchKey::StartRateControl).then(|| {
        Ok(GovernorSwitchReadResult {
            switch: state(connection)?,
            mutation_result: None,
        })
    })
}

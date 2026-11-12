//! Application-owned policy repository on the existing SQLite writer.
use crate::jobs::{JobError as E, JobResult};
use arc_ipc::{
    jobs::JobId,
    policy::{DecisionAvailability, JobPolicyResult},
};
use arc_policy::*;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

mod retention;
pub use arc_policy::{RecordedDecision, DECISION_RETENTION_MS, MAX_EXPIRY_BATCH, POLICY_STATE_KEY};
pub use retention::expire;

/// IMMEDIATE first, coherent observation second, commit while publication is
/// fenced. The provider invokes exactly once and performs no native sampling.
pub fn transaction<T>(
    connection: &mut Connection,
    provider: &dyn ObservationProvider,
    apply: impl FnOnce(&Connection, &EffectivePolicy) -> JobResult<T>,
) -> JobResult<T> {
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let mut pending = Some((tx, apply));
    let mut result = None;
    provider.with_observation(&mut |input| {
        let Some((tx, apply)) = pending.take() else {
            return;
        };
        result = Some((|| {
            let effective = arc_policy::evaluator::evaluate(input);
            let value = apply(&tx, &effective)?;
            tx.commit()?;
            Ok(value)
        })());
    });
    result.unwrap_or(Err(E::StorageFailure))
}

fn json<T: serde::Serialize>(value: &T) -> JobResult<String> {
    serde_json::to_string(value).map_err(|_| E::StorageFailure)
}
fn tag<T: serde::Serialize>(value: &T) -> JobResult<String> {
    Ok(json(value)?.trim_matches('"').to_owned())
}
fn valid_now(now: u64) -> JobResult<()> {
    (now <= arc_core::meter::MAX_SAFE_INTEGER)
        .then_some(())
        .ok_or(E::InvalidRequest)
}
fn key(
    policy: &EffectivePolicy,
    kind: &str,
    reason: &str,
    generation: Option<u64>,
) -> JobResult<String> {
    let key = json(&(
        &policy.policy_version,
        &policy.profile,
        policy.mode,
        policy.reason,
        kind,
        reason,
        policy.observation.availability,
        &policy.observation.sampler,
        generation,
    ))?;
    (key.len() <= 512).then_some(key).ok_or(E::InvalidRequest)
}
fn insert(
    connection: &Connection,
    job_id: Option<&JobId>,
    policy: &EffectivePolicy,
    kind: &str,
    reason: &str,
    generation: Option<u64>,
    now: u64,
) -> JobResult<()> {
    let key = key(policy, kind, reason, generation)?;
    let last: Option<String> = connection
        .query_row(
            "SELECT transition_key FROM policy_decisions WHERE job_id IS ?1
         ORDER BY rowid DESC LIMIT 1",
            [job_id.map(JobId::as_str)],
            |r| r.get(0),
        )
        .optional()?
        .flatten();
    if last.as_ref() == Some(&key) {
        return Ok(());
    }
    let expires = if job_id.is_none() {
        Some(
            now.saturating_add(DECISION_RETENTION_MS)
                .min(arc_core::meter::MAX_SAFE_INTEGER),
        )
    } else {
        job_id
            .map(|id| retention::terminal_at(connection, id))
            .transpose()?
            .flatten()
            .map(|terminal| {
                terminal
                    .saturating_add(DECISION_RETENTION_MS)
                    .min(arc_core::meter::MAX_SAFE_INTEGER)
            })
    };
    connection.execute(
        "INSERT INTO policy_decisions(id,job_id,policy_version,decision,reason_code,created_at,
         observation_availability,observation_age_ms,observation_json,transition_key,
         permit_generation,recorded_at_ms,expires_at_ms)
         VALUES(lower(hex(randomblob(16))),?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?5,?11)",
        params![
            job_id.map(JobId::as_str),
            policy.policy_version,
            kind,
            reason,
            now as i64,
            tag(&policy.observation.availability)?,
            policy.observation.age_ms.map(|n| n as i64),
            json(&policy.observation)?,
            key,
            generation.map(|n| n as i64),
            expires.map(|n| n as i64)
        ],
    )?;
    let cap = if job_id.is_some() {
        MAX_JOB_DECISIONS
    } else {
        MAX_GLOBAL_DECISIONS
    };
    connection.execute(
        "DELETE FROM policy_decisions WHERE id IN (SELECT id FROM policy_decisions
         WHERE job_id IS ?1 ORDER BY rowid DESC LIMIT ?2 OFFSET ?3)",
        params![
            job_id.map(JobId::as_str),
            MAX_EXPIRY_BATCH as i64,
            cap as i64
        ],
    )?;
    Ok(())
}

/// Call inside the admission transaction. Global state and decision cannot
/// become visible without the corresponding permit/queue change.
pub fn record_effective(
    connection: &Connection,
    policy: &EffectivePolicy,
    now: u64,
) -> JobResult<()> {
    valid_now(now)?;
    if connection.is_autocommit() {
        return Err(E::InvalidTransition);
    }
    if !policy.is_valid() {
        return Err(E::InvalidRequest);
    }
    let state = json(policy)?;
    if state.len() > MAX_POLICY_BYTES {
        return Err(E::InvalidRequest);
    }
    insert(
        connection,
        None,
        policy,
        &tag(&policy.mode)?,
        &tag(&policy.reason)?,
        None,
        now,
    )?;
    connection.execute(
        "INSERT INTO policy_states(key,enabled,policy_revision,updated_at,policy_version,state_json)
         VALUES(?1,1,?2,?3,?4,?5) ON CONFLICT(key) DO UPDATE SET
         policy_revision=excluded.policy_revision,updated_at=excluded.updated_at,
         policy_version=excluded.policy_version,state_json=excluded.state_json",
        params![POLICY_STATE_KEY,POLICY_SCHEMA_VERSION,now as i64,POLICY_VERSION,state])?;
    Ok(())
}
pub fn record(
    connection: &Connection,
    job_id: &JobId,
    policy: &EffectivePolicy,
    kind: DecisionKind,
    reason: DecisionReason,
    generation: Option<u64>,
    now: u64,
) -> JobResult<()> {
    let record = RecordedDecision {
        policy_version: POLICY_VERSION.into(),
        kind,
        reason,
        decided_at_ms: now,
        permit_generation: generation,
        observation: policy.observation.clone(),
    };
    if !record.is_valid() {
        return Err(E::InvalidRequest);
    }
    record_effective(connection, policy, now)?;
    if retention::job_expired(connection, job_id, now)? {
        return Ok(());
    }
    insert(
        connection,
        Some(job_id),
        policy,
        &tag(&kind)?,
        &tag(&reason)?,
        generation,
        now,
    )
}

/// Bounded indexed last-decision read. Historical age stays as recorded;
/// current observation is separately evaluated, never inferred from that row.
pub fn job(
    connection: &Connection,
    id: &JobId,
    effective: EffectivePolicy,
    now: u64,
) -> JobResult<JobPolicyResult> {
    valid_now(now)?;
    if !effective.is_valid() {
        return Err(E::InvalidRequest);
    }
    let exists: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM jobs WHERE id=?1)",
        [id.as_str()],
        |r| r.get(0),
    )?;
    let mut result = JobPolicyResult {
        schema_version: POLICY_SCHEMA_VERSION,
        job_id: id.clone(),
        effective,
        decision_availability: if exists {
            DecisionAvailability::NotRecorded
        } else {
            DecisionAvailability::NotFound
        },
        last_decision: None,
    };
    if !exists {
        return Ok(result);
    }
    if retention::job_expired(connection, id, now)? {
        result.decision_availability = DecisionAvailability::Expired;
        return Ok(result);
    }
    let row = connection.query_row(
        "SELECT policy_version,decision,reason_code,recorded_at_ms,permit_generation,observation_json,
         expires_at_ms FROM policy_decisions WHERE job_id=?1 ORDER BY rowid DESC LIMIT 1",
        [id.as_str()],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,
            r.get::<_,Option<i64>>(3)?,r.get::<_,Option<i64>>(4)?,r.get::<_,Option<String>>(5)?,r.get::<_,Option<i64>>(6)?))).optional()?;
    let Some((version, kind, reason, time, generation, observation, expiry)) = row else {
        return Ok(result);
    };
    let unsigned = |value: Option<i64>| -> JobResult<Option<u64>> {
        value
            .map(|v| u64::try_from(v).map_err(|_| E::StorageFailure))
            .transpose()
    };
    let (time, generation, expiry) = (unsigned(time)?, unsigned(generation)?, unsigned(expiry)?);
    // Preserve a protected latest decision even when its indexed expiry is due.
    let _ = expiry;
    let (Some(time), Some(observation)) = (time, observation) else {
        return Ok(result);
    };
    let record = RecordedDecision {
        policy_version: version,
        kind: serde_json::from_value(serde_json::Value::String(kind))
            .map_err(|_| E::StorageFailure)?,
        reason: serde_json::from_value(serde_json::Value::String(reason))
            .map_err(|_| E::StorageFailure)?,
        decided_at_ms: time,
        permit_generation: generation,
        observation: serde_json::from_str(&observation).map_err(|_| E::StorageFailure)?,
    };
    if !record.is_valid() {
        return Err(E::StorageFailure);
    }
    result.decision_availability = DecisionAvailability::Available;
    result.last_decision = Some(record);
    Ok(result)
}

/// Stable terminal retention anchor, written with the lifecycle/permit change.
/// This minimal marker also prevents cancellation/recovery polls renewing TTL.
pub(crate) fn mark_terminal(connection: &Connection, id: &JobId, terminal: u64) -> JobResult<()> {
    valid_now(terminal)?;
    if connection.is_autocommit() {
        return Err(E::InvalidTransition);
    }
    let terminal = retention::terminal_at(connection, id)?.unwrap_or(terminal);
    connection.execute("INSERT OR IGNORE INTO audit_events(id,severity,event_type,entity_id,payload_redacted_json,created_at) VALUES(?1,'INFO','TRAFFIC_POLICY_TERMINAL',?2,NULL,?3)",
        params![format!("traffic-policy-terminal-{}",id.as_str()),id.as_str(),terminal as i64])?;
    let expiry = terminal
        .saturating_add(DECISION_RETENTION_MS)
        .min(arc_core::meter::MAX_SAFE_INTEGER);
    connection.execute("UPDATE policy_decisions SET expires_at_ms=?2 WHERE job_id=?1 AND (expires_at_ms IS NULL OR expires_at_ms>?2)",params![id.as_str(),expiry as i64])?;
    Ok(())
}

use super::{JobError, JobResult};
use arc_ipc::{
    jobs::{
        IdentityObservation, JobId, JobSnapshot, LifecycleReason, Outcome, Phase, ProcessIdentity,
    },
    Classification,
};
use rusqlite::{params, Connection, Row};
use serde::{de::DeserializeOwned, Serialize};

pub(super) const SELECT_ONE: &str = "SELECT j.id, j.job_class, j.locality, j.state,
    j.created_at, j.finished_at, j.exit_code, d.* FROM jobs j JOIN job_lifecycle d ON d.job_id=j.id WHERE j.id=?1";
fn invalid() -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        std::io::Error::other("invalid lifecycle metadata").into(),
    )
}
fn timestamp(row: &Row<'_>, name: &str) -> rusqlite::Result<u64> {
    u64::try_from(row.get::<_, i64>(name)?).map_err(|_| invalid())
}
fn optional_timestamp(row: &Row<'_>, name: &str) -> rusqlite::Result<Option<u64>> {
    row.get::<_, Option<i64>>(name)?
        .map(|n| u64::try_from(n).map_err(|_| invalid()))
        .transpose()
}
fn value<T: DeserializeOwned>(row: &Row<'_>, name: &str) -> rusqlite::Result<T> {
    let text: String = row.get(name)?;
    serde_json::from_value(serde_json::Value::String(text)).map_err(|_| invalid())
}
pub(super) fn text(value: &impl Serialize) -> JobResult<String> {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .ok_or(JobError::StorageFailure)
}
pub(super) fn legacy_unknown_state(state: &str, kind: Option<&str>, code: Option<u8>) -> bool {
    state == "COMPLETED" && matches!(kind, None | Some("EXITED")) && code.is_none()
}
// New storage states remain the protocol-1 COMPLETED phase on the wire.
pub(super) fn phase(state: &str, kind: Option<&str>, code: Option<u8>) -> rusqlite::Result<Phase> {
    match (state, kind, code) {
        ("SUCCEEDED", Some("EXITED"), Some(0)) | ("FAILED", Some("EXITED"), Some(1..=u8::MAX)) => {
            Ok(Phase::Completed)
        }
        ("SUCCEEDED" | "FAILED", _, _) => Err(invalid()),
        ("COMPLETED", None | Some("EXITED"), None) => Ok(Phase::OutcomeUnknown),
        _ => serde_json::from_value(serde_json::Value::String(state.to_owned()))
            .map_err(|_| invalid()),
    }
}
// A protocol-1 snapshot cannot carry a completion timestamp without an outcome.
// Keep these historical rows distinct from live jobs with an unknown outcome.
pub(super) fn legacy_unknown(connection: &Connection, id: &JobId) -> JobResult<bool> {
    Ok(connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM jobs j JOIN job_lifecycle d ON d.job_id=j.id
         WHERE j.id=?1 AND j.state='COMPLETED' AND j.exit_code IS NULL
         AND (d.outcome_kind IS NULL OR d.outcome_kind='EXITED'))",
        [id.as_str()],
        |row| row.get(0),
    )?)
}
pub(super) fn decode(row: &Row<'_>) -> rusqlite::Result<JobSnapshot> {
    if timestamp(row, "created_at")? != timestamp(row, "registered_at_ms")? {
        return Err(invalid());
    }
    let child = match row.get::<_, Option<u32>>("child_pid")? {
        None => None,
        Some(pid) => Some(ProcessIdentity {
            pid,
            start_token: row.get("child_start_token")?,
            boot_id: row.get("child_boot_id")?,
        }),
    };
    let kind: Option<String> = row.get("outcome_kind")?;
    let fields = (
        row.get::<_, Option<u8>>("exit_code")?,
        row.get::<_, Option<u8>>("signal")?,
        row.get::<_, Option<bool>>("core_dumped")?,
        row.get::<_, Option<String>>("spawn_reason")?,
        row.get::<_, Option<i32>>("os_code")?,
    );
    let state: String = row.get("state")?;
    let phase = phase(&state, kind.as_deref(), fields.0)?;
    let legacy_unknown = legacy_unknown_state(&state, kind.as_deref(), fields.0);
    // Protocol-1 requires a non-terminal reason for an unknown outcome.
    // NONE avoids inventing an identity failure while retaining the observation.
    let outcome = match (kind.as_deref(), fields) {
        (None, (None, None, None, None, None)) => None,
        (Some("EXITED"), (None, None, None, None, None)) if legacy_unknown => None,
        (None, (Some(code), None, None, None, None)) if state == "COMPLETED" => {
            Some(Outcome::Exited { code })
        }
        (Some("EXITED"), (Some(code), None, None, None, None)) => Some(Outcome::Exited { code }),
        (Some("SIGNALED"), (None, Some(signal), Some(core_dumped), None, None)) => {
            Some(Outcome::Signaled {
                signal,
                core_dumped,
            })
        }
        (Some("SPAWN_FAILED"), (None, None, None, Some(_), os_code)) => {
            Some(Outcome::SpawnFailed {
                reason: value(row, "spawn_reason")?,
                os_code,
            })
        }
        _ => return Err(invalid()),
    };
    Ok(JobSnapshot {
        job_id: JobId::try_from(row.get::<_, String>("id")?).map_err(|_| invalid())?,
        phase,
        classification: Classification {
            classification: value(row, "job_class")?,
            locality: value(row, "locality")?,
            reason: value(row, "classification_reason")?,
            uncertainty: value(row, "classification_uncertainty")?,
        },
        observer: ProcessIdentity {
            pid: row.get("observer_pid")?,
            start_token: row.get("observer_start_token")?,
            boot_id: row.get("observer_boot_id")?,
        },
        observer_verified_at_ms: timestamp(row, "observer_verified_at_ms")?,
        child,
        child_verified_at_ms: optional_timestamp(row, "child_verified_at_ms")?,
        observation: IdentityObservation {
            target: value(row, "observation_target")?,
            status: value(row, "observation_status")?,
            observed_at_ms: timestamp(row, "observation_at_ms")?,
            reason: if legacy_unknown {
                LifecycleReason::None
            } else {
                value(row, "observation_reason")?
            },
        },
        reason: if legacy_unknown {
            LifecycleReason::None
        } else {
            value(row, "reason")?
        },
        outcome,
        registered_at_ms: timestamp(row, "created_at")?,
        launch_claimed_at_ms: optional_timestamp(row, "launch_claimed_at_ms")?,
        completed_at_ms: if legacy_unknown {
            None
        } else {
            optional_timestamp(row, "finished_at")?
        },
        updated_at_ms: timestamp(row, "updated_at_ms")?,
        revision: timestamp(row, "revision")?,
    })
}
/// Always called within the same IMMEDIATE transaction as the transition read.
pub(super) fn write(connection: &Connection, j: &JobSnapshot, new: bool) -> JobResult<()> {
    if !j.is_valid() {
        return Err(JobError::InvalidRequest);
    }
    // A synthetic unknown snapshot must never replace historical terminal data.
    if !new && j.outcome.is_none() && legacy_unknown(connection, &j.job_id)? {
        return Err(JobError::InvalidTransition);
    }
    let (kind, code, signal, core_dumped, spawn_reason, os_code) = match &j.outcome {
        None => (None, None, None, None, None, None),
        Some(Outcome::Exited { code }) => (Some("EXITED"), Some(*code), None, None, None, None),
        Some(Outcome::Signaled {
            signal,
            core_dumped,
        }) => (
            Some("SIGNALED"),
            None,
            Some(*signal),
            Some(*core_dumped),
            None,
            None,
        ),
        Some(Outcome::SpawnFailed { reason, os_code }) => (
            Some("SPAWN_FAILED"),
            None,
            None,
            None,
            Some(text(reason)?),
            *os_code,
        ),
    };
    let state = match (&j.phase, &j.outcome) {
        (Phase::Completed, Some(Outcome::Exited { code: 0 })) => "SUCCEEDED".to_owned(),
        (Phase::Completed, Some(Outcome::Exited { .. })) => "FAILED".to_owned(),
        _ => text(&j.phase)?,
    };
    if new {
        connection.execute("INSERT INTO jobs (id,job_class,locality,enforcement,state,created_at) VALUES (?1,?2,?3,'OBSERVE',?4,?5)",
            params![j.job_id.as_str(), text(&j.classification.classification)?, text(&j.classification.locality)?, state, (j.registered_at_ms as i64)])?;
    } else {
        connection.execute("UPDATE jobs SET state=?2, started_at=coalesce(started_at,?3), finished_at=?4, exit_code=?5 WHERE id=?1",
            params![j.job_id.as_str(), state, j.child_verified_at_ms.map(|v| v as i64), j.completed_at_ms.map(|v| v as i64), code])?;
    }
    connection.execute("INSERT INTO job_lifecycle (job_id,observer_pid,observer_start_token,observer_boot_id,observer_verified_at_ms,child_pid,child_start_token,child_boot_id,child_verified_at_ms,classification_reason,classification_uncertainty,launch_claimed_at_ms,observation_target,observation_status,observation_at_ms,observation_reason,outcome_kind,signal,core_dumped,spawn_reason,os_code,revision,updated_at_ms,reason,registered_at_ms)
        VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25) ON CONFLICT(job_id) DO UPDATE SET
        observer_pid=excluded.observer_pid,observer_start_token=excluded.observer_start_token,observer_boot_id=excluded.observer_boot_id,observer_verified_at_ms=excluded.observer_verified_at_ms,child_pid=excluded.child_pid,child_start_token=excluded.child_start_token,child_boot_id=excluded.child_boot_id,child_verified_at_ms=excluded.child_verified_at_ms,classification_reason=excluded.classification_reason,classification_uncertainty=excluded.classification_uncertainty,launch_claimed_at_ms=excluded.launch_claimed_at_ms,observation_target=excluded.observation_target,observation_status=excluded.observation_status,observation_at_ms=excluded.observation_at_ms,observation_reason=excluded.observation_reason,outcome_kind=excluded.outcome_kind,signal=excluded.signal,core_dumped=excluded.core_dumped,spawn_reason=excluded.spawn_reason,os_code=excluded.os_code,revision=excluded.revision,updated_at_ms=excluded.updated_at_ms,reason=excluded.reason",
        params![
            j.job_id.as_str(),
            j.observer.pid,
            j.observer.start_token,
            j.observer.boot_id,
            (j.observer_verified_at_ms as i64),
            j.child.as_ref().map(|c| c.pid),
            j.child.as_ref().map(|c| c.start_token.as_str()),
            j.child.as_ref().map(|c| c.boot_id.as_str()),
            j.child_verified_at_ms.map(|v| v as i64),
            text(&j.classification.reason)?,
            text(&j.classification.uncertainty)?,
            j.launch_claimed_at_ms.map(|v| v as i64),
            text(&j.observation.target)?,
            text(&j.observation.status)?,
            (j.observation.observed_at_ms as i64),
            text(&j.observation.reason)?,
            kind,
            signal,
            core_dumped,
            spawn_reason,
            os_code,
            (j.revision as i64),
            (j.updated_at_ms as i64),
            text(&j.reason)?,
            (j.registered_at_ms as i64),
        ])?;
    Ok(())
}

//! Read committed classification, admission and permit facts in one snapshot.
//! No native probe, lifecycle reconciliation, policy write or permit mutation.
use super::{get, governor, store, valid_now, JobError as E, JobResult};
use arc_ipc::{
    jobs::JobId,
    traffic::{
        diagnostics::{AdmissionDiagnostic, AdmissionState as S, DiagnosticResult},
        Resources,
    },
};
use arc_policy::{DecisionKind, ObservationProvider};
use arc_traffic::Policy;
use rusqlite::{Connection, OptionalExtension};

pub fn diagnostics(
    connection: &mut Connection,
    id: &JobId,
    policy: &Policy,
    now: u64,
) -> JobResult<DiagnosticResult> {
    valid_now(now, None)?;
    policy.validate().map_err(|_| E::InvalidRequest)?;
    let tx = connection.transaction()?;
    // Reading the job establishes the SQLite snapshot for all subsequent reads.
    let job = get(&tx, id)?;
    let mut result = DiagnosticResult {
        schema_version: 1,
        job_id: id.clone(),
        recorded_at_ms: job.as_ref().map_or(now, |job| now.max(job.updated_at_ms)),
        liveness_fresh: false,
        classification: job.as_ref().map(|job| job.classification),
        admission: None,
        limits: (*policy).into(),
    };
    if let Some(entry) = store::entry(&tx, id)? {
        let job = job.ok_or(E::StorageFailure)?;
        let disposition = governor::disposition(&tx, id)?;
        let unreserved = disposition.as_ref().filter(|mode| mode.unreserved);
        let grant = entry
            .granted_at
            .or_else(|| unreserved.and_then(|mode| mode.ready_at));
        let queue_wait = grant
            .map(|at| at.checked_sub(entry.queued_at).ok_or(E::StorageFailure))
            .transpose()?;
        let mut effective = None;
        arc_policy::evaluator::MissingObservation.with_observation(&mut |input| {
            effective = Some(arc_policy::evaluator::evaluate(input))
        });
        let last_decision = crate::policy::job(
            &tx,
            id,
            effective.ok_or(E::StorageFailure)?,
            result.recorded_at_ms,
        )?
        .last_decision;
        if let Some(decision) = &last_decision {
            result.recorded_at_ms = result.recorded_at_ms.max(decision.decided_at_ms);
        }
        let state = match entry.state.as_str() {
            "QUEUED" if unreserved.is_some_and(|mode| mode.release_id.is_some()) => S::Unconfirmed,
            "QUEUED" if unreserved.is_some() => S::Unreserved,
            "QUEUED" => S::Queued,
            "RESERVED" if entry.generation.is_some() => S::Reserved,
            "RESERVED" | "RETAINED" => S::Unconfirmed,
            "RUNNING" => S::Running,
            "COMPLETED" if job.outcome.is_some() => S::Completed,
            "COMPLETED" => S::Unconfirmed,
            "CANCELLED" => S::Cancelled,
            "ABANDONED" => S::Abandoned,
            _ => return Err(E::StorageFailure),
        };
        let (root, reservation, active) = if let Some(generation) = entry.generation {
            let root = entry.root.clone().unwrap_or_else(|| id.clone());
            let row = tx.query_row(
                "SELECT cpu_millis_est,memory_bytes_est,state FROM permits WHERE job_id=?1 AND generation=?2",
                rusqlite::params![root.as_str(), generation as i64],
                |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?, r.get::<_, String>(2)?)),
            ).optional()?.ok_or(E::StorageFailure)?;
            if !matches!(row.2.as_str(), "ACTIVE" | "RELEASED") {
                return Err(E::StorageFailure);
            }
            (
                Some(root),
                Some(Resources {
                    cpu_millis: u64::try_from(row.0).map_err(|_| E::StorageFailure)?,
                    memory_bytes: u64::try_from(row.1).map_err(|_| E::StorageFailure)?,
                }),
                row.2 == "ACTIVE",
            )
        } else {
            (None, None, false)
        };
        result.admission = Some(AdmissionDiagnostic {
            state,
            sequence: entry.sequence,
            queued_at_ms: entry.queued_at,
            // Governor OFF eligibility has a wait timestamp, never a grant.
            granted_at_ms: entry.granted_at,
            queue_wait_ms: queue_wait,
            wait_reason: last_decision.as_ref().and_then(|d| {
                (state == S::Queued && d.kind == DecisionKind::Wait)
                    .then_some(d.reason)
                    .filter(|reason| reason.legacy_wait().is_some())
            }),
            last_decision,
            root_job_id: root,
            permit_generation: entry.generation,
            reservation,
            permit_active: active,
            outcome: job.outcome,
        });
    }
    if !result.is_valid() {
        return Err(E::StorageFailure);
    }
    tx.commit()?;
    Ok(result)
}

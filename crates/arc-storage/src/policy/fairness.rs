//! Storage codec for the additive fairness reason (AT-TRF-08).
//!
//! `policy_decisions.reason_code` holds the raw recorded tag. A fairness
//! reason is stored as written; `policy::job`, and so every protocol-1, v1
//! diagnostics and policy reply, reads it through [`projected`] as its valid
//! existing pair. Only [`negotiated`] returns the raw reason, for the opt-in
//! read `arc_ipc::traffic::fairness::OPERATION_DECISION`.
use super::{insert, record_effective, retention, tag, E};
use crate::jobs::JobResult;
use arc_ipc::jobs::JobId;
use arc_policy::{
    DecisionKind, DecisionReason, EffectivePolicy, FairnessReason, NegotiatedReason,
    RecordedDecision, POLICY_VERSION,
};
use rusqlite::{Connection, OptionalExtension};

fn parse(reason: String) -> JobResult<NegotiatedReason> {
    serde_json::from_value(serde_json::Value::String(reason)).map_err(|_| E::StorageFailure)
}

/// The existing-vocabulary reason of one stored tag.
pub(super) fn projected(reason: String) -> JobResult<DecisionReason> {
    Ok(parse(reason)?.projected())
}

/// Record the grant decision of a job that received a fair turn. Call inside
/// the admission transaction in place of `policy::record` for that grant; the
/// same validation, retention and transition de-duplication apply.
pub fn record(
    connection: &Connection,
    job_id: &JobId,
    policy: &EffectivePolicy,
    reason: FairnessReason,
    generation: u64,
    now: u64,
) -> JobResult<()> {
    let (kind, projection) = reason.projection();
    let record = RecordedDecision {
        policy_version: POLICY_VERSION.into(),
        kind,
        reason: projection,
        decided_at_ms: now,
        permit_generation: Some(generation),
        observation: policy.observation.clone(),
    };
    if kind != DecisionKind::Admit || !record.is_valid() {
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
        Some(generation),
        now,
    )
}

/// Record a decision of a job that may already hold a fair turn. A repeated
/// or launch-time `admitted` for the permit generation that was granted as a
/// fair turn records that reason again, so it stays the job's last recorded
/// decision; every other decision is recorded exactly as `policy::record`.
pub(crate) fn keep(
    connection: &Connection,
    job_id: &JobId,
    policy: &EffectivePolicy,
    kind: DecisionKind,
    reason: DecisionReason,
    generation: Option<u64>,
    now: u64,
) -> JobResult<()> {
    if let ((DecisionKind::Admit, DecisionReason::Admitted), Some(generation)) =
        ((kind, reason), generation)
    {
        let mut statement = connection.prepare(
            "SELECT reason_code FROM policy_decisions
             WHERE job_id=?1 AND decision='admit' AND permit_generation=?2 ORDER BY rowid DESC",
        )?;
        let rows = statement.query_map((job_id.as_str(), generation as i64), |row| {
            row.get::<_, String>(0)
        })?;
        for row in rows {
            if let NegotiatedReason::Fairness(fair) = parse(row?)? {
                return record(connection, job_id, policy, fair, generation, now);
            }
        }
    }
    super::record(connection, job_id, policy, kind, reason, generation, now)
}

/// The raw last recorded reason of one job, under the same retention rule as
/// `policy::job`. `None` when the job has no retained decision.
pub fn negotiated(
    connection: &Connection,
    id: &JobId,
    now: u64,
) -> JobResult<Option<NegotiatedReason>> {
    super::valid_now(now)?;
    if retention::job_expired(connection, id, now)? {
        return Ok(None);
    }
    connection
        .query_row(
            "SELECT reason_code FROM policy_decisions WHERE job_id=?1 ORDER BY rowid DESC LIMIT 1",
            [id.as_str()],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .map(parse)
        .transpose()
}

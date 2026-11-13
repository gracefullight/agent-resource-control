//! Shared Governor state read. The daemon's existing writer owns mutations.
use crate::jobs::{JobError, JobResult};
use arc_ipc::governor::{GovernorSwitchKey, GovernorSwitchState, SWITCH_KEY};
use rusqlite::{Connection, OptionalExtension};

/// Call inside the decision transaction for admission/launch; a missing row is
/// default ON. Corrupt data and SQL failure never become a fabricated state.
pub fn get_switch(connection: &Connection) -> JobResult<GovernorSwitchState> {
    let raw: Option<(i64, i64, i64)> = connection
        .query_row(
            "SELECT enabled,policy_revision,updated_at FROM policy_states WHERE key=?1",
            [SWITCH_KEY],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let Some((enabled, revision, updated_at_ms)) = raw else {
        return Ok(GovernorSwitchState::default());
    };
    let state = GovernorSwitchState {
        key: GovernorSwitchKey::GovernorEnabled,
        enabled: match enabled {
            0 => false,
            1 => true,
            _ => return Err(JobError::StorageFailure),
        },
        revision: u64::try_from(revision).map_err(|_| JobError::StorageFailure)?,
        updated_at_ms: Some(u64::try_from(updated_at_ms).map_err(|_| JobError::StorageFailure)?),
    };
    state
        .is_valid()
        .then_some(state)
        .ok_or(JobError::StorageFailure)
}

use arc_ipc::governor::{
    GetGovernorSwitchRequest, GovernorConflictReason as Conflict,
    GovernorMutationResult as Mutation, GovernorRejectionReason as Rejection,
    GovernorSwitchReadResult, SetGovernorSwitchRequest, MAX_MUTATION_RECEIPTS, MAX_SAFE_INTEGER,
};
use rusqlite::{params, Transaction};

fn receipt(
    connection: &Connection,
    id: &str,
) -> JobResult<Option<(SetGovernorSwitchRequest, Mutation)>> {
    let raw = connection.query_row(
        "SELECT expected_revision,desired_enabled,result_enabled,result_revision,result_updated_at_ms,changed FROM governor_mutation_receipts WHERE mutation_id=?1",
        [id], |r| Ok((r.get::<_,i64>(0)? as u64,r.get::<_,bool>(1)?,r.get::<_,bool>(2)?,r.get::<_,i64>(3)? as u64,r.get::<_,Option<i64>>(4)?.map(|n|n as u64),r.get::<_,bool>(5)?))
    ).optional()?;
    raw.map(|(base, desired, enabled, revision, at, changed)| {
        let request = SetGovernorSwitchRequest {
            key: GovernorSwitchKey::GovernorEnabled,
            enabled: desired,
            expected_revision: base,
            mutation_id: id.into(),
        };
        let result = Mutation::Committed {
            mutation_id: id.into(),
            switch: GovernorSwitchState {
                key: GovernorSwitchKey::GovernorEnabled,
                enabled,
                revision,
                updated_at_ms: at,
            },
            changed,
            replayed: true,
        };
        if !result.is_valid_for(&request) {
            return Err(JobError::StorageFailure);
        }
        Ok((request, result))
    })
    .transpose()
}

/// Caller supplies a read transaction so current state and original receipt are
/// from one authoritative snapshot, even after an ambiguous post-COMMIT reply.
pub fn read_switch(
    connection: &Connection,
    query: &GetGovernorSwitchRequest,
) -> JobResult<GovernorSwitchReadResult> {
    let value = GovernorSwitchReadResult {
        switch: get_switch(connection)?,
        mutation_result: query
            .mutation_id
            .as_deref()
            .map(|id| receipt(connection, id))
            .transpose()?
            .flatten()
            .map(|(_, r)| r),
    };
    value
        .is_valid_for(query)
        .then_some(value)
        .ok_or(JobError::InvalidRequest)
}

/// Tentative result only: the daemon must COMMIT before publishing this value.
/// All business policy and receipt retention run in the existing single writer.
pub fn set_switch(
    tx: &Transaction<'_>,
    request: &SetGovernorSwitchRequest,
    now_ms: u64,
) -> JobResult<Mutation> {
    if !request.is_valid() || now_ms > MAX_SAFE_INTEGER {
        return Err(JobError::InvalidRequest);
    }
    let switch = get_switch(tx)?;
    if let Some((previous, result)) = receipt(tx, &request.mutation_id)? {
        return Ok(if previous == *request {
            result
        } else {
            Mutation::Conflict {
                mutation_id: request.mutation_id.clone(),
                switch,
                reason: Conflict::MutationIdConflict,
            }
        });
    }
    if switch.revision != request.expected_revision {
        return Ok(Mutation::Conflict {
            mutation_id: request.mutation_id.clone(),
            switch,
            reason: Conflict::RevisionConflict,
        });
    }
    let changed = switch.enabled != request.enabled;
    if changed && switch.revision == MAX_SAFE_INTEGER {
        return Ok(Mutation::Rejected {
            mutation_id: request.mutation_id.clone(),
            switch,
            reason: Rejection::RevisionExhausted,
        });
    }
    let revision = switch.revision + u64::from(changed);
    let count: i64 = tx.query_row("SELECT count(*) FROM governor_mutation_receipts", [], |r| {
        r.get(0)
    })?;
    let remove = count
        .saturating_add(1)
        .saturating_sub(MAX_MUTATION_RECEIPTS as i64)
        .max(0);
    let eligible: i64 = tx.query_row(
        "SELECT count(*) FROM governor_mutation_receipts WHERE expected_revision<?1",
        [revision as i64],
        |r| r.get(0),
    )?;
    if eligible < remove {
        return Ok(Mutation::Rejected {
            mutation_id: request.mutation_id.clone(),
            switch,
            reason: Rejection::ReceiptCapacity,
        });
    }
    let switch = if changed {
        let at = now_ms.max(switch.updated_at_ms.unwrap_or(0));
        tx.execute("INSERT INTO policy_states(key,enabled,policy_revision,policy_version,updated_at) VALUES(?1,?2,?3,'governor.v1',?4) ON CONFLICT(key) DO UPDATE SET enabled=excluded.enabled,policy_revision=excluded.policy_revision,policy_version=excluded.policy_version,updated_at=excluded.updated_at",params![SWITCH_KEY,request.enabled,revision as i64,at as i64])?;
        let switch = get_switch(tx)?;
        crate::jobs::traffic::governor::reconcile_switch(tx, &switch, at)?;
        switch
    } else {
        switch
    };
    tx.execute("DELETE FROM governor_mutation_receipts WHERE sequence IN (SELECT sequence FROM governor_mutation_receipts WHERE expected_revision<?1 ORDER BY sequence LIMIT ?2)",params![revision as i64,remove])?;
    tx.execute("INSERT INTO governor_mutation_receipts(mutation_id,expected_revision,desired_enabled,result_enabled,result_revision,result_updated_at_ms,changed,committed_at_ms) VALUES(?1,?2,?3,?3,?4,?5,?6,?7)",params![request.mutation_id,request.expected_revision as i64,request.enabled,switch.revision as i64,switch.updated_at_ms.map(|v|v as i64),changed,now_ms as i64])?;
    Ok(Mutation::Committed {
        mutation_id: request.mutation_id.clone(),
        switch,
        changed,
        replayed: false,
    })
}

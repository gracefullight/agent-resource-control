//! Stage linkage: which job records one hook or exec request started.
//!
//! One `audit_events` row per linked stage, written by the daemon's single
//! writer. No migration, and no command, path or environment text.
use crate::jobs::{JobError, JobResult};
use arc_ipc::{
    jobs::{JobId, MAX_TIMESTAMP},
    request::{RequestId, StageRequest, StagesResult, MAX_STAGES},
};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

const EVENT: &str = "REQUEST_STAGE";
const PREFIX: &str = "request-stage-";

/// Apply one validated wire request. Linking is idempotent for the same
/// request; a job belongs to at most one request.
pub fn apply(
    connection: &mut Connection,
    request: &StageRequest,
    now_ms: u64,
) -> JobResult<StagesResult> {
    let StageRequest::Link {
        request_id,
        job_id,
        observer,
    } = request
    else {
        let StageRequest::Stages { request_id } = request else {
            return Err(JobError::InvalidRequest);
        };
        return stages(connection, request_id);
    };
    if !request.is_valid() || now_ms > MAX_TIMESTAMP {
        return Err(JobError::InvalidRequest);
    }
    let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    // Only the wrapper that registered a job may name its request.
    let owned: bool = tx.query_row(
        "SELECT EXISTS(SELECT 1 FROM job_lifecycle WHERE job_id=?1 AND observer_pid=?2
         AND observer_start_token=?3 AND observer_boot_id=?4)",
        params![
            job_id.as_str(),
            observer.pid,
            observer.start_token,
            observer.boot_id
        ],
        |row| row.get(0),
    )?;
    if !owned {
        return Err(JobError::InvalidRequest);
    }
    let id = format!("{PREFIX}{}", job_id.as_str());
    let linked: Option<String> = tx
        .query_row(
            "SELECT entity_id FROM audit_events WHERE id=?1 AND event_type=?2",
            params![id, EVENT],
            |row| row.get(0),
        )
        .optional()?;
    match linked {
        Some(existing) if existing == request_id.as_str() => (),
        Some(_) => return Err(JobError::Conflict),
        None => {
            let count: i64 = tx.query_row(
                "SELECT count(*) FROM audit_events WHERE event_type=?1 AND entity_id=?2",
                params![EVENT, request_id.as_str()],
                |row| row.get(0),
            )?;
            if count >= MAX_STAGES as i64 {
                return Err(JobError::InvalidRequest);
            }
            tx.execute(
                "INSERT INTO audit_events(id,severity,event_type,entity_id,payload_redacted_json,created_at)
                 VALUES(?1,'INFO',?2,?3,NULL,?4)",
                params![id, EVENT, request_id.as_str(), now_ms as i64],
            )?;
        }
    }
    let result = stages(&tx, request_id)?;
    tx.commit()?;
    Ok(result)
}

/// The stage records of one request in link order. Unknown requests are empty.
pub fn stages(connection: &Connection, request_id: &RequestId) -> JobResult<StagesResult> {
    let mut statement = connection.prepare(
        "SELECT substr(id,?3) FROM audit_events WHERE event_type=?1 AND entity_id=?2
         ORDER BY rowid LIMIT ?4",
    )?;
    let jobs = statement
        .query_map(
            params![
                EVENT,
                request_id.as_str(),
                PREFIX.len() as i64 + 1,
                MAX_STAGES as i64
            ],
            |row| row.get::<_, String>(0),
        )?
        .map(|id| JobId::try_from(id?).map_err(|_| JobError::StorageFailure))
        .collect::<JobResult<Vec<_>>>()?;
    let result = StagesResult {
        request_id: request_id.clone(),
        jobs,
    };
    result
        .is_valid()
        .then_some(result)
        .ok_or(JobError::StorageFailure)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jobs::{start, StartEvidence};
    use arc_core::process_identity::ProcessIdentity;
    use arc_ipc::jobs::StartRequest;
    use std::fs;

    fn id(n: u32) -> JobId {
        JobId::try_from(format!("{n:032x}")).unwrap()
    }
    fn register(connection: &mut Connection, n: u32, observer: &ProcessIdentity) {
        let request = StartRequest::Register {
            job_id: id(n),
            observer: observer.clone(),
            classification: serde_json::from_str(
                r#"{"classification":"UNKNOWN","locality":"UNKNOWN","reason":"UNSUPPORTED_SYNTAX","uncertainty":"OBSERVE"}"#,
            )
            .unwrap(),
            usage: None,
        };
        start(connection, &request, &StartEvidence::gather(&request), 100).unwrap();
    }
    fn link(request: u32, job: u32, observer: &ProcessIdentity) -> StageRequest {
        StageRequest::Link {
            request_id: id(request),
            job_id: id(job),
            observer: observer.clone(),
        }
    }

    #[test]
    fn stage_links_are_owner_bound_idempotent_ordered_and_bounded() {
        let root = std::env::temp_dir().join(format!(
            "arc-request-{}",
            arc_core::process_identity::random_job_id().unwrap()
        ));
        fs::create_dir(&root).unwrap();
        let mut connection = crate::open_and_migrate(root.join("test.sqlite3")).unwrap();
        let observer = ProcessIdentity::capture(std::process::id()).unwrap();
        let mut other = observer.clone();
        other.pid += 1;
        for n in 2..=20 {
            register(&mut connection, n, &observer);
        }
        let read = StageRequest::Stages { request_id: id(1) };
        assert_eq!(apply(&mut connection, &read, 200).unwrap().jobs, []);

        // An unregistered job, another observer and a self link add nothing.
        for rejected in [
            link(1, 99, &observer),
            link(1, 2, &other),
            link(2, 2, &observer),
        ] {
            assert_eq!(
                apply(&mut connection, &rejected, 200),
                Err(JobError::InvalidRequest)
            );
        }
        assert_eq!(
            apply(&mut connection, &link(1, 2, &observer), MAX_TIMESTAMP + 1),
            Err(JobError::InvalidRequest)
        );
        assert_eq!(apply(&mut connection, &read, 200).unwrap().jobs, []);

        let first = apply(&mut connection, &link(1, 3, &observer), 200).unwrap();
        assert_eq!(first.jobs, [id(3)]);
        let both = apply(&mut connection, &link(1, 2, &observer), 200).unwrap();
        assert_eq!((both.request_id, both.jobs), (id(1), vec![id(3), id(2)]));
        // A retry is unchanged; a second request cannot take the same job.
        let retry = apply(&mut connection, &link(1, 3, &observer), 900).unwrap();
        assert_eq!(retry.jobs, [id(3), id(2)]);
        assert_eq!(
            apply(&mut connection, &link(4, 3, &observer), 200),
            Err(JobError::Conflict)
        );
        // A request record may itself be a job, as on the exec route.
        let nested = apply(&mut connection, &link(2, 4, &observer), 200).unwrap();
        assert_eq!(nested.jobs, [id(4)]);

        for n in 5..5 + MAX_STAGES as u32 - 2 {
            apply(&mut connection, &link(1, n, &observer), 200).unwrap();
        }
        assert_eq!(
            apply(&mut connection, &link(1, 20, &observer), 200),
            Err(JobError::InvalidRequest)
        );
        let stored = stages(&connection, &id(1)).unwrap();
        assert_eq!(stored.jobs.len(), MAX_STAGES);
        let rows: i64 = connection
            .query_row(
                "SELECT count(*) FROM audit_events WHERE event_type='REQUEST_STAGE'
                 AND severity='INFO' AND payload_redacted_json IS NULL",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(rows, MAX_STAGES as i64 + 1);
        drop(connection);
        fs::remove_dir_all(root).unwrap();
    }
}

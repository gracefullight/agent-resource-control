//! Usage attribution on a new job row (lane B, MTR-004, MET-026).
//!
//! Called once per created job, inside the transaction that inserted it:
//! from lifecycle registration (`transitions::start`) and from Traffic
//! admission. Writes `jobs.vendor`, `jobs.hook_overhead_ms` and
//! `jobs.project_key` (migration 0009); `project` is the fairness key an
//! enqueue already carries, used when the usage fields name no project. A
//! job without any of them keeps NULL columns: a direct command records no
//! vendor or overhead, and no value is ever invented.
use super::JobResult;
use arc_ipc::{
    jobs::{usage::UsageFields, JobId},
    traffic::fairness::ProjectKey,
};
use rusqlite::{params, Connection};

pub(crate) fn record(
    connection: &Connection,
    job_id: &JobId,
    usage: Option<&UsageFields>,
    project: Option<&ProjectKey>,
) -> JobResult<()> {
    let vendor = usage.and_then(|u| u.vendor).map(|v| v.as_str());
    let overhead = usage
        .and_then(|u| u.hook_overhead_ms)
        .map(|o| i64::try_from(o.get()))
        .transpose()
        .map_err(|_| super::JobError::InvalidRequest)?;
    let project = usage
        .and_then(|u| u.project_key.as_ref())
        .or(project)
        .map(ProjectKey::as_str);
    if vendor.is_none() && overhead.is_none() && project.is_none() {
        return Ok(());
    }
    let changed = connection.execute(
        "UPDATE jobs SET vendor=?2,hook_overhead_ms=?3,project_key=?4 WHERE id=?1",
        params![job_id.as_str(), vendor, overhead, project],
    )?;
    if changed == 1 {
        Ok(())
    } else {
        Err(super::JobError::StorageFailure)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arc_ipc::jobs::usage::{HookOverheadMs, Vendor};

    const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const C: &str = "cccccccccccccccccccccccccccccccc";
    const X: &str = "dddddddddddddddddddddddddddddddd";

    fn database() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        connection
            .execute_batch(
                "CREATE TABLE jobs(id TEXT PRIMARY KEY, vendor TEXT, hook_overhead_ms INTEGER,
                 project_key TEXT);",
            )
            .unwrap();
        for id in [A, B, C] {
            connection
                .execute("INSERT INTO jobs(id) VALUES(?1)", [id])
                .unwrap();
        }
        connection
    }

    fn row(connection: &Connection, id: &str) -> (Option<String>, Option<i64>, Option<String>) {
        connection
            .query_row(
                "SELECT vendor,hook_overhead_ms,project_key FROM jobs WHERE id=?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap()
    }

    fn id(text: &str) -> JobId {
        JobId::try_from(text.to_owned()).unwrap()
    }

    #[test]
    fn hook_fields_and_the_project_key_are_stored_and_absent_fields_stay_null() {
        let connection = database();
        let key = ProjectKey::try_from("ab".repeat(32)).unwrap();
        let other = ProjectKey::try_from("cd".repeat(32)).unwrap();
        let usage = UsageFields {
            vendor: Some(Vendor::ClaudeCode),
            hook_overhead_ms: Some(HookOverheadMs::try_from(0).unwrap()),
            project_key: None,
        };
        record(&connection, &id(A), Some(&usage), Some(&key)).unwrap();
        assert_eq!(
            row(&connection, A),
            (Some("claude-code".into()), Some(0), Some("ab".repeat(32)))
        );
        // The registration's own project key wins over a fairness key.
        let usage = UsageFields {
            project_key: Some(other),
            ..UsageFields::default()
        };
        record(&connection, &id(B), Some(&usage), Some(&key)).unwrap();
        assert_eq!(row(&connection, B), (None, None, Some("cd".repeat(32))));
        // A direct command without a key writes nothing.
        record(&connection, &id(C), None, None).unwrap();
        record(&connection, &id(C), Some(&UsageFields::default()), None).unwrap();
        assert_eq!(row(&connection, C), (None, None, None));
        // A row that does not exist is a storage failure, never a silent loss.
        assert_eq!(
            record(&connection, &id(X), Some(&usage), None),
            Err(super::super::JobError::StorageFailure)
        );
    }
}

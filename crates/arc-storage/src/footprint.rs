//! Allocated-footprint readings of a job (lane B: AT-MTR-02, G-01 item 4).
//!
//! Columns `jobs.footprint_{start,end}_{bytes,completeness}` (migration
//! 0010): bytes are NULL exactly when a reading is unavailable or was never
//! taken. The daemon's start and end attachment points
//! (`arc-daemon src/db_worker/footprint.rs`) write them through [`record`];
//! [`read`] is the reply of `meter.job_footprint`. `end_bytes` is the Current
//! Footprint (what remains); the delta is `end - start` only when both are
//! complete. Neither is ever derived from Disk Written.
use crate::jobs::{JobError, JobResult};
use arc_ipc::{
    jobs::JobId,
    meter::{
        footprint::{JobFootprint, JobFootprintReply, BASIS_ALLOCATED, SCHEMA_VERSION},
        quality::{Availability, Flagged},
    },
};
use rusqlite::{params, Connection, OptionalExtension};

pub const REASON_NOT_MEASURED: &str = "not_measured";
pub const REASON_WALK_FAILED: &str = "walk_failed";
pub const REASON_READING_INCOMPLETE: &str = "reading_incomplete";

/// Which reading of a job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Point {
    Start,
    End,
}

/// One stored reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reading {
    Complete(u64),
    Partial(u64),
    Unavailable,
}

/// Store one reading of `job`. Returns `false` when no such job exists. A
/// start and end readings are each written once; a later one never replaces them.
pub fn record(
    connection: &Connection,
    job: &JobId,
    point: Point,
    reading: Reading,
) -> JobResult<bool> {
    let (bytes, completeness) = match reading {
        Reading::Complete(bytes) => (Some(bytes), "complete"),
        Reading::Partial(bytes) => (Some(bytes), "partial"),
        Reading::Unavailable => (None, "unavailable"),
    };
    let bytes = bytes
        .map(i64::try_from)
        .transpose()
        .map_err(|_| JobError::InvalidRequest)?;
    let sql = match point {
        Point::Start => {
            "UPDATE jobs SET footprint_start_bytes=?2,footprint_start_completeness=?3
             WHERE id=?1 AND footprint_start_completeness IS NULL"
        }
        Point::End => {
            "UPDATE jobs SET footprint_end_bytes=?2,footprint_end_completeness=?3
             WHERE id=?1 AND footprint_end_completeness IS NULL"
        }
    };
    let changed = connection.execute(sql, params![job.as_str(), bytes, completeness])?;
    if changed == 1 {
        return Ok(true);
    }
    Ok(connection
        .query_row("SELECT 1 FROM jobs WHERE id=?1", [job.as_str()], |_| Ok(()))
        .optional()?
        .is_some())
}

fn flagged(bytes: Option<i64>, completeness: Option<&str>) -> Flagged<u64> {
    let value = bytes.and_then(|b| u64::try_from(b).ok());
    let availability = match (completeness, value) {
        (Some("complete"), Some(_)) => Availability::Complete,
        (Some("partial"), Some(_)) => Availability::Partial,
        (Some(_), _) => return Flagged::unavailable(BASIS_ALLOCATED, REASON_WALK_FAILED),
        (None, _) => return Flagged::unavailable(BASIS_ALLOCATED, REASON_NOT_MEASURED),
    };
    Flagged {
        value,
        availability,
        basis: BASIS_ALLOCATED.to_owned(),
        reason: None,
    }
}

/// The footprint of `job`, or `None` when no such job is recorded.
pub fn read(connection: &Connection, job: &JobId) -> JobResult<Option<JobFootprintReply>> {
    type Row = (Option<i64>, Option<String>, Option<i64>, Option<String>);
    let row: Option<Row> = connection
        .query_row(
            "SELECT footprint_start_bytes,footprint_start_completeness,
                    footprint_end_bytes,footprint_end_completeness FROM jobs WHERE id=?1",
            [job.as_str()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    let Some((start, start_c, end, end_c)) = row else {
        return Ok(None);
    };
    let start = flagged(start, start_c.as_deref());
    let end = flagged(end, end_c.as_deref());
    let delta = match (&start, &end) {
        (
            Flagged {
                value: Some(a),
                availability: Availability::Complete,
                ..
            },
            Flagged {
                value: Some(b),
                availability: Availability::Complete,
                ..
            },
        ) => match (i64::try_from(*b), i64::try_from(*a)) {
            (Ok(b), Ok(a)) => Flagged {
                value: Some(b - a),
                availability: Availability::Complete,
                basis: BASIS_ALLOCATED.to_owned(),
                reason: None,
            },
            _ => Flagged::unavailable(BASIS_ALLOCATED, REASON_READING_INCOMPLETE),
        },
        _ if start.availability == Availability::Unavailable
            && end.availability == Availability::Unavailable
            && start.reason.as_deref() == Some(REASON_NOT_MEASURED)
            && end.reason.as_deref() == Some(REASON_NOT_MEASURED) =>
        {
            Flagged::unavailable(BASIS_ALLOCATED, REASON_NOT_MEASURED)
        }
        _ => Flagged::unavailable(BASIS_ALLOCATED, REASON_READING_INCOMPLETE),
    };
    Ok(Some(JobFootprintReply {
        schema_version: SCHEMA_VERSION,
        job_id: job.clone(),
        footprint: JobFootprint {
            start_bytes: start,
            end_bytes: end,
            delta_bytes: delta,
        },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIGRATION: &str = "CREATE TABLE jobs(id TEXT PRIMARY KEY,
        footprint_start_bytes INTEGER, footprint_start_completeness TEXT,
        footprint_end_bytes INTEGER, footprint_end_completeness TEXT);";

    fn id(c: char) -> JobId {
        JobId::try_from(c.to_string().repeat(32)).unwrap()
    }

    fn database() -> Connection {
        let connection = Connection::open_in_memory().unwrap();
        connection.execute_batch(MIGRATION).unwrap();
        for c in ['a', 'b', 'c', 'd'] {
            connection
                .execute("INSERT INTO jobs(id) VALUES(?1)", [id(c).as_str()])
                .unwrap();
        }
        connection
    }

    #[test]
    fn current_footprint_and_delta_are_separate_flagged_readings() {
        let db = database();
        assert!(record(&db, &id('a'), Point::Start, Reading::Complete(4096)).unwrap());
        // The start reading is written once.
        assert!(record(&db, &id('a'), Point::Start, Reading::Complete(1)).unwrap());
        assert!(record(&db, &id('a'), Point::End, Reading::Complete(1024)).unwrap());
        // A retry after the directory changes must keep the first end reading.
        assert!(record(&db, &id('a'), Point::End, Reading::Complete(8192)).unwrap());
        let f = read(&db, &id('a')).unwrap().unwrap().footprint;
        assert_eq!(f.start_bytes.value, Some(4096));
        assert_eq!(f.end_bytes.value, Some(1024));
        assert_eq!(f.delta_bytes.value, Some(-3072));
        assert_eq!(f.delta_bytes.availability, Availability::Complete);
        for value in [
            &f.start_bytes.basis,
            &f.end_bytes.basis,
            &f.delta_bytes.basis,
        ] {
            assert_eq!(value, BASIS_ALLOCATED);
        }

        // A partial end keeps its lower bound; the delta is then unavailable.
        record(&db, &id('b'), Point::Start, Reading::Complete(10)).unwrap();
        record(&db, &id('b'), Point::End, Reading::Partial(20)).unwrap();
        let f = read(&db, &id('b')).unwrap().unwrap().footprint;
        assert_eq!(
            (f.end_bytes.value, f.end_bytes.availability),
            (Some(20), Availability::Partial)
        );
        assert_eq!(f.delta_bytes.value, None);
        assert_eq!(
            f.delta_bytes.reason.as_deref(),
            Some(REASON_READING_INCOMPLETE)
        );

        // A failed walk and a never-taken reading are unavailable, never 0.
        record(&db, &id('c'), Point::Start, Reading::Unavailable).unwrap();
        let f = read(&db, &id('c')).unwrap().unwrap().footprint;
        assert_eq!(f.start_bytes.reason.as_deref(), Some(REASON_WALK_FAILED));
        assert_eq!(f.end_bytes.reason.as_deref(), Some(REASON_NOT_MEASURED));
        let f = read(&db, &id('d')).unwrap().unwrap().footprint;
        assert_eq!(f.delta_bytes.reason.as_deref(), Some(REASON_NOT_MEASURED));
        for value in [&f.start_bytes, &f.end_bytes] {
            assert!(value.is_consistent() && value.value.is_none());
        }
        // An unknown job is neither recorded nor read.
        assert!(!record(&db, &id('e'), Point::End, Reading::Complete(1)).unwrap());
        assert_eq!(read(&db, &id('e')).unwrap(), None);
    }
}

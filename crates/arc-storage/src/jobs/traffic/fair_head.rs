//! Queue-head selection for one admission attempt (AT-TRF-08).
//!
//! `admission::try_grant` asks here which queued entry may be admitted, inside
//! its own IMMEDIATE transaction, and passes the answer to
//! `arc_traffic::decide` as `is_fifo_head`. This is the only place that orders
//! the queue for a grant.
//!
//! The earliest queued entry decides the class, as before. When it carries no
//! project key it is the head: a keyless entry passes nothing and nothing
//! passes it, so a queue without keys is ordered exactly as before. Otherwise
//! every project is represented by its own earliest queued entry (first in,
//! first out within a project), and among the representatives of that class
//! queued ahead of the first keyless entry the head is the one whose project
//! was granted a permit of that class least recently, a project never granted
//! first, ties by sequence. No weights, virtual time or second queue.
//!
//! Reported reasons. A waiter that is not the head still receives the `FIFO`
//! wait reply: it means "not the queue head" and sets the wrapper's poll
//! cadence. Its recorded decision, which diagnostics and the desktop read, is
//! `class_window` when the head was queued after it, so "waiting for an
//! earlier job" is never shown to a job that was passed over.
use super::{JobError, JobResult};
use arc_ipc::traffic::WaitReason;
use arc_policy::DecisionReason;
use rusqlite::{Connection, OptionalExtension};

/// The queued entry that may be admitted now, with today's FIFO head.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Head {
    pub sequence: u64,
    earliest: u64,
}

impl Head {
    /// The head was chosen ahead of an entry queued before it.
    pub(super) fn fair_turn(self) -> bool {
        self.sequence != self.earliest
    }

    /// The decision recorded for a waiting entry.
    pub(super) fn recorded(self, entry: u64, reason: WaitReason) -> DecisionReason {
        match reason {
            WaitReason::Fifo if entry < self.sequence => DecisionReason::ClassWindow,
            WaitReason::Fifo => DecisionReason::Fifo,
            WaitReason::ClassWindow => DecisionReason::ClassWindow,
            WaitReason::CpuBudget => DecisionReason::CpuBudget,
            WaitReason::MemoryBudget => DecisionReason::MemoryBudget,
        }
    }
}

const FAIR: &str = "SELECT h.sequence FROM (
       SELECT t.project_key AS project, MIN(t.sequence) AS sequence FROM traffic_entries t
       WHERE t.state='QUEUED' AND t.project_key IS NOT NULL AND t.sequence < COALESCE(
         (SELECT MIN(n.sequence) FROM traffic_entries n
          WHERE n.state='QUEUED' AND n.project_key IS NULL), 9223372036854775807)
       GROUP BY t.project_key) h
     JOIN traffic_entries e ON e.sequence=h.sequence JOIN jobs j ON j.id=e.job_id
     WHERE j.job_class=?1
     ORDER BY (SELECT MAX(g.granted_at_ms) FROM traffic_entries g JOIN jobs c ON c.id=g.job_id
               WHERE g.project_key=h.project AND g.root_job_id IS NULL AND c.job_class=?1),
              h.sequence
     LIMIT 1";

pub(super) fn select(connection: &Connection) -> JobResult<Head> {
    let (earliest, keyed, class): (i64, bool, String) = connection.query_row(
        "SELECT t.sequence, t.project_key IS NOT NULL, j.job_class FROM traffic_entries t
         JOIN jobs j ON j.id=t.job_id WHERE t.state='QUEUED' ORDER BY t.sequence LIMIT 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    let head = if keyed {
        connection
            .query_row(FAIR, [class], |row| row.get::<_, i64>(0))
            .optional()?
            .ok_or(JobError::StorageFailure)?
    } else {
        earliest
    };
    let unsigned = |value: i64| u64::try_from(value).map_err(|_| JobError::StorageFailure);
    Ok(Head {
        sequence: unsigned(head)?,
        earliest: unsigned(earliest)?,
    })
}

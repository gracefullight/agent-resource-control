//! Job footprint at its start and end attachment points (lane B: AT-MTR-02,
//! G-01 item 4).
//!
//! A reading is the allocated bytes (`st_blocks`) under the job's working
//! directory, the current directory of its verified observer (the `arcctl`
//! wrapper that starts the child there): a bounded walk on one device that
//! follows no symlink (`arc_platform::footprint`), run on the blocking pool,
//! off the SQLite writer and holding no ticket. It is stored once in
//! `jobs.footprint_*` (migration 0010); a walk that hit its bound is partial
//! and one that could not run is unavailable, never 0. A path is held only in
//! memory and never stored, logged or sent.
//!
//! Each reading is bound to the one writer call whose own transaction made
//! it due. The writer reads the job's [`Marks`] immediately before and after
//! that call's operation ([`watched`]), so no other request commits between
//! them:
//!
//! - **End** (a lifecycle, Traffic or Governor completion): the walk runs
//!   *before* the completion is submitted, while the job still holds its
//!   permit, reservation or release and the child has exited. The writer
//!   stores it right after the commit that set the outcome, before it serves
//!   any other request, or stores `unavailable` when no reading was taken.
//!   So no successor admitted by the release, no lifecycle status and no
//!   replay can observe the outcome before its reading is bound, and a later
//!   walk can never replace it.
//! - **Start of a registration** (a lifecycle `Register`): measured before
//!   the registration is submitted and stored by the call that created it;
//!   the child is spawned only after its reply.
//! - **Start of a Traffic grant or Governor release**: the grant is decided
//!   by the writer, so the walk runs after the commit that set it and before
//!   its reply, and only that call is due it. A per-job guard serializes the
//!   job's requests (polls, replays, status) with that walk, so nothing that
//!   lets the child run is returned first. A cancelled, timed-out or failed
//!   original leaves the start not measured; a replay finds the mark already
//!   set and takes none.
//!
//! The hooks never change a lifecycle reply.
use super::DbClient;
use arc_ipc::jobs::{JobId, ProcessIdentity, StartRequest};
use arc_ipc::traffic::JobRequest;
use arc_platform::footprint::{self as native, Bounds, Footprint};
use arc_storage::{
    footprint::{self as stored, Point, Reading},
    jobs::JobError,
};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock, Weak},
    time::{Duration, Instant},
};

type JobLock = tokio::sync::Mutex<()>;
static JOB_LOCKS: OnceLock<Mutex<HashMap<String, Weak<JobLock>>>> = OnceLock::new();

/// Serialize each job's requests with its off-writer readings, so a replay,
/// poll or status of the same job cannot return a grant before its start
/// reading is stored. Weak entries are removed on subsequent admissions, so
/// idle jobs are not retained. Dropping a cancelled handler also drops its
/// walk future: a late blocking walk cannot submit a reading after the guard
/// is released, and a reading not yet bound is never submitted later.
pub(crate) async fn guard(
    job: &JobId,
    deadline: Instant,
) -> Result<tokio::sync::OwnedMutexGuard<()>, &'static str> {
    let lock = {
        let mut locks = JOB_LOCKS
            .get_or_init(|| Mutex::new(HashMap::new()))
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        locks.retain(|_, lock| lock.strong_count() != 0);
        match locks.get(job.as_str()).and_then(Weak::upgrade) {
            Some(lock) => lock,
            None => {
                let lock = Arc::new(JobLock::new(()));
                locks.insert(job.as_str().to_owned(), Arc::downgrade(&lock));
                lock
            }
        }
    };
    if let Ok(guard) = Arc::clone(&lock).try_lock_owned() {
        return Ok(guard);
    }
    fixture_gate::waiting(job);
    tokio::time::timeout_at(tokio::time::Instant::from_std(deadline), lock.lock_owned())
        .await
        .map_err(|_| arc_ipc::ERROR_DB_BUSY)
}

/// The longest one walk may take; a lifecycle reply waits for it.
const WALK_BUDGET: Duration = Duration::from_millis(1500);

/// One reading for the single writer.
pub(crate) enum Operation {
    Record {
        job_id: JobId,
        point: Point,
        reading: Reading,
    },
}
pub(crate) enum Value {
    Recorded(Result<bool, JobError>),
}

/// Returns `jobs::Value::Footprint(..)`.
pub(crate) fn execute(
    connection: &mut rusqlite::Connection,
    operation: Operation,
) -> super::jobs::Value {
    match operation {
        Operation::Record {
            job_id,
            point,
            reading,
        } => super::jobs::Value::Footprint(Value::Recorded(stored::record(
            connection, &job_id, point, reading,
        ))),
    }
}

/// The allocated bytes under `observer`'s working directory, on the
/// blocking pool, within the walk budget and `deadline`.
async fn measure(
    job: &JobId,
    point: Point,
    observer: ProcessIdentity,
    deadline: Instant,
) -> Reading {
    let directory = tokio::task::spawn_blocking(move || native::working_directory(&observer)).await;
    let Ok(Some(directory)) = directory else {
        return Reading::Unavailable;
    };
    if !fixture_gate::after_capture(job, point, deadline).await {
        return Reading::Unavailable;
    }
    let remaining = deadline.saturating_duration_since(Instant::now());
    let end = Instant::now() + WALK_BUDGET.min(remaining / 3);
    let walk = tokio::task::spawn_blocking(move || {
        native::allocated_bytes(
            &directory,
            Bounds {
                max_entries: native::MAX_ENTRIES,
                deadline: end,
            },
        )
    });
    let reading = match walk.await {
        Ok(Footprint::Complete(bytes)) => Reading::Complete(bytes),
        Ok(Footprint::Partial(bytes)) => Reading::Partial(bytes),
        Ok(Footprint::Unavailable) | Err(_) => Reading::Unavailable,
    };
    fixture_gate::measured(job, point, reading);
    reading
}

/// A reading taken before a writer call, for the writer to bind to it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Bound {
    /// The start reading of a registration this call may create.
    pub registration: Option<Reading>,
    /// The end reading of an outcome this call may commit.
    pub end: Option<Reading>,
}

/// The start reading of a lifecycle registration, before it is submitted;
/// `None` for a claim or attach. `Err` when a fixture barrier outlived the
/// deadline: nothing is submitted then.
pub(crate) async fn before_registration(
    request: &StartRequest,
    deadline: Instant,
) -> Result<Bound, &'static str> {
    let StartRequest::Register {
        job_id, observer, ..
    } = request
    else {
        return Ok(Bound::default());
    };
    if !fixture_gate::checkpoint(job_id, Point::Start, deadline).await {
        return Err(arc_ipc::ERROR_DB_BUSY);
    }
    Ok(Bound {
        registration: Some(measure(job_id, Point::Start, observer.clone(), deadline).await),
        end: None,
    })
}

/// The end reading of a completion, before it is submitted: the child has
/// exited and the job still holds whatever the completion releases. `Err`
/// when a fixture barrier outlived the deadline: nothing is submitted then.
pub(crate) async fn before_end(
    job_id: &JobId,
    observer: &ProcessIdentity,
    deadline: Instant,
) -> Result<Bound, &'static str> {
    if !fixture_gate::checkpoint(job_id, Point::End, deadline).await {
        return Err(arc_ipc::ERROR_DB_BUSY);
    }
    Ok(Bound {
        registration: None,
        end: Some(measure(job_id, Point::End, observer.clone(), deadline).await),
    })
}

/// Whether a job's registration, start and end have committed: a lifecycle
/// row, a Traffic grant or a Governor release, and a recorded outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Marks {
    registered: bool,
    started: bool,
    ended: bool,
}

/// The committed marks of `job` on the writer's own connection, or `None`
/// when they cannot be read (then no reading is taken).
pub(crate) fn marks(connection: &rusqlite::Connection, job: &JobId) -> Option<Marks> {
    connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM job_lifecycle WHERE job_id=?1),
                    EXISTS(SELECT 1 FROM traffic_entries WHERE job_id=?1 AND granted_at_ms IS NOT NULL)
                 OR EXISTS(SELECT 1 FROM governor_jobs WHERE job_id=?1 AND release_id IS NOT NULL),
                    EXISTS(SELECT 1 FROM job_lifecycle WHERE job_id=?1 AND outcome_kind IS NOT NULL)",
            [job.as_str()],
            |r| {
                Ok(Marks {
                    registered: r.get(0)?,
                    started: r.get(1)?,
                    ended: r.get(2)?,
                })
            },
        )
        .ok()
}

/// What the one writer call between `before` and `after` made due: its own
/// transaction created the registration, committed the grant (or release),
/// or committed the outcome. A replay or a concurrent attempt finds the mark
/// already set and is due nothing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Due {
    pub registration: bool,
    pub start: bool,
    pub end: bool,
}
pub(crate) fn transition(before: Option<Marks>, after: Option<Marks>) -> Due {
    let (Some(before), Some(after)) = (before, after) else {
        return Due::default();
    };
    Due {
        registration: after.registered && !before.registered,
        start: after.started && !before.started,
        end: after.ended && !before.ended,
    }
}

/// On the writer, immediately after the call's commit: store the readings
/// that `bound` carries for what it made due. An outcome committed without
/// a reading is bound `unavailable`, so nothing later can supply one. Returns
/// whether a grant or release made a start reading due after the commit.
pub(crate) fn bind(
    connection: &rusqlite::Connection,
    job: &JobId,
    due: Due,
    bound: Bound,
) -> Option<Point> {
    if due.end {
        let reading = bound.end.unwrap_or(Reading::Unavailable);
        let _ = stored::record(connection, job, Point::End, reading);
    }
    if due.registration {
        if let Some(reading) = bound.registration {
            let _ = stored::record(connection, job, Point::Start, reading);
        }
    }
    (due.start && !due.end).then_some(Point::Start)
}

/// Run `operation` on the writer with the readings `bound` carries, also
/// reporting a start reading its own commit made due, if any.
pub(crate) async fn watched(
    client: &DbClient,
    operation: super::jobs::Operation,
    job_id: &JobId,
    bound: Bound,
    deadline: Instant,
) -> Result<(super::jobs::Value, Option<Point>), &'static str> {
    match super::jobs::call(
        client,
        super::jobs::Operation::Watched(Box::new(operation), job_id.clone(), bound),
        deadline,
    )
    .await?
    {
        super::jobs::Value::Watched(value, point) => Ok((*value, point)),
        _ => Err(arc_ipc::ERROR_DB_BUSY),
    }
}

/// The job and observer whose readings a Traffic request can make due.
pub(crate) fn traffic_job(request: &JobRequest) -> Option<(&JobId, &ProcessIdentity)> {
    match request {
        JobRequest::Enqueue {
            job_id, observer, ..
        }
        | JobRequest::Poll { job_id, observer }
        | JobRequest::Complete {
            job_id, observer, ..
        } => Some((job_id, observer)),
        _ => None,
    }
}

/// The end reading of a Traffic completion before it is submitted; nothing
/// for any other request.
pub(crate) async fn before_traffic(
    request: &JobRequest,
    deadline: Instant,
) -> Result<Bound, &'static str> {
    match request {
        JobRequest::Complete {
            job_id, observer, ..
        } => before_end(job_id, observer, deadline).await,
        _ => Ok(Bound::default()),
    }
}

/// Run a Traffic `operation` for `request` on the writer, watched when the
/// request can make a reading due.
pub(crate) async fn traffic(
    client: &DbClient,
    request: &JobRequest,
    operation: super::traffic::Operation,
    bound: Bound,
    deadline: Instant,
) -> Result<(super::traffic::Value, Option<Point>), &'static str> {
    let operation = super::jobs::Operation::Traffic(operation);
    let (value, point) = match traffic_job(request) {
        Some((job_id, _)) => watched(client, operation, job_id, bound, deadline).await?,
        None => (super::jobs::call(client, operation, deadline).await?, None),
    };
    match value {
        super::jobs::Value::Traffic(value) => Ok((value, point)),
        _ => Err(arc_ipc::ERROR_DB_BUSY),
    }
}

/// [`after_commit`] for the job a Traffic request names.
pub(crate) async fn after_traffic(
    client: &DbClient,
    request: &JobRequest,
    point: Option<Point>,
    deadline: Instant,
) {
    if let Some((job_id, observer)) = traffic_job(request) {
        after_commit(client, job_id, observer, point, deadline).await;
    }
}

/// Traffic and Governor attachment point: after the writer call whose own
/// transaction committed the grant or release (`point`), measure and store
/// that start reading once, before the reply lets the child run. `None`
/// takes no reading.
pub(crate) async fn after_commit(
    client: &DbClient,
    job_id: &JobId,
    observer: &ProcessIdentity,
    point: Option<Point>,
    deadline: Instant,
) {
    let Some(point) = point else {
        return;
    };
    if !fixture_gate::checkpoint(job_id, point, deadline).await {
        return;
    }
    let reading = measure(job_id, point, observer.clone(), deadline).await;
    let stored = super::jobs::call(
        client,
        super::jobs::Operation::Footprint(Operation::Record {
            job_id: job_id.clone(),
            point,
            reading,
        }),
        deadline,
    )
    .await;
    // A busy writer leaves the start not measured; the reply is unchanged.
    let _stored = matches!(
        stored,
        Ok(super::jobs::Value::Footprint(Value::Recorded(Ok(true))))
    );
}

#[path = "footprint_gate.rs"]
mod fixture_gate;

#[cfg(test)]
#[path = "footprint_tests.rs"]
mod tests;

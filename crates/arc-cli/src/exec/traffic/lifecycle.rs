//! Traffic lifecycle acknowledgements never authorize a second execution.
use super::{job, Route};
use crate::exec::{preparation::write_notice, transport::Continuity};
use arc_ipc::{
    jobs::{JobId, Outcome, ProcessIdentity},
    traffic::{JobRequest, JobResult},
};
use std::{
    path::Path,
    time::{Duration, Instant},
};

pub(in crate::exec) fn cancel(
    socket: &Path,
    id: &JobId,
    observer: &ProcessIdentity,
    generation: Option<u64>,
    route: Route,
    continuity: &Continuity,
) -> bool {
    let request = JobRequest::Cancel {
        job_id: id.clone(),
        observer: observer.clone(),
        // A Governor pause can retire this generation before a no-spawn
        // cleanup reaches the writer. Cancellation belongs to this owner/job,
        // including a later readmission of the same job. The writer still
        // retains any attached/running child; legacy generation checks remain.
        permit_generation: if route == Route::Governor {
            None
        } else {
            generation
        },
    };
    let result = job(
        socket,
        &request,
        route,
        Instant::now() + Duration::from_millis(500),
        &|| false,
        &mut false,
        continuity,
    );
    let confirmed = matches!(result, Ok(JobResult::Cancelled {})) && continuity.confirmed();
    if !confirmed {
        write_notice("QUEUE_CLEANUP_UNCONFIRMED\nQueue cleanup is unconfirmed.\n");
    }
    confirmed
}

pub(in crate::exec) fn attach(
    socket: &Path,
    id: &JobId,
    observer: &ProcessIdentity,
    generation: u64,
    child: ProcessIdentity,
    route: Route,
    continuity: &Continuity,
) -> bool {
    let request = JobRequest::Attach {
        job_id: id.clone(),
        observer: observer.clone(),
        child,
        permit_generation: generation,
    };
    matches!(
        job(
            socket,
            &request,
            route,
            Instant::now() + Duration::from_secs(2),
            &|| false,
            &mut false,
            continuity,
        ),
        Ok(JobResult::Running {})
    )
}

pub(in crate::exec) fn complete(
    socket: &Path,
    id: &JobId,
    observer: &ProcessIdentity,
    generation: u64,
    outcome: Outcome,
    route: Route,
    continuity: &Continuity,
) -> bool {
    let request = JobRequest::Complete {
        job_id: id.clone(),
        observer: observer.clone(),
        outcome: outcome.clone(),
        permit_generation: generation,
    };
    matches!(job(socket, &request, route, Instant::now() + Duration::from_secs(2), &|| false, &mut false, continuity), Ok(JobResult::Completed { outcome: actual, .. }) if actual == outcome)
}

pub(in crate::exec) fn complete_or_warn(
    socket: &Path,
    id: &JobId,
    observer: &ProcessIdentity,
    generation: u64,
    outcome: Outcome,
    route: Route,
    continuity: &Continuity,
) -> bool {
    let completed = complete(socket, id, observer, generation, outcome, route, continuity)
        && continuity.confirmed();
    if !completed {
        write_notice("COMPLETION_UNCONFIRMED\nRecording is unconfirmed.\n");
    }
    completed
}

pub(in crate::exec) fn fallback(
    socket: &Path,
    id: &JobId,
    observer: &ProcessIdentity,
    child: Option<ProcessIdentity>,
    outcome: Option<Outcome>,
    route: Route,
    continuity: &Continuity,
) -> bool {
    let request = JobRequest::Fallback {
        job_id: id.clone(),
        observer: observer.clone(),
        permit_generation: None,
        child,
        outcome,
    };
    matches!(
        job(
            socket,
            &request,
            route,
            Instant::now() + Duration::from_secs(2),
            &|| false,
            &mut false,
            continuity,
        ),
        Ok(JobResult::Completed { .. } | JobResult::Abandoned {} | JobResult::Cancelled {})
    )
}

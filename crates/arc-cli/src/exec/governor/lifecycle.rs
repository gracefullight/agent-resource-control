//! Completion and attachment for Governor-owned unreserved releases.
use super::*;

pub(in crate::exec) fn attach(
    socket: &Path,
    id: &JobId,
    observer: &ProcessIdentity,
    release: &str,
    child: ProcessIdentity,
    continuity: &Continuity,
) -> bool {
    matches!(
        call(
            socket,
            &GovernorJobRequest::Attach {
                job_id: id.clone(),
                observer: observer.clone(),
                release_id: release.into(),
                child
            },
            Instant::now() + Duration::from_secs(2),
            &|| false,
            continuity
        ),
        Ok(GovernorJobResult::Traffic {
            result: JobResult::Running {}
        })
    ) && continuity.confirmed()
}
pub(in crate::exec) fn complete(
    socket: &Path,
    id: &JobId,
    observer: &ProcessIdentity,
    release: &str,
    outcome: arc_ipc::jobs::Outcome,
    continuity: &Continuity,
) -> bool {
    matches!(call(socket, &GovernorJobRequest::Complete { job_id: id.clone(), observer: observer.clone(), release_id: release.into(), outcome: outcome.clone() }, Instant::now()+Duration::from_secs(2), &|| false, continuity), Ok(GovernorJobResult::Traffic { result: JobResult::Completed { outcome: actual, .. } }) if actual == outcome)
        && continuity.confirmed()
}

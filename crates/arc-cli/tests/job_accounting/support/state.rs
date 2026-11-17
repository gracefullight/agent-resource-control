use super::*;

pub(super) fn capture_relation(
    state: &mut State,
    socket: &std::path::Path,
    observer: &arc_core::process_identity::ProcessIdentity,
    id: &str,
) {
    let relation = arc_platform::job_lineage::relation(socket, observer, id);
    if matches!(relation, AccountingRelation::Unverified {}) {
        let detail = match arc_platform::job_lineage::ancestors(observer) {
            Ok(chain) => format!(
                "walked {} ancestors; an ancestry record may be unverified",
                chain.len()
            ),
            Err(error) => format!("native ancestry unavailable: {error}"),
        };
        state.ancestry_diagnostics.insert(id.to_owned(), detail);
    }
    state.relations.insert(id.to_owned(), relation);
}

pub(super) fn registered(
    job_id: JobId,
    observer: ProcessIdentity,
    classification: arc_ipc::Classification,
) -> JobSnapshot {
    JobSnapshot {
        job_id,
        observer,
        classification,
        phase: Phase::Registered,
        observer_verified_at_ms: 1,
        child: None,
        child_verified_at_ms: None,
        observation: IdentityObservation {
            target: ObservationTarget::Observer,
            status: ObservationStatus::VerifiedMatch,
            observed_at_ms: 1,
            reason: LifecycleReason::RegisteredNotClaimed,
        },
        reason: LifecycleReason::RegisteredNotClaimed,
        outcome: None,
        registered_at_ms: 1,
        launch_claimed_at_ms: None,
        completed_at_ms: None,
        updated_at_ms: 1,
        revision: 1,
    }
}
pub(super) fn claimed(job: &mut JobSnapshot) {
    job.phase = Phase::Launching;
    job.launch_claimed_at_ms = Some(1);
}
pub(super) fn attached(job: &mut JobSnapshot, child: ProcessIdentity) {
    job.child = Some(child);
    job.child_verified_at_ms = Some(1);
    job.phase = Phase::Running;
    job.reason = LifecycleReason::ChildVerified;
    job.observation = IdentityObservation {
        target: ObservationTarget::Child,
        status: ObservationStatus::VerifiedMatch,
        observed_at_ms: 1,
        reason: job.reason,
    };
}
pub(super) fn completed(job: &mut JobSnapshot, outcome: Outcome) {
    (job.phase, job.reason) = match &outcome {
        Outcome::Exited { .. } => (Phase::Completed, LifecycleReason::ObservedExit),
        Outcome::Signaled { .. } => (Phase::Signaled, LifecycleReason::ObservedSignal),
        Outcome::SpawnFailed { .. } => (Phase::SpawnFailed, LifecycleReason::ObservedSpawnFailure),
    };
    job.observation.reason = job.reason;
    job.completed_at_ms = Some(1);
    job.outcome = Some(outcome);
}
pub(super) fn queued_result(
    job_id: &JobId,
    state: &mut State,
    peer: &QueuedPeer,
) -> Option<JobResult> {
    if !state.queued_poll_seen {
        state.queued_poll_seen = true;
        peer.reached.send(()).ok()?;
    }
    match peer.release.try_recv() {
        Ok(()) => Some(JobResult::Granted {
            sequence: 1,
            queued_at_ms: 1,
            granted_at_ms: 291,
            permit_generation: 1,
            reservation: Resources {
                cpu_millis: 1000,
                memory_bytes: 1024,
            },
            lineage_context: state.contexts[job_id.as_str()].clone(),
        }),
        Err(mpsc::TryRecvError::Empty) => Some(JobResult::Queued {
            sequence: 1,
            queued_at_ms: 1,
            reason: arc_ipc::traffic::WaitReason::ClassWindow,
        }),
        Err(mpsc::TryRecvError::Disconnected) => None,
    }
}

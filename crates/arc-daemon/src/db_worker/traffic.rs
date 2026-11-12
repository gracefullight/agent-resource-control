//! Traffic uses the existing bounded SQLite worker; native probes happen outside it.
use super::{jobs, DbClient};
use arc_core::process_identity::{probe_live, IdentityStatus, ProcessIdentity};
use arc_ipc::{
    jobs::{
        JobId, JobSnapshot, LifecycleRequest, ObservationStatus, ObservationTarget, Phase,
        StatusRequest, StatusResult as LifecycleStatus,
    },
    traffic::{JobRequest, LineageContext, StatusResult as TrafficStatus},
};
use arc_storage::jobs::{traffic, JobError};
use std::{
    io::{self, Read},
    time::Instant,
};

#[path = "traffic_recovery.rs"]
mod recovery;
#[path = "traffic_witness.rs"]
mod witness;
use recovery::recover;

pub(crate) fn recover_at_start(connection: &mut rusqlite::Connection) -> Result<(), JobError> {
    recovery::recover_at_start(connection)
}

pub(crate) struct RequestOperation {
    request: JobRequest,
    owner: Option<traffic::OwnerEvidence>,
    child: Option<traffic::ChildEvidence>,
    context: LineageContext,
    proof: Option<traffic::LineageProof>,
    now: u64,
}

pub(crate) enum Operation {
    Request(Box<RequestOperation>),
    Head,
    ReconcileQueue(traffic::QueueObservation),
    RecoverWitness(Box<traffic::WitnessEvidence>),
    RecoverUnspawned(Box<traffic::UnspawnedEvidence>),
    RecoverUncertain(Box<traffic::UncertainEvidence>),
    Candidates(u64),
    Lineage(Vec<ProcessIdentity>),
    Status(JobId),
}

pub(crate) enum Value {
    Reply(Result<serde_json::Value, JobError>),
    Head(Result<Option<Box<JobSnapshot>>, JobError>),
    Candidates(Result<Vec<traffic::RecoveryCandidate>, JobError>),
    Lineage(Result<Vec<(traffic::LineageSnapshot, ProcessIdentity)>, JobError>),
    Status(Result<TrafficStatus, JobError>),
    Witness(Result<bool, JobError>),
    Uncertain(Result<traffic::UncertainResult, JobError>),
}

#[cfg(test)]
#[allow(dead_code)] // Standalone worker targets retain this adapter.
pub(crate) fn execute(connection: &mut rusqlite::Connection, op: Operation) -> Value {
    execute_observed(connection, op, None)
}
pub(crate) fn execute_observed(
    connection: &mut rusqlite::Connection,
    op: Operation,
    reader: Option<&super::observation::Observations>,
) -> Value {
    let provider: &dyn arc_policy::ObservationProvider = reader
        .map(|r| r as &dyn arc_policy::ObservationProvider)
        .unwrap_or(&arc_policy::evaluator::MissingObservation);
    match op {
        Operation::Head => {
            Value::Head(traffic::queue_head(connection).map(|job| job.map(Box::new)))
        }
        Operation::Candidates(after) => {
            Value::Candidates(traffic::recovery_candidates(connection, after, 64))
        }
        Operation::Lineage(ancestors) => {
            Value::Lineage(traffic::lineage_anchors(connection, &ancestors))
        }
        Operation::Status(id) => Value::Status(traffic::status(connection, &id)),
        Operation::ReconcileQueue(observation) => Value::Reply(
            traffic::reconcile_queued(connection, &observation).map(|_| serde_json::Value::Null),
        ),
        Operation::RecoverWitness(evidence) => {
            Value::Witness(traffic::recover_witness(connection, &evidence))
        }
        Operation::RecoverUnspawned(evidence) => {
            Value::Witness(traffic::recover_unspawned(connection, &evidence))
        }
        Operation::RecoverUncertain(evidence) => {
            Value::Uncertain(traffic::reconcile_uncertain(connection, &evidence))
        }
        Operation::Request(input) => {
            let RequestOperation {
                request,
                owner,
                child,
                context,
                proof,
                now,
            } = *input;
            let reply = match &request {
                JobRequest::Enqueue { .. } => traffic::enqueue_observed(
                    connection,
                    &request,
                    owner.as_ref().expect("owner evidence"),
                    &arc_traffic::Policy::default(),
                    &context,
                    proof.as_ref(),
                    now,
                    provider,
                ),
                JobRequest::Poll { .. } => traffic::poll_observed(
                    connection,
                    &request,
                    owner.as_ref().expect("owner evidence"),
                    &arc_traffic::Policy::default(),
                    &context,
                    proof.as_ref(),
                    now,
                    provider,
                ),
                JobRequest::Attach { .. } => traffic::attach(
                    connection,
                    &request,
                    child.as_ref().expect("child evidence"),
                    now,
                ),
                JobRequest::Complete { .. } => traffic::complete(connection, &request, now),
                JobRequest::Cancel { .. } => traffic::cancel_observed(
                    connection,
                    &request,
                    owner.as_ref().expect("owner evidence"),
                    now,
                    provider,
                ),
                JobRequest::Fallback { .. } => {
                    traffic::fallback(connection, &request, child.as_ref(), now)
                }
                JobRequest::Status { job_id } | JobRequest::StatusV2 { job_id } => {
                    return Value::Reply(traffic::status(connection, job_id).and_then(|reply| {
                        serde_json::to_value(reply).map_err(|_| JobError::StorageFailure)
                    }))
                }
            };
            Value::Reply(reply.and_then(|reply| {
                serde_json::to_value(reply).map_err(|_| JobError::StorageFailure)
            }))
        }
    }
}

async fn call(
    client: &DbClient,
    operation: Operation,
    deadline: Instant,
) -> Result<Value, &'static str> {
    match jobs::call(client, jobs::Operation::Traffic(operation), deadline).await? {
        jobs::Value::Traffic(value) => Ok(value),
        _ => Err(arc_ipc::ERROR_DB_BUSY),
    }
}

fn context() -> io::Result<LineageContext> {
    let mut bytes = [0u8; 32];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    LineageContext::try_from(bytes.iter().map(|b| format!("{b:02x}")).collect::<String>())
        .map_err(io::Error::other)
}

/// Only identities bound to the actual socket opener can mutate a job.
pub(crate) async fn handle(
    client: DbClient,
    request: JobRequest,
    peer_pid: u32,
    deadline: Instant,
) -> Result<serde_json::Value, &'static str> {
    #[cfg(test)]
    return handle_inner(client, request, peer_pid, deadline, |_| {}).await;
    #[cfg(not(test))]
    handle_inner(client, request, peer_pid, deadline).await
}

async fn handle_inner(
    client: DbClient,
    request: JobRequest,
    peer_pid: u32,
    deadline: Instant,
    #[cfg(test)] mut after_proof: impl FnMut(bool),
) -> Result<serde_json::Value, &'static str> {
    if let JobRequest::Status { job_id } | JobRequest::StatusV2 { job_id } = &request {
        return status(
            &client,
            job_id,
            matches!(&request, JobRequest::StatusV2 { .. }),
            deadline,
        )
        .await;
    }
    let observer = match &request {
        JobRequest::Enqueue { observer, .. }
        | JobRequest::Poll { observer, .. }
        | JobRequest::Attach { observer, .. }
        | JobRequest::Complete { observer, .. }
        | JobRequest::Cancel { observer, .. }
        | JobRequest::Fallback { observer, .. } => Some(observer),
        JobRequest::Status { .. } | JobRequest::StatusV2 { .. } => None,
    };
    if let Some(observer) = observer {
        if observer.pid != peer_pid || probe_live(observer) != IdentityStatus::VerifiedMatch {
            return Err(arc_ipc::ERROR_INVALID_REQUEST);
        }
    }
    if matches!(
        request,
        JobRequest::Enqueue { .. } | JobRequest::Poll { .. }
    ) {
        recover(&client, deadline).await?;
    }
    let child = match &request {
        JobRequest::Attach {
            observer, child, ..
        }
        | JobRequest::Fallback {
            observer,
            child: Some(child),
            ..
        } => {
            if !is_descendant(child, observer) && !super::job_meter::owned_child(child, observer) {
                return Err(arc_ipc::ERROR_INVALID_REQUEST);
            }
            Some(
                traffic::ChildEvidence::gather(child)
                    .with_meter_evidence(super::job_meter::evidence(observer, Some(child))),
            )
        }
        _ => None,
    };
    // Attachment changes the anchor's revision. If it races the transaction,
    // gather the snapshot and native ancestry again instead of dropping proof.
    // Each retry uses a fresh bounded worker ticket and the same request ID.
    for _ in 0..3 {
        let proof = lineage_proof(&client, &request, deadline).await?;
        let had_proof = proof.is_some();
        #[cfg(test)]
        after_proof(had_proof);
        let owner = observer.map(|observer| {
            traffic::OwnerEvidence::gather(observer)
                .with_meter_evidence(super::job_meter::evidence(observer, None))
        });
        let context = context().map_err(|_| arc_ipc::ERROR_DB_BUSY)?;
        match call(
            &client,
            Operation::Request(Box::new(RequestOperation {
                request: request.clone(),
                owner,
                child: child.clone(),
                context,
                proof,
                now: jobs::now(),
            })),
            deadline,
        )
        .await?
        {
            Value::Reply(Err(JobError::IdentityUnavailable)) if had_proof => continue,
            Value::Reply(value) => return value.map_err(JobError::code),
            _ => return Err(arc_ipc::ERROR_DB_BUSY),
        }
    }
    Err(arc_ipc::ERROR_DB_BUSY)
}

#[cfg(test)]
#[path = "traffic_fault_tests.rs"]
mod fault_tests;
#[cfg(test)]
#[path = "traffic_launch_fixture.rs"]
mod launch_fixture;
#[cfg(test)]
#[path = "traffic_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "traffic_witness_repair_tests.rs"]
mod witness_repair_tests;

pub(super) async fn lineage_proof(
    client: &DbClient,
    request: &JobRequest,
    deadline: Instant,
) -> Result<Option<traffic::LineageProof>, &'static str> {
    let observer = match request {
        JobRequest::Enqueue { observer, .. } | JobRequest::Poll { observer, .. } => observer,
        _ => return Ok(None),
    };
    let ancestors = live_ancestors(observer);
    if ancestors.is_empty() {
        return Ok(None);
    }
    let snapshots = match call(client, Operation::Lineage(ancestors), deadline).await? {
        Value::Lineage(value) => value.map_err(JobError::code)?,
        _ => return Err(arc_ipc::ERROR_DB_BUSY),
    };
    Ok(snapshots.into_iter().find_map(|(snapshot, anchor)| {
        is_descendant(observer, &anchor).then(|| {
            traffic::LineageProof::verified_with_recheck(&snapshot, &anchor, is_descendant)
        })
    }))
}

/// Capture a bounded chain from the authenticated socket peer. It is only a
/// candidate lookup: the chosen path is independently checked again at grant.
fn live_ancestors(observer: &ProcessIdentity) -> Vec<ProcessIdentity> {
    let mut current = observer.clone();
    let mut result = Vec::new();
    for _ in 0..128 {
        let Ok(pid) = parent_pid(current.pid) else {
            break;
        };
        if pid == 0 || pid == current.pid || probe_live(&current) != IdentityStatus::VerifiedMatch {
            break;
        }
        let Ok(parent) = ProcessIdentity::capture(pid) else {
            break;
        };
        if probe_live(&parent) != IdentityStatus::VerifiedMatch || result.contains(&parent) {
            break;
        }
        result.push(parent.clone());
        current = parent;
    }
    result
}

/// Status must observe a child before describing it as running. Use the legacy
/// by-ID reconciliation, which applies verified observations transactionally,
/// then read the Traffic state after any recovery has released its permit.
async fn status(
    client: &DbClient,
    id: &JobId,
    extended: bool,
    deadline: Instant,
) -> Result<serde_json::Value, &'static str> {
    recover(client, deadline).await?;
    // Legacy lifecycle records have no Traffic row. Avoid a fresh lifecycle
    // observation for them: the caller can use the protocol-1 result directly.
    match call(client, Operation::Status(id.clone()), deadline).await? {
        Value::Status(value) if matches!(value.as_ref(), Ok(TrafficStatus::NotFound {})) => {
            return serde_json::to_value(TrafficStatus::NotFound {})
                .map_err(|_| arc_ipc::ERROR_DB_BUSY);
        }
        Value::Status(Err(error)) => return Err(error.code()),
        Value::Status(Ok(_)) => (),
        _ => return Err(arc_ipc::ERROR_DB_BUSY),
    }
    let lifecycle = jobs::handle(
        client.clone(),
        LifecycleRequest::Status(StatusRequest::ById { job_id: id.clone() }),
        deadline,
    )
    .await?;
    let lifecycle: LifecycleStatus =
        serde_json::from_value(lifecycle).map_err(|_| arc_ipc::ERROR_DB_BUSY)?;
    if let LifecycleStatus::Found { ref job } = lifecycle {
        // This checks the requested row, not only the FIFO head. The storage
        // transaction rechecks its state and revision before changing it.
        let observation = traffic::QueueObservation::gather(job, jobs::now());
        match call(client, Operation::ReconcileQueue(observation), deadline).await? {
            Value::Reply(value) => {
                value.map_err(JobError::code)?;
            }
            _ => return Err(arc_ipc::ERROR_DB_BUSY),
        }
    }
    let verified_running = matches!(lifecycle,
        LifecycleStatus::Found { ref job }
            if job.phase == Phase::Running
                && job.child.is_some()
                && job.observation.target == ObservationTarget::Child
                && job.observation.status == ObservationStatus::VerifiedMatch
    );
    let state = match call(client, Operation::Status(id.clone()), deadline).await? {
        Value::Status(value) => value.map_err(JobError::code)?,
        _ => return Err(arc_ipc::ERROR_DB_BUSY),
    };
    let state = match state {
        TrafficStatus::Reserved {
            queued_at_ms,
            granted_at_ms,
            ..
        } if verified_running => TrafficStatus::Running {
            queue_wait_ms: granted_at_ms
                .checked_sub(queued_at_ms)
                .ok_or(arc_ipc::ERROR_DB_BUSY)?,
        },
        TrafficStatus::Reserved { .. } if extended => TrafficStatus::Unconfirmed {},
        // If no current identity confirms a recorded running child, let the
        // caller fall back to the reconciled lifecycle's unknown status.
        TrafficStatus::Running { .. } if !verified_running => return Err(arc_ipc::ERROR_DB_BUSY),
        state => state,
    };
    serde_json::to_value(state).map_err(|_| arc_ipc::ERROR_DB_BUSY)
}

/// Check each native parent link and identity again; unknown ancestry fails closed.
pub(super) fn is_descendant(descendant: &ProcessIdentity, ancestor: &ProcessIdentity) -> bool {
    if descendant == ancestor
        || probe_live(descendant) != IdentityStatus::VerifiedMatch
        || probe_live(ancestor) != IdentityStatus::VerifiedMatch
    {
        return false;
    }
    let mut current = descendant.clone();
    for _ in 0..128 {
        let Ok(parent_pid) = parent_pid(current.pid) else {
            return false;
        };
        if probe_live(&current) != IdentityStatus::VerifiedMatch
            || parent_pid == 0
            || parent_pid == current.pid
        {
            return false;
        }
        let Ok(parent) = ProcessIdentity::capture(parent_pid) else {
            return false;
        };
        if parent == *ancestor {
            return probe_live(ancestor) == IdentityStatus::VerifiedMatch;
        }
        current = parent;
    }
    false
}

pub(super) fn parent_pid(pid: u32) -> io::Result<u32> {
    #[cfg(target_os = "macos")]
    {
        let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
        let size = std::mem::size_of_val(&info) as libc::c_int;
        // SAFETY: info is a writable proc_bsdinfo buffer with its exact size.
        if unsafe {
            libc::proc_pidinfo(
                pid as i32,
                libc::PROC_PIDTBSDINFO,
                0,
                (&mut info as *mut libc::proc_bsdinfo).cast(),
                size,
            )
        } != size
        {
            return Err(io::Error::last_os_error());
        }
        Ok(info.pbi_ppid)
    }
    #[cfg(target_os = "linux")]
    {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat"))?;
        let end = stat
            .rfind(')')
            .ok_or_else(|| io::Error::other("process identity unavailable"))?;
        stat[end + 1..]
            .split_whitespace()
            .nth(1)
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| io::Error::other("process identity unavailable"))
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = pid;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "ancestry unavailable",
        ))
    }
}

//! OwnerValidator then SafetyValidator. Both run in full immediately before
//! every signal request, including each escalation, on fresh evidence.
use super::{
    evidence::{self, Child, Session},
    registry::Capability,
    Policy,
};
use arc_core::process_identity::IdentityStatus;
use arc_ipc::jobs::JobSnapshot;
use arc_storage::reaper::{DecisionState, Resource, LEVEL_PROVEN};

pub(super) type Rejection = (DecisionState, &'static str);

pub(super) struct Facts<'a> {
    pub job: &'a JobSnapshot,
    pub resource: Option<&'a Resource>,
    pub capability: Option<&'a Capability>,
    pub now_ms: u64,
    pub policy: &'a Policy,
}

const BLOCKED: DecisionState = DecisionState::Blocked;
const SKIPPED: DecisionState = DecisionState::Skipped;
const RETRY: DecisionState = DecisionState::RetryLater;
/// The wrapper is gone: this resource can never be acted on again.
pub(super) const SUPERVISOR_MISSING: &str = "SUPERVISOR_MISSING";

/// Proven ownership, an identity-bound capability, matching fresh identity,
/// an ended owner session, no active lease or reference, and elapsed grace.
pub(super) fn owner<'a>(facts: &Facts<'a>) -> Result<(&'a Resource, &'a Capability), Rejection> {
    let resource = facts
        .resource
        .filter(|resource| resource.ownership_level == LEVEL_PROVEN)
        .ok_or((BLOCKED, "UNKNOWN_OWNERSHIP"))?;
    // Only the live wrapper retains the child's wait; without it nothing can
    // signal safely, whether or not a capability was ever registered here.
    match evidence::live(&facts.job.observer) {
        IdentityStatus::VerifiedMatch => (),
        IdentityStatus::Indeterminate => return Err((RETRY, "IDENTITY_INDETERMINATE")),
        _ => return Err((BLOCKED, SUPERVISOR_MISSING)),
    }
    // The endpoint and token do not survive a daemon restart.
    let capability = facts
        .capability
        .ok_or((BLOCKED, "INSUFFICIENT_AUTHORITY"))?;
    let recorded = evidence::recorded(capability);
    // Fresh native evidence first: a live PID whose start time, executable,
    // parent or UID differs from the record is another process.
    match evidence::child(&recorded) {
        Child::Matches => (),
        Child::Missing => return Err((SKIPPED, "PROCESS_MISSING")),
        Child::Mismatch => return Err((BLOCKED, "IDENTITY_MISMATCH")),
        Child::Indeterminate => return Err((RETRY, "IDENTITY_INDETERMINATE")),
    }
    // The durable job row, the ownership row and the capability must agree.
    if facts.job.child.as_ref() != Some(&recorded.process)
        || facts.job.observer != capability.observer
        || resource.session.as_ref() != Some(&capability.session)
    {
        return Err((BLOCKED, "RECORD_MISMATCH"));
    }
    if !crate::connection::job_ownership::managed_child(&recorded.process, &capability.observer) {
        return Err((BLOCKED, "OWNERSHIP_UNVERIFIED"));
    }
    match evidence::session(capability) {
        Session::Active => return Err((SKIPPED, "SESSION_ACTIVE")),
        Session::Indeterminate => return Err((RETRY, "SESSION_INDETERMINATE")),
        Session::Ended => (),
    }
    let ended = resource
        .session_ended_at_ms
        .ok_or((RETRY, "SESSION_END_UNRECORDED"))?;
    if resource.active_references > 0 {
        return Err((SKIPPED, "ACTIVE_REFERENCE"));
    }
    if facts.now_ms.saturating_sub(ended) < facts.policy.session_grace_ms {
        return Err((RETRY, "SESSION_GRACE_PENDING"));
    }
    Ok((resource, capability))
}

/// Not protected and not attached to a terminal foreground.
pub(super) fn safety(resource: &Resource, capability: &Capability) -> Result<(), Rejection> {
    if resource.protected {
        return Err((BLOCKED, "PROTECTED"));
    }
    match evidence::foreground(&capability.child.process) {
        Ok(false) => Ok(()),
        Ok(true) => Err((BLOCKED, "FOREGROUND")),
        Err(_) => Err((RETRY, "FOREGROUND_INDETERMINATE")),
    }
}

pub(super) fn validate(facts: &Facts<'_>) -> Result<(), Rejection> {
    let (resource, capability) = owner(facts)?;
    safety(resource, capability)
}

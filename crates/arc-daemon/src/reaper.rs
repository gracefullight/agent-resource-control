//! Bounded process Reaper for managed-exec children (SRS 12.5, 12.6, 17.2).
//!
//! `OwnerValidator -> SafetyValidator -> ReaperExecutor` on a low-priority
//! automatic pass. It runs only when explicitly configured; the full Sweeper
//! stays off. This daemon never signals a job child: it asks the supervising
//! wrapper, which revalidates identity under its own wait gate. Every decision
//! is recorded with a reason and observation time.
use crate::db_worker::{reaper_store, DbClient};
use arc_ipc::{jobs::JobId, reaper::Signal};
use arc_storage::reaper::{
    Candidate, Decision, DecisionState, Resource, Tracking, STATE_TERMINATING,
};
use std::{
    sync::{Arc, OnceLock},
    time::{Duration, Instant},
};

#[path = "reaper/evidence.rs"]
mod evidence;
#[path = "reaper/executor.rs"]
mod executor;
#[path = "reaper/fixture.rs"]
pub(crate) mod fixture;
#[path = "reaper/registry.rs"]
mod registry;
#[path = "reaper/validate.rs"]
mod validate;

use registry::{Capability, Registry};

const SWITCH: &str = "ARC_REAPER";
const INVALID: &str = "invalid ARC_REAPER: expected `process` or unset";
/// One writer admission or one wrapper round trip.
const STEP_BUDGET: Duration = Duration::from_secs(2);

/// Frozen at startup. Scope is managed-exec children of an ended owner session.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Policy {
    /// SRS 12.2: an ended session's orphan becomes eligible after ten minutes.
    pub session_grace_ms: u64,
    /// Time allowed for SIGTERM before re-evaluating for escalation.
    pub term_grace_ms: u64,
    /// Time allowed for a confirmed outcome after SIGKILL.
    pub kill_grace_ms: u64,
    pub allow_kill: bool,
    pub interval: Duration,
    /// Candidates of each kind considered in one pass.
    pub batch: usize,
}

impl Policy {
    const PROCESS: Policy = Policy {
        session_grace_ms: 600_000,
        term_grace_ms: 5_000,
        kill_grace_ms: 5_000,
        allow_kill: true,
        interval: Duration::from_secs(30),
        batch: 64,
    };

    /// `None` means off: no pass runs and no registration is accepted.
    pub(crate) fn current() -> Result<Option<&'static Policy>, &'static str> {
        static POLICY: OnceLock<Result<Option<Policy>, &'static str>> = OnceLock::new();
        POLICY
            .get_or_init(|| parse(std::env::var_os(SWITCH).as_deref()))
            .as_ref()
            .map(Option::as_ref)
            .map_err(|error| *error)
    }
}

fn parse(value: Option<&std::ffi::OsStr>) -> Result<Option<Policy>, &'static str> {
    match value.map(|value| value.to_str()) {
        None => Ok(None),
        Some(Some("process")) => Ok(Some(Policy::PROCESS)),
        Some(_) => Err(INVALID),
    }
}

pub(crate) struct Reaper {
    registry: Arc<Registry>,
    client: DbClient,
    task: tokio::task::JoinHandle<()>,
}

/// Start the automatic pass when the bounded Reaper is configured.
pub(crate) fn start(client: &DbClient) -> Option<Reaper> {
    let policy = Policy::current().ok().flatten()?;
    let registry = Arc::new(Registry::default());
    let task = tokio::spawn(run(client.clone(), Arc::clone(&registry), policy));
    Some(Reaper {
        registry,
        client: client.clone(),
        task,
    })
}

impl Reaper {
    pub(crate) fn handler(&self) -> crate::connection::reaper_dispatch::Handler {
        let (registry, client) = (Arc::clone(&self.registry), self.client.clone());
        Arc::new(move |request, deadline| {
            let (registry, client) = (Arc::clone(&registry), client.clone());
            Box::pin(async move { registry::register(&registry, &client, request, deadline).await })
        })
    }

    /// Stop before the writer: no ticket of this task outlives the worker.
    pub(crate) async fn shutdown(self) {
        self.task.abort();
        let _ = self.task.await;
    }
}

async fn run(client: DbClient, registry: Arc<Registry>, policy: &'static Policy) {
    let mut last = None;
    loop {
        let sequence = match fixture::seams() {
            // A fixture daemon runs exactly the passes its test requests.
            Some(seams) => loop {
                match (seams.requested)() {
                    Some(sequence) if Some(sequence) != last => break Some(sequence),
                    _ => tokio::time::sleep(Duration::from_millis(5)).await,
                }
            },
            None => {
                tokio::time::sleep(policy.interval).await;
                None
            }
        };
        pass(&client, &registry, policy).await;
        if let (Some(seams), Some(sequence)) = (fixture::seams(), sequence) {
            last = Some(sequence);
            (seams.completed)(sequence);
        }
    }
}

struct Pass<'a> {
    client: &'a DbClient,
    registry: &'a Registry,
    policy: &'a Policy,
    now_ms: u64,
}

async fn pass(client: &DbClient, registry: &Registry, policy: &Policy) {
    let pass = Pass {
        client,
        registry,
        policy,
        now_ms: fixture::now_ms(),
    };
    let deadline = Instant::now() + STEP_BUDGET;
    // A busy writer postpones the whole pass; nothing is decided on old data.
    let Ok(candidates) = reaper_store::inventory(client, pass.now_ms, policy.batch, deadline).await
    else {
        return;
    };
    for candidate in &candidates {
        pass.consider(candidate).await;
        // Low priority: yield to request handlers between candidates.
        tokio::task::yield_now().await;
    }
}

fn summary(candidate: &Candidate, capability: Option<&Capability>) -> serde_json::Value {
    let level = candidate
        .resource
        .as_ref()
        .map_or(arc_storage::reaper::LEVEL_UNKNOWN, |resource| {
            resource.ownership_level.as_str()
        });
    // Identity facts only: never an executable path, argument or environment.
    serde_json::json!({
        "level": level,
        "kind": "MANAGED_BIRTH_RECORD",
        "child": candidate.job.child,
        "observer_pid": candidate.job.observer.pid,
        "executable_identity": capability
            .map(|c| format!("{}:{}", c.child.executable_device, c.child.executable_inode)),
        "uid": capability.map(|c| c.child.effective_uid),
        "session": capability.map(|c| &c.session),
        "session_ended_at_ms": candidate.resource.as_ref().and_then(|r| r.session_ended_at_ms),
    })
}

impl Pass<'_> {
    async fn record(
        &self,
        candidate: &Candidate,
        capability: Option<&Capability>,
        state: DecisionState,
        reason: &'static str,
        tracking: Tracking,
    ) -> bool {
        let decision = Decision {
            job_id: candidate.job.job_id.clone(),
            state,
            reason,
            observed_at_ms: self.now_ms,
            evidence: summary(candidate, capability),
            tracking,
        };
        reaper_store::decide(self.client, decision, Instant::now() + STEP_BUDGET)
            .await
            .is_ok()
    }

    async fn consider(&self, candidate: &Candidate) {
        let job_id = &candidate.job.job_id;
        let Some(resource) = &candidate.resource else {
            // SAFE-001: no ownership record, so never a candidate for a signal.
            let unknown = (DecisionState::Blocked, "UNKNOWN_OWNERSHIP");
            self.record(candidate, None, unknown.0, unknown.1, Tracking::Keep)
                .await;
            return;
        };
        let capability = self
            .registry
            .get(job_id)
            .filter(|_| fixture::fault("authority").is_none());
        let known = capability.as_ref();
        let terminating = resource.state == STATE_TERMINATING;
        let partial = DecisionState::PartialFailure;
        if candidate.job.outcome.is_some() {
            // The outcome is written only by the wrapper's consuming wait. It
            // confirms a reclaim only together with a signal the wrapper
            // accepted; a claim whose delivery this daemon never observed, or
            // whose capability did not survive, is recorded as unattributed.
            self.registry.remove(job_id);
            let (state, reason) = match known {
                _ if !terminating => (DecisionState::Skipped, "JOB_COMPLETED"),
                Some(capability) if capability.accepted => {
                    (DecisionState::Reclaimed, "TERMINATION_CONFIRMED")
                }
                _ => (partial, "TERMINATION_UNATTRIBUTED"),
            };
            self.record(candidate, known, state, reason, Tracking::Release)
                .await;
            return;
        }
        let unconfirmed = "TERMINATION_UNCONFIRMED";
        let signal = match known.map(|c| (c.term_sent_at_ms, c.kill_sent_at_ms)) {
            // The capability did not survive: termination cannot be confirmed.
            None if terminating => {
                self.record(candidate, None, partial, unconfirmed, Tracking::Keep)
                    .await;
                return;
            }
            Some((Some(_), Some(kill))) => {
                if self.now_ms.saturating_sub(kill) >= self.policy.kill_grace_ms {
                    self.registry.remove(job_id);
                    self.record(candidate, known, partial, unconfirmed, Tracking::Keep)
                        .await;
                }
                return;
            }
            Some((Some(term), None)) => {
                if self.now_ms.saturating_sub(term) < self.policy.term_grace_ms {
                    return;
                }
                Signal::Kill
            }
            _ => Signal::Term,
        };
        let resource = self.observe_session(job_id, resource, known).await;
        let facts = validate::Facts {
            job: &candidate.job,
            resource: Some(&resource),
            capability: known,
            now_ms: self.now_ms,
            policy: self.policy,
        };
        // Every condition is evaluated again before each request, including
        // the escalation that follows an unanswered SIGTERM.
        if let Err((state, reason)) = validate::validate(&facts) {
            // A vanished wrapper ends tracking; a claimed termination that it
            // never confirmed is recorded as unconfirmed, never as reclaimed.
            let gone = reason == validate::SUPERVISOR_MISSING;
            if gone {
                self.registry.remove(job_id);
            }
            if gone && terminating {
                self.record(candidate, known, partial, unconfirmed, Tracking::Keep)
                    .await;
            } else {
                let tracking = if gone {
                    Tracking::Release
                } else {
                    Tracking::Keep
                };
                self.record(candidate, known, state, reason, tracking).await;
            }
            return;
        }
        let Some(capability) = known else {
            return;
        };
        if signal == Signal::Kill && !self.policy.allow_kill {
            self.registry.remove(job_id);
            let reason = "ESCALATION_NOT_PERMITTED";
            self.record(candidate, known, partial, reason, Tracking::Keep)
                .await;
            return;
        }
        let claim = match signal {
            Signal::Term => "SIGTERM_REQUESTED",
            Signal::Kill => "SIGKILL_REQUESTED",
        };
        let (blocked, claimed) = (DecisionState::Blocked, DecisionState::Terminating);
        // The fenced claim commits before any request. Without it, no signal.
        if !self
            .record(candidate, known, claimed, claim, Tracking::Keep)
            .await
        {
            self.record(candidate, known, blocked, "CLAIM_REJECTED", Tracking::Keep)
                .await;
            return;
        }
        // A claim that sent nothing is withdrawn, unless an earlier signal of
        // this job is still outstanding.
        let unsent = if capability.term_sent_at_ms.is_none() {
            Tracking::Withdraw
        } else {
            Tracking::Keep
        };
        let child = evidence::requested(capability);
        let deadline = Instant::now() + STEP_BUDGET;
        let (state, reason, tracking) =
            match executor::request(job_id, capability, child, signal, deadline).await {
                executor::Reply::Signaled => {
                    self.registry.sent(job_id, signal, self.now_ms, true);
                    return;
                }
                executor::Reply::Refused(reason) => (blocked, reason.code(), unsent),
                executor::Reply::Unreachable(reason) => (blocked, reason, unsent),
                executor::Reply::Unconfirmed => {
                    // Delivery is assumed for timing only. The wrapper's
                    // acceptance was not observed, so no later outcome of this
                    // request alone is recorded as a reclaim.
                    self.registry.sent(job_id, signal, self.now_ms, false);
                    let retry = DecisionState::RetryLater;
                    (retry, "ACTION_UNCONFIRMED", Tracking::Keep)
                }
            };
        self.record(candidate, known, state, reason, tracking).await;
    }

    /// Persist the first observation of an ended owner session, releasing its
    /// lease. Grace is measured from that recorded time on the Reaper clock.
    async fn observe_session(
        &self,
        job_id: &JobId,
        resource: &Resource,
        capability: Option<&Capability>,
    ) -> Resource {
        let mut resource = resource.clone();
        let ended = resource.session_ended_at_ms.is_none()
            && capability.is_some_and(|c| evidence::session(c) == evidence::Session::Ended);
        if ended {
            let deadline = Instant::now() + STEP_BUDGET;
            if let Ok(at) =
                reaper_store::end_session(self.client, job_id, self.now_ms, deadline).await
            {
                resource.session_ended_at_ms = Some(at);
                resource.active_references = resource.active_references.saturating_sub(1);
            }
        }
        resource
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bounded_reaper_is_off_unless_explicitly_selected() {
        assert_eq!(parse(None), Ok(None));
        assert_eq!(
            parse(Some(std::ffi::OsStr::new("process"))),
            Ok(Some(Policy::PROCESS))
        );
        for value in ["", "on", "1", "true", "PROCESS", "process ", "sweeper"] {
            assert_eq!(parse(Some(std::ffi::OsStr::new(value))), Err(INVALID));
        }
        assert_eq!(Policy::PROCESS.session_grace_ms, 600_000);
        assert_eq!(
            (Policy::PROCESS.term_grace_ms, Policy::PROCESS.allow_kill),
            (5_000, true)
        );
    }
}

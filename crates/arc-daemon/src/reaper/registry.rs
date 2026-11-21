//! Verified registrations. The endpoint and token live only in this daemon's
//! memory: after a restart no capability exists, so nothing can be signalled.
use super::evidence;
use crate::db_worker::{reaper_store, DbClient};
use arc_core::process_identity::{probe_live, IdentityStatus, ProcessIdentity};
use arc_ipc::{
    jobs::JobId,
    reaper::{ChildEvidence, RegisterRequest, Signal},
};
use arc_platform::reaper_identity;
use arc_storage::{jobs::JobError, reaper::Registration};
use std::{collections::HashMap, sync::Mutex, time::Instant};

const MAX_CAPABILITIES: usize = 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Capability {
    pub observer: ProcessIdentity,
    pub child: ChildEvidence,
    pub session: ProcessIdentity,
    pub endpoint: String,
    pub token: String,
    /// When each signal request was sent, on the Reaper clock. A request
    /// whose reply was never read is timed here as if it had been delivered.
    pub term_sent_at_ms: Option<u64>,
    pub kill_sent_at_ms: Option<u64>,
    /// The wrapper itself reported that it delivered at least one signal.
    pub accepted: bool,
}

#[derive(Default)]
pub(crate) struct Registry(Mutex<HashMap<String, Capability>>);

impl Registry {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Capability>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
    pub(super) fn get(&self, job_id: &JobId) -> Option<Capability> {
        self.lock().get(job_id.as_str()).cloned()
    }
    pub(super) fn remove(&self, job_id: &JobId) {
        self.lock().remove(job_id.as_str());
    }
    pub(super) fn sent(&self, job_id: &JobId, signal: Signal, now_ms: u64, accepted: bool) {
        if let Some(capability) = self.lock().get_mut(job_id.as_str()) {
            match signal {
                Signal::Term => capability.term_sent_at_ms = Some(now_ms),
                Signal::Kill => capability.kill_sent_at_ms = Some(now_ms),
            }
            capability.accepted |= accepted;
        }
    }
    fn insert(&self, job_id: &JobId, capability: Capability) -> bool {
        let mut entries = self.lock();
        match entries.get(job_id.as_str()) {
            // A retried registration is idempotent; a second claimant is not.
            Some(existing) => {
                existing.token == capability.token && existing.child == capability.child
            }
            None if entries.len() >= MAX_CAPABILITIES => false,
            None => {
                entries.insert(job_id.as_str().to_owned(), capability);
                true
            }
        }
    }
}

/// The wrapper's private endpoint must be served by the live observer itself.
pub(super) async fn endpoint_peer(
    endpoint: &str,
    observer: &ProcessIdentity,
    deadline: Instant,
) -> Result<tokio::net::UnixStream, &'static str> {
    if !arc_ipc::reaper::valid_endpoint(endpoint) {
        return Err("ENDPOINT_UNAVAILABLE");
    }
    let Ok(Ok(stream)) = tokio::time::timeout_at(
        tokio::time::Instant::from_std(deadline),
        tokio::net::UnixStream::connect(endpoint),
    )
    .await
    else {
        return Err("ENDPOINT_UNAVAILABLE");
    };
    if crate::platform::ensure_same_user(&stream).is_err()
        || crate::platform::peer_pid(&stream).ok() != Some(observer.pid)
        || probe_live(observer) != IdentityStatus::VerifiedMatch
    {
        return Err("ENDPOINT_UNAUTHENTICATED");
    }
    Ok(stream)
}

/// Accept a registration only when this daemon's own fresh native evidence
/// agrees with every claim. The dispatcher has already required that the
/// direct peer is the named observer.
pub(super) async fn register(
    registry: &Registry,
    client: &DbClient,
    request: RegisterRequest,
    deadline: Instant,
) -> Result<(), &'static str> {
    const INVALID: &str = arc_ipc::ERROR_INVALID_REQUEST;
    let child = &request.child.process;
    let verified = || {
        probe_live(&request.observer) == IdentityStatus::VerifiedMatch
            && crate::connection::job_ownership::managed_child(child, &request.observer)
            && matches!(evidence::child(&request.child), evidence::Child::Matches)
            // The session is the observer's live direct parent, never launchd.
            && request.session.pid > 1
            && probe_live(&request.session) == IdentityStatus::VerifiedMatch
            && reaper_identity::capture(&request.observer)
                .is_ok_and(|observer| observer.parent_pid == request.session.pid)
    };
    if !verified() {
        return Err(INVALID);
    }
    drop(
        endpoint_peer(&request.endpoint, &request.observer, deadline)
            .await
            .map_err(|_| INVALID)?,
    );
    // Bracket the callback check: every fact must still hold afterwards.
    if !verified() {
        return Err(INVALID);
    }
    let registration = Registration {
        job_id: request.job_id.clone(),
        observer: request.observer.clone(),
        child: request.child.clone(),
        session: request.session.clone(),
    };
    reaper_store::register(client, registration, super::fixture::now_ms(), deadline)
        .await
        .map_err(|error| match error {
            JobError::Busy | JobError::StorageFailure => arc_ipc::ERROR_DB_BUSY,
            _ => INVALID,
        })?;
    let capability = Capability {
        observer: request.observer,
        child: request.child,
        session: request.session,
        endpoint: request.endpoint,
        token: request.token,
        term_sent_at_ms: None,
        kill_sent_at_ms: None,
        accepted: false,
    };
    registry
        .insert(&request.job_id, capability)
        .then_some(())
        .ok_or(INVALID)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(pid: u32) -> ProcessIdentity {
        ProcessIdentity {
            pid,
            start_token: "7".into(),
            boot_id: "00000000-0000-0000-0000-000000000000".into(),
        }
    }
    fn capability(token: &str) -> Capability {
        Capability {
            observer: identity(20),
            child: ChildEvidence {
                process: identity(30),
                parent_pid: 20,
                executable: "/bin/sleep".into(),
                executable_device: 1,
                executable_inode: 2,
                real_uid: 501,
                effective_uid: 501,
            },
            session: identity(10),
            endpoint: "/tmp/arc-r-0/s".into(),
            token: token.into(),
            term_sent_at_ms: None,
            kill_sent_at_ms: None,
            accepted: false,
        }
    }
    fn job(n: usize) -> JobId {
        JobId::try_from(format!("{n:032x}")).unwrap()
    }

    #[test]
    fn a_second_claimant_is_rejected_and_only_a_reported_delivery_is_accepted() {
        let registry = Registry::default();
        assert!(registry.insert(&job(1), capability("a")));
        // A retried registration is idempotent; another token or child is not.
        assert!(registry.insert(&job(1), capability("a")));
        assert!(!registry.insert(&job(1), capability("b")));
        let mut other = capability("a");
        other.child.executable_inode = 3;
        assert!(!registry.insert(&job(1), other));

        // An unread reply times the request and never counts as accepted.
        registry.sent(&job(1), Signal::Term, 100, false);
        let unconfirmed = registry.get(&job(1)).unwrap();
        assert_eq!(
            (unconfirmed.term_sent_at_ms, unconfirmed.accepted),
            (Some(100), false)
        );
        registry.sent(&job(1), Signal::Kill, 200, true);
        // A later unread reply does not take a reported delivery back.
        registry.sent(&job(1), Signal::Kill, 300, false);
        let accepted = registry.get(&job(1)).unwrap();
        assert_eq!(
            (accepted.term_sent_at_ms, accepted.kill_sent_at_ms),
            (Some(100), Some(300))
        );
        assert!(accepted.accepted);
        // A signal for a job without a capability records nothing.
        registry.sent(&job(2), Signal::Term, 100, true);
        assert_eq!(registry.get(&job(2)), None);
        registry.remove(&job(1));
        assert_eq!(registry.get(&job(1)), None);

        for n in 0..MAX_CAPABILITIES {
            assert!(registry.insert(&job(n + 10), capability("a")));
        }
        assert!(!registry.insert(&job(5), capability("a")));
    }

    #[test]
    fn a_stale_record_keeps_the_pid_and_changes_only_the_start_time() {
        let recorded = capability("a").child;
        let stale = evidence::stale(&recorded);
        assert_eq!(stale.process.pid, recorded.process.pid);
        assert_eq!(stale.process.start_token, "6");
        assert!(stale.process.is_valid());
        let mut expected = recorded.clone();
        expected.process.start_token = "6".into();
        assert_eq!(stale, expected);
        // The smallest valid start time still yields a different valid one.
        let mut first = recorded;
        first.process.start_token = "1".into();
        assert_ne!(evidence::stale(&first), first);
        assert!(evidence::stale(&first).process.is_valid());
    }
}

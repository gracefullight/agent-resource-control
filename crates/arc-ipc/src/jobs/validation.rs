use super::{
    JobSnapshot, JobSummary, LifecycleReason as R, ObservationStatus as S, ObservationTarget as T,
    Outcome, Phase as P, MAX_TIMESTAMP,
};
impl Outcome {
    pub fn is_valid(&self) -> bool {
        match self {
            Self::Signaled { signal, .. } => (1..=native_signal_max()).contains(signal),
            _ => true,
        }
    }
}
impl JobSummary {
    pub fn is_valid(&self) -> bool {
        self.registered_at_ms <= self.updated_at_ms && self.updated_at_ms <= MAX_TIMESTAMP
    }
}
impl JobSnapshot {
    pub fn is_valid(&self) -> bool {
        let times = [
            Some(self.observer_verified_at_ms),
            self.child_verified_at_ms,
            Some(self.observation.observed_at_ms),
            self.launch_claimed_at_ms,
            self.completed_at_ms,
            Some(self.updated_at_ms),
        ];
        if !self.observer.is_valid()
            || !self.classification.is_consistent()
            || self.child.as_ref().is_some_and(|c| !c.is_valid())
            || self.child.is_none() && self.child_verified_at_ms.is_some()
            || self.revision == 0
            || self.revision > MAX_TIMESTAMP
            || self.registered_at_ms > MAX_TIMESTAMP
            || times
                .iter()
                .flatten()
                .any(|t| *t < self.registered_at_ms || *t > MAX_TIMESTAMP)
            || times.iter().flatten().any(|t| *t > self.updated_at_ms)
            || self.launch_claimed_at_ms.is_some_and(|claim| {
                self.child_verified_at_ms.is_some_and(|t| t < claim)
                    || self.completed_at_ms.is_some_and(|t| t < claim)
            })
            || self.reason != self.observation.reason
            || self.observation.target == T::Child && self.child.is_none()
            || self.observation.target == T::None && self.observation.status != S::NotAttached
            || self.observation.status == S::NotAttached && self.child.is_some()
            || self.outcome.as_ref().is_some_and(|o| !o.is_valid())
        {
            return false;
        }
        let terminal = matches!(self.phase, P::Completed | P::Signaled | P::SpawnFailed);
        if terminal != self.outcome.is_some() || terminal != self.completed_at_ms.is_some() {
            return false;
        }
        if self.launch_claimed_at_ms.is_none()
            && (self.phase != P::Registered || self.child.is_some())
        {
            return false;
        }
        if self.phase == P::Registered && self.launch_claimed_at_ms.is_some()
            || self.phase == P::Launching && self.child.is_some()
            || !terminal
                && matches!(
                    self.reason,
                    R::ObservedExit | R::ObservedSignal | R::ObservedSpawnFailure
                )
        {
            return false;
        }
        match (&self.phase, &self.outcome) {
            (P::Completed, Some(Outcome::Exited { .. })) => self.reason == R::ObservedExit,
            (P::Signaled, Some(Outcome::Signaled { .. })) => self.reason == R::ObservedSignal,
            (P::SpawnFailed, Some(Outcome::SpawnFailed { .. })) => {
                self.child.is_none() && self.reason == R::ObservedSpawnFailure
            }
            (P::Completed | P::Signaled | P::SpawnFailed, _) => false,
            (P::Running, None) => {
                self.child.is_some()
                    && self.child_verified_at_ms.is_some()
                    && self.observation.target == T::Child
                    && self.observation.status == S::VerifiedMatch
                    && self.reason == R::ChildVerified
            }
            (_, None) => true,
            _ => false,
        }
    }
}

// Platform signal ranges are narrower than the protocol envelope's u8 bound.
fn native_signal_max() -> u8 {
    if cfg!(target_os = "macos") {
        31
    } else if cfg!(target_os = "linux") {
        64
    } else {
        127
    }
}

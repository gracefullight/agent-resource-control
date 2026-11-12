//! Optional display reads never authorize a launch or alter admission.
use super::transport::{self, Failure};
use arc_ipc::{
    jobs::JobId,
    policy::{self, Capabilities, JobPolicyResult},
};
use serde_json::{json, Value};
use std::{
    path::Path,
    time::{Duration, Instant},
};
#[cfg(test)]
#[path = "../../../arc-ipc/tests/fixtures/memory_pressure.rs"]
pub(crate) mod fixture;

pub(crate) struct JobRead {
    pub value: JobPolicyResult,
    pub elapsed: Duration,
}
impl JobRead {
    pub fn age_ms(&self) -> Option<u64> {
        self.value
            .effective
            .observation
            .age_ms?
            .checked_add(u64::try_from(self.elapsed.as_nanos().div_ceil(1_000_000)).ok()?)
    }
    pub fn stale(&self) -> bool {
        use policy::ObservationAvailability::Available;
        let observation = &self.value.effective.observation;
        observation.availability == Available
            && self
                .age_ms()
                .zip(observation.stale_after_ms)
                .is_none_or(|(age, limit)| age > limit)
    }
}

pub(crate) fn job(
    socket: &Path,
    id: &JobId,
    deadline: Instant,
    cancelled: &dyn Fn() -> bool,
) -> Result<JobRead, Failure> {
    let started = Instant::now();
    let value = transport::call_until_no_busy_retry(
        socket,
        policy::OPERATION_POLICY_CAPABILITIES,
        Value::Null,
        deadline,
        cancelled,
    )?;
    if !value.is_object()
        || serde_json::to_vec(&value)
            .map_or(true, |bytes| bytes.len() > policy::MAX_POLICY_QUERY_BYTES)
    {
        return Err(Failure::InvalidReply);
    }
    let caps: Capabilities = serde_json::from_value(value).map_err(|_| Failure::InvalidReply)?;
    if !caps.is_valid() {
        return Err(Failure::InvalidReply);
    }
    let value = transport::call_until_no_busy_retry(
        socket,
        policy::OPERATION_POLICY_JOB,
        json!({ "job_id": id }),
        deadline,
        cancelled,
    )?;
    let value = policy::decode_job(&value, id).map_err(|_| Failure::InvalidReply)?;
    Ok(JobRead {
        value,
        elapsed: started.elapsed(),
    })
}
pub(crate) fn confirmed_hold(result: &JobRead) -> bool {
    !result.stale()
        && result.value.effective.mode == policy::PolicyMode::MemoryHold
        && result.value.last_decision.as_ref().is_some_and(|d| {
            d.kind == policy::DecisionKind::Wait
                && d.reason == policy::DecisionReason::MemoryPressure
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use arc_ipc::policy::*;
    fn critical(age: u64, elapsed: u64) -> JobRead {
        let mut effective = fixture::fallback();
        effective.mode = PolicyMode::MemoryHold;
        effective.reason = PolicyReason::MemoryCritical;
        effective.observation = ObservationSummary {
            availability: ObservationAvailability::Available,
            pressure: Some(arc_core::meter::MemoryPressure::Critical),
            age_ms: Some(age),
            stale_after_ms: Some(3000),
            sampler: Some(SamplerIdentity {
                daemon_instance: arc_ipc::jobs::ProcessIdentity::capture(std::process::id())
                    .unwrap(),
                generation: 1,
                monotonic_epoch: 1,
            }),
            sequence: Some(1),
            observed_at_ms: Some(1),
            observed_monotonic_ms: Some(1),
        };
        let decision = RecordedDecision {
            policy_version: POLICY_VERSION.into(),
            kind: DecisionKind::Wait,
            reason: DecisionReason::MemoryPressure,
            decided_at_ms: 1,
            permit_generation: None,
            observation: effective.observation.clone(),
        };
        JobRead {
            value: JobPolicyResult {
                schema_version: 1,
                job_id: JobId::try_from("a".repeat(32)).unwrap(),
                effective,
                decision_availability: DecisionAvailability::Available,
                last_decision: Some(decision),
            },
            elapsed: Duration::from_millis(elapsed),
        }
    }
    #[test]
    fn status_and_short_notice_account_for_exact_elapsed_read_boundary_without_mutating_history() {
        for (age, elapsed) in [(2000, 1000), (2850, 150)] {
            let mut read = critical(age, elapsed);
            assert!(confirmed_hold(&read));
            assert_eq!(read.age_ms(), Some(3000));
            read.elapsed += Duration::from_millis(1);
            assert!(read.stale());
            assert!(!confirmed_hold(&read));
            assert_eq!(
                read.value.last_decision.unwrap().observation.age_ms,
                Some(age)
            );
        }
        let mut read = critical(3000, 0);
        read.elapsed = Duration::from_nanos(1);
        assert!(
            read.stale(),
            "a fractional read cannot rejuvenate an exact-boundary observation"
        );
    }
}

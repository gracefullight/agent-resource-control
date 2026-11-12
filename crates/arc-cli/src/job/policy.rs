//! Current policy and the last committed decision are deliberately separate.
use crate::exec::{policy, transport::Failure};
use arc_ipc::{jobs::JobId, policy::PolicyMode};
use std::time::{Duration, Instant};

pub(super) fn show(id: &JobId, can_wait: bool) {
    let result = arc_core::socket_path()
        .map_err(|_| Failure::Local)
        .and_then(|socket| {
            policy::job(
                &socket,
                id,
                Instant::now() + Duration::from_secs(2),
                &|| false,
            )
        });
    match result {
        Ok(result) => print!("{}", render(&result, can_wait)),
        Err(Failure::Unsupported) => println!("Memory admission: unsupported by this daemon"),
        Err(_) => {
            println!("Memory admission: unavailable; current policy and job decision are unknown")
        }
    }
}
pub(super) fn render(result: &policy::JobRead, can_wait: bool) -> String {
    let effective = &result.value.effective;
    let reason = serde_json::json!(effective.reason);
    let availability = serde_json::json!(effective.observation.availability);
    let label = if result.stale() {
        "Last received policy (stale; current admission unconfirmed)"
    } else {
        "Effective policy"
    };
    let mut out = format!("{label}: {}\nPolicy profile: {}\nPolicy mode: {}\nPolicy reason: {}\nObservation availability: {}\nObservation age (ms): {}\n",
        effective.policy_version, effective.profile, if effective.mode == PolicyMode::MemoryHold { "memory hold" } else { "Initial policy active (L0)" },
        reason.as_str().unwrap_or("unknown"), availability.as_str().unwrap_or("unknown"),
        result.age_ms().map_or_else(|| "unknown".into(), |n| n.to_string()));
    out.push_str(&format!(
        "Observation sampler: {}\nObservation sequence: {}\n",
        serde_json::json!(effective.observation.sampler),
        effective
            .observation
            .sequence
            .map_or_else(|| "unknown".into(), |n| n.to_string())
    ));
    if can_wait && policy::confirmed_hold(result) {
        out.push_str("Wait reason: Waiting for memory\nStarts automatically when memory permits; other queue rules still apply.\n");
    }
    if let Some(decision) = &result.value.last_decision {
        let reason = serde_json::json!(decision.reason);
        out.push_str(&format!("Last recorded job decision: {}\nDecision policy: {}\nDecision Unix ms: {}\nRecorded observation age (ms): {}\n",
            reason.as_str().unwrap_or("unknown"), decision.policy_version, decision.decided_at_ms,
            decision.observation.age_ms.map_or_else(|| "unknown".into(), |age| age.to_string())));
    } else {
        out.push_str(&format!(
            "Job decision availability: {}\n",
            serde_json::json!(result.value.decision_availability)
                .as_str()
                .unwrap_or("unknown")
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::exec::policy::fixture;
    #[test]
    fn stale_critical_never_claims_a_current_pressure_hold_or_changes_recorded_age() {
        use arc_ipc::policy::*;
        let mut current = fixture::fallback();
        current.reason = PolicyReason::ObservationFailed;
        current.observation.availability = ObservationAvailability::Failed;
        let result = JobPolicyResult {
            schema_version: 1,
            job_id: JobId::try_from("a".repeat(32)).unwrap(),
            effective: current,
            decision_availability: DecisionAvailability::NotRecorded,
            last_decision: None,
        };
        let text = render(
            &policy::JobRead {
                value: result,
                elapsed: Duration::ZERO,
            },
            true,
        );
        assert!(text.contains("Initial policy active (L0)"));
        assert!(text.contains("observation_failed"));
        assert!(!text.contains("Waiting for memory"));
        assert!(text.contains("not_recorded"));
    }
}

//! Shared wire contract acceptance; production routing belongs to task 2.
#[path = "fixtures/memory_pressure.rs"]
mod fixture;
use arc_ipc::{jobs::JobId, policy::*, Response};
use serde_json::json;

#[test]
fn additive_reads_match_ids_and_reject_unknown_positional_and_oversized_queries() {
    let id = JobId::try_from("a".repeat(32)).unwrap();
    let request = job_request("job", &id);
    let result = JobPolicyResult {
        schema_version: 1,
        job_id: id.clone(),
        effective: fixture::fallback(),
        decision_availability: DecisionAvailability::NotRecorded,
        last_decision: None,
    };
    let response = job_response(&request, &result);
    assert_eq!(interpret_job(&request, &response), Ok(result.clone()));
    for payload in [
        json!([id]),
        json!({"job_id":id,"history":true}),
        json!({"job_id":"a".repeat(513)}),
    ] {
        assert!(decode_job_request(&payload).is_err());
    }
    let mut mismatched = request.clone();
    mismatched.request_id = "other".into();
    assert_eq!(
        interpret_job(&mismatched, &response),
        Err(PolicyReadError::InvalidReply)
    );
    let mut wrong_job = result.clone();
    wrong_job.job_id = JobId::try_from("b".repeat(32)).unwrap();
    assert!(decode_job(&json!(wrong_job), &id).is_err());
    let mut missing_decision = result;
    missing_decision.decision_availability = DecisionAvailability::Available;
    assert!(!missing_decision.is_valid());
}
#[test]
fn old_daemon_unsupported_and_unavailable_remain_distinct_from_explicit_l0() {
    let request = current_request("current");
    let response = current_response(&request, &fixture::fallback());
    assert_eq!(
        interpret_current(&request, &response)
            .unwrap()
            .observation
            .pressure,
        None
    );
    for (code, expected) in [
        (
            arc_ipc::ERROR_UNSUPPORTED_CAPABILITY,
            PolicyReadError::Unsupported,
        ),
        (ERROR_POLICY_UNAVAILABLE, PolicyReadError::Unavailable),
        (
            arc_ipc::ERROR_PROTOCOL_INCOMPATIBLE,
            PolicyReadError::Protocol,
        ),
    ] {
        assert_eq!(
            interpret_current(&request, &Response::error("current", code)),
            Err(expected)
        );
    }
    let mut invalid = request.clone();
    invalid.payload = json!({});
    assert_eq!(
        current_response(&invalid, &fixture::fallback()).status,
        arc_ipc::STATUS_ERROR
    );
    let probe = capabilities_request("probe");
    assert_eq!(
        interpret_capabilities(&probe, &capabilities_response(&probe)),
        Ok(Capabilities::supported())
    );
    let old_traffic =
        json!({"state":"QUEUED","sequence":1,"queued_at_ms":1,"reason":"MEMORY_BUDGET"});
    let legacy: arc_ipc::traffic::JobResult = serde_json::from_value(old_traffic.clone()).unwrap();
    assert_eq!(serde_json::to_value(legacy).unwrap(), old_traffic);
}
#[test]
fn current_replies_reject_positional_children_unknown_fields_and_invalid_versions() {
    let policy = fixture::fallback();
    assert!(decode_current(&json!(policy)).is_ok());
    let mut payload = json!(policy);
    payload["observation"] = json!(["missing", null, null, null, null, null, null, null]);
    assert!(decode_current(&payload).is_err());
    payload = json!(policy);
    payload["extra"] = json!(true);
    assert!(decode_current(&payload).is_err());
    payload = json!(policy);
    payload["schema_version"] = json!(2);
    assert!(decode_current(&payload).is_err());
    payload = json!(policy);
    payload["profile"] = json!("a".repeat(MAX_POLICY_BYTES));
    assert!(decode_current(&payload).is_err());
}

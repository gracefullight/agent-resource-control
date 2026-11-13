//! Contract validation only: real routing/commit/native acceptance is downstream.
use arc_ipc::{
    governor::*,
    jobs::{JobId, ProcessIdentity},
    policy::launch,
    Response,
};
use serde_json::{json, Value};

fn setter(enabled: bool, revision: u64) -> SetGovernorSwitchRequest {
    SetGovernorSwitchRequest {
        key: GovernorSwitchKey::GovernorEnabled,
        enabled,
        expected_revision: revision,
        mutation_id: format!("{revision}-{}", "a".repeat(32)),
    }
}
fn state(enabled: bool, revision: u64) -> GovernorSwitchState {
    GovernorSwitchState {
        key: GovernorSwitchKey::GovernorEnabled,
        enabled,
        revision,
        updated_at_ms: Some(10),
    }
}
fn observer() -> ProcessIdentity {
    ProcessIdentity {
        pid: 42,
        start_token: "123".into(),
        boot_id: "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".into(),
    }
}
fn id() -> JobId {
    JobId::try_from("b".repeat(32)).unwrap()
}

#[test]
fn default_read_optional_timestamp_and_strict_owner_control_payloads() {
    assert_eq!(
        json!(GovernorSwitchState::default()),
        json!({"key":"governor.enabled","enabled":true,"revision":0})
    );
    assert!(decode_switch_state(&json!(GovernorSwitchState::default())).is_ok());
    assert!(decode_get_switch(&json!({"key":"governor.enabled"})).is_ok());
    let request = json!(setter(false, 0));
    for operation in [OPERATION_SET_SWITCH, OPERATION_PAUSE] {
        assert_eq!(decode_set_switch(operation, &request), Ok(setter(false, 0)));
    }
    assert!(decode_set_switch(OPERATION_RESUME, &request).is_err());
    assert!(decode_set_switch(OPERATION_RESUME, &json!(setter(true, 0))).is_ok());
    assert!(decode_set_switch("other", &request).is_err());
    for payload in [
        Value::Null,
        json!([]),
        json!({}),
        json!({"key":"meter.enabled"}),
        json!({"key":"governor.enabled","extra":true}),
    ] {
        assert!(decode_get_switch(&payload).is_err(), "{payload}");
    }
    for (field, value) in [
        ("extra", json!(true)),
        ("key", json!("meter.enabled")),
        ("enabled", json!(0)),
        ("expected_revision", json!(-1)),
        ("expected_revision", json!(0.5)),
        ("expected_revision", json!(MAX_SAFE_INTEGER + 1)),
        ("mutation_id", json!("a".repeat(MAX_SWITCH_BYTES))),
        ("mutation_id", json!(format!("00-{}", "a".repeat(32)))),
        ("mutation_id", json!(format!("1-{}", "a".repeat(32)))),
        ("mutation_id", json!(format!("0-{}", "A".repeat(32)))),
        ("mutation_id", json!(format!("0-{}", "g".repeat(32)))),
    ] {
        let mut bad = request.clone();
        bad[field] = value;
        assert!(
            decode_set_switch(OPERATION_SET_SWITCH, &bad).is_err(),
            "{bad}"
        );
    }
    for payload in [
        json!({"key":"governor.enabled","enabled":false,"revision":0}),
        json!(state(true, MAX_SAFE_INTEGER + 1)),
        json!({"key":"governor.enabled","enabled":true,"revision":1,"updated_at_ms":null}),
        json!({"key":"governor.enabled","enabled":true,"revision":0,"updated_at_ms":MAX_SAFE_INTEGER+1}),
    ] {
        assert!(decode_switch_state(&payload).is_err(), "{payload}");
    }
}

#[test]
fn mutation_results_bind_identity_cas_noop_replay_and_confirmed_rejections() {
    let request = setter(false, 4);
    let committed = GovernorMutationResult::Committed {
        mutation_id: request.mutation_id.clone(),
        switch: state(false, 5),
        changed: true,
        replayed: false,
    };
    assert!(decode_mutation_result(&json!(committed), &request).is_ok());
    let mut replay = json!(committed);
    replay["replayed"] = json!(true);
    assert!(decode_mutation_result(&replay, &request).is_ok());
    let noop = GovernorMutationResult::Committed {
        mutation_id: request.mutation_id.clone(),
        switch: state(false, 4),
        changed: false,
        replayed: false,
    };
    assert!(decode_mutation_result(&json!(noop), &request).is_ok());
    for (field, value) in [
        ("mutation_id", json!("other")),
        ("changed", json!(false)),
        ("switch", json!(state(true, 5))),
        ("switch", json!(state(false, 6))),
        ("extra", json!(true)),
        ("status", json!("UNCERTAIN")),
    ] {
        let mut bad = json!(committed);
        bad[field] = value;
        assert!(decode_mutation_result(&bad, &request).is_err(), "{bad}");
    }
    let conflict = GovernorMutationResult::Conflict {
        mutation_id: request.mutation_id.clone(),
        switch: state(true, 6),
        reason: GovernorConflictReason::RevisionConflict,
    };
    assert!(decode_mutation_result(&json!(conflict), &request).is_ok());
    let collision = GovernorMutationResult::Conflict {
        mutation_id: request.mutation_id.clone(),
        switch: state(true, 4),
        reason: GovernorConflictReason::MutationIdConflict,
    };
    assert!(decode_mutation_result(&json!(collision), &request).is_ok());
    let rejected = GovernorMutationResult::Rejected {
        mutation_id: request.mutation_id.clone(),
        switch: state(true, 4),
        reason: GovernorRejectionReason::ReceiptCapacity,
    };
    assert!(decode_mutation_result(&json!(rejected), &request).is_ok());
    let max = setter(false, MAX_SAFE_INTEGER);
    let exhausted = GovernorMutationResult::Rejected {
        mutation_id: max.mutation_id.clone(),
        switch: state(true, MAX_SAFE_INTEGER),
        reason: GovernorRejectionReason::RevisionExhausted,
    };
    assert!(decode_mutation_result(&json!(exhausted), &max).is_ok());
    let mut bad = json!(exhausted);
    bad["switch"]["revision"] = json!(MAX_SAFE_INTEGER - 1);
    assert!(decode_mutation_result(&bad, &max).is_err());
}

#[test]
fn receipt_readback_separates_current_state_from_historical_mutation_outcome() {
    let query = GetGovernorSwitchRequest {
        key: GovernorSwitchKey::GovernorEnabled,
        mutation_id: Some(setter(false, 0).mutation_id),
    };
    assert!(decode_get_switch(&json!(query)).is_ok());
    let committed = GovernorMutationResult::Committed {
        mutation_id: query.mutation_id.clone().unwrap(),
        switch: state(false, 1),
        changed: true,
        replayed: true,
    };
    let read = GovernorSwitchReadResult {
        switch: state(true, 2),
        mutation_result: Some(committed.clone()),
    };
    assert!(decode_switch_read_result(&json!(read), &query).is_ok());
    assert!(decode_switch_read_result(&json!(read), &GetGovernorSwitchRequest::default()).is_err());
    let unknown = GovernorSwitchReadResult {
        switch: GovernorSwitchState::default(),
        mutation_result: None,
    };
    assert!(decode_switch_read_result(&json!(unknown), &query).is_ok());
    for (field, value) in [
        ("replayed", json!(false)),
        ("status", json!("REJECTED")),
        ("mutation_id", json!("0-bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb")),
        ("switch", json!(state(false, 3))),
    ] {
        let mut bad = json!(read);
        bad["mutation_result"][field] = value;
        assert!(decode_switch_read_result(&bad, &query).is_err());
    }
    let mut inconsistent = json!(read);
    inconsistent["switch"] = json!(state(true, 1));
    assert!(decode_switch_read_result(&inconsistent, &query).is_err());
    for id in [
        "bad",
        "00-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        "9007199254740992-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
    ] {
        assert!(decode_get_switch(&json!({"key":"governor.enabled","mutation_id":id})).is_err());
    }
}

#[test]
fn capability_is_separate_and_preserves_legacy_strict_results() {
    let request = capabilities_request("probe");
    assert_eq!(
        interpret_capabilities(&request, &capabilities_response(&request)),
        Ok(GovernorCapabilities::supported())
    );
    assert_eq!(
        interpret_capabilities(
            &request,
            &Response::error("probe", arc_ipc::ERROR_UNSUPPORTED_CAPABILITY)
        ),
        Err(arc_ipc::ERROR_UNSUPPORTED_CAPABILITY)
    );
    let mut response = capabilities_response(&request);
    response.result.as_mut().unwrap()["extra"] = json!(true);
    assert!(interpret_capabilities(&request, &response).is_err());
    response = capabilities_response(&request);
    response.request_id = "other".into();
    assert!(interpret_capabilities(&request, &response).is_err());
    let mut bad = request.clone();
    bad.payload = json!({});
    assert_eq!(capabilities_response(&bad).status, arc_ipc::STATUS_ERROR);
    assert!(interpret_capabilities(&bad, &capabilities_response(&request)).is_err());
    bad = request.clone();
    bad.protocol_version = 2;
    assert_eq!(
        capabilities_response(&bad).error.unwrap().code,
        arc_ipc::ERROR_PROTOCOL_INCOMPATIBLE
    );
    assert_eq!(
        interpret_capabilities(&bad, &capabilities_response(&request)),
        Err(arc_ipc::ERROR_PROTOCOL_INCOMPATIBLE)
    );
    let old_policy = arc_ipc::policy::Capabilities::supported();
    assert_eq!(
        json!(old_policy),
        json!({"capability":"policy.memory.v1","launch_capability":"traffic.memory-launch.v1"})
    );
    let old_traffic =
        json!({"state":"QUEUED","sequence":1,"queued_at_ms":1,"reason":"MEMORY_BUDGET"});
    let legacy: arc_ipc::traffic::JobResult = serde_json::from_value(old_traffic.clone()).unwrap();
    assert_eq!(json!(legacy), old_traffic);
    assert!(serde_json::from_value::<arc_ipc::traffic::JobResult>(
        json!({"state":"UNRESERVED","job_id":id(),"switch_revision":1,"ready_at_ms":10})
    )
    .is_err());
}

#[test]
fn unreserved_grant_and_fenced_ack_never_forge_a_permit() {
    let request = GovernorLaunchRequest::Unreserved {
        job_id: id(),
        observer: observer(),
    };
    assert!(decode_launch(&json!(request)).is_ok());
    let result = GovernorLaunchResult::Unreserved {
        switch_revision: 1,
        release_id: "c".repeat(32),
        valid_for_ms: 100,
    };
    assert!(decode_launch_result(&json!(result)).is_ok());
    let ack = GovernorLaunchAck::Unreserved {
        ack: GovernorUnreservedAck::Started {
            job_id: id(),
            observer: observer(),
            release_id: "c".repeat(32),
            child: ProcessIdentity {
                pid: 43,
                ..observer()
            },
        },
    };
    assert!(decode_launch_ack(&json!(ack), &request, &result).is_ok());
    for (field, value) in [
        ("release_id", json!("d".repeat(32))),
        ("job_id", json!("d".repeat(32))),
        (
            "observer",
            json!([42, "123", "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"]),
        ),
        (
            "child",
            json!({"pid":0,"start_token":"123","boot_id":"aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"}),
        ),
    ] {
        let mut bad = json!(ack);
        bad["ack"][field] = value;
        assert!(decode_launch_ack(&bad, &request, &result).is_err());
    }
    let reserved = GovernorLaunchRequest::Reserved {
        request: launch::LaunchRequest {
            job_id: id(),
            observer: observer(),
            permit_generation: 3,
        },
    };
    assert!(
        decode_launch_ack(&json!(ack), &reserved, &result).is_ok(),
        "pause can convert a reserved request before release"
    );
    for payload in [
        json!({"mode":"UNRESERVED","job_id":id(),"observer":observer(),"permit_generation":0}),
        json!({"mode":"RESERVED","request":{"job_id":id(),"observer":observer(),"permit_generation":0}}),
    ] {
        assert!(decode_launch(&payload).is_err());
    }
    for payload in [
        json!({"state":"UNRESERVED","switch_revision":MAX_SAFE_INTEGER+1,"release_id":"c".repeat(32),"valid_for_ms":1}),
        json!({"state":"UNRESERVED","switch_revision":1,"release_id":"c".repeat(32),"valid_for_ms":0}),
        json!({"state":"UNRESERVED","switch_revision":1,"release_id":"invalid","valid_for_ms":1}),
    ] {
        assert!(decode_launch_result(&payload).is_err());
    }
    for result in [
        GovernorLaunchResult::Retry {},
        GovernorLaunchResult::Unconfirmed {},
        GovernorLaunchResult::Cancelled {},
        GovernorLaunchResult::NotFound {},
    ] {
        assert!(decode_launch_result(&json!(result)).is_ok());
        assert!(!ack.matches(&request, &result));
    }
    let unspawned = GovernorLaunchAck::Unreserved {
        ack: GovernorUnreservedAck::Unspawned {
            job_id: id(),
            observer: observer(),
            release_id: "c".repeat(32),
        },
    };
    assert!(decode_launch_ack(&json!(unspawned), &request, &result).is_ok());
}

#[test]
fn additive_job_route_wraps_strict_traffic_and_unreserved_lifecycle() {
    for request in [
        GovernorJobRequest::Traffic {
            request: arc_ipc::traffic::JobRequest::Poll {
                job_id: id(),
                observer: observer(),
            },
        },
        GovernorJobRequest::Attach {
            job_id: id(),
            observer: observer(),
            release_id: "c".repeat(32),
            child: ProcessIdentity {
                pid: 43,
                ..observer()
            },
        },
        GovernorJobRequest::Complete {
            job_id: id(),
            observer: observer(),
            release_id: "c".repeat(32),
            outcome: arc_ipc::jobs::Outcome::Exited { code: 0 },
        },
    ] {
        assert!(decode_job(&json!(request)).is_ok());
        let mut bad = json!(request);
        bad["extra"] = json!(true);
        assert!(decode_job(&bad).is_err());
    }
    for result in [
        GovernorJobResult::Unreserved {
            job_id: id(),
            switch_revision: 1,
            ready_at_ms: 10,
        },
        GovernorJobResult::Traffic {
            result: arc_ipc::traffic::JobResult::Cancelled {},
        },
        GovernorJobResult::Unconfirmed { job_id: id() },
    ] {
        assert!(decode_job_result(&json!(result)).is_ok());
    }
    assert!(decode_job_result(&json!({"state":"UNRESERVED","job_id":id(),"switch_revision":1,"ready_at_ms":MAX_SAFE_INTEGER+1})).is_err());
}

#[test]
fn governor_status_route_round_trips_legacy_status_payloads() {
    for action in ["STATUS", "STATUS_V2"] {
        let request = json!({"action":"TRAFFIC","request":{"action":action,"job_id":id()}});
        let decoded = decode_job(&request).unwrap();
        assert_eq!(json!(decoded), request);
    }
    for status in [
        json!({"state":"RESERVED","sequence":1,"queued_at_ms":10,"granted_at_ms":20,"reservation":{"cpu_millis":1000,"memory_bytes":1024}}),
        json!({"state":"RUNNING","queue_wait_ms":10}),
        json!({"state":"QUEUED","sequence":1,"queued_at_ms":10}),
        json!({"state":"COMPLETED","queue_wait_ms":10,"outcome":{"kind":"EXITED","code":0}}),
        json!({"state":"CANCELLED"}),
        json!({"state":"ABANDONED"}),
        json!({"state":"UNCONFIRMED"}),
        json!({"state":"NOT_FOUND"}),
    ] {
        let legacy: arc_ipc::traffic::StatusResult =
            serde_json::from_value(status.clone()).unwrap();
        assert!(legacy.is_valid(), "{status}");
        assert_eq!(json!(legacy), status, "legacy bytes retain their shape");
        let wrapped = json!({"state":"STATUS","result":status});
        let decoded =
            decode_job_result(&wrapped).expect("Governor must carry legacy status replies");
        assert_eq!(json!(decoded), wrapped);
        assert!(serde_json::from_value::<arc_ipc::traffic::StatusResult>(wrapped).is_err());

        let mut bad = json!({"state":"STATUS","result":status});
        bad["result"]["permit_generation"] = json!(1);
        assert!(decode_job_result(&bad).is_err(), "{bad}");
    }
    for status in [
        json!({"state":"RESERVED","sequence":1,"queued_at_ms":10,"granted_at_ms":20,"reservation":{"cpu_millis":1000,"memory_bytes":1024}}),
        json!({"state":"RUNNING","queue_wait_ms":10}),
    ] {
        assert!(decode_job_result(&json!({"state":"TRAFFIC","result":status})).is_err());
        assert!(serde_json::from_value::<arc_ipc::traffic::JobResult>(status).is_err());
    }
}

#[test]
fn governor_status_route_rejects_invalid_nested_payloads() {
    for status in [
        json!({"state":"RESERVED","sequence":0,"queued_at_ms":10,"granted_at_ms":20,"reservation":{"cpu_millis":1000,"memory_bytes":1024}}),
        json!({"state":"RESERVED","sequence":1,"queued_at_ms":20,"granted_at_ms":10,"reservation":{"cpu_millis":1000,"memory_bytes":1024}}),
        json!({"state":"RESERVED","sequence":1,"queued_at_ms":10,"granted_at_ms":20,"reservation":{"cpu_millis":0,"memory_bytes":1024}}),
        json!({"state":"RESERVED","sequence":1,"queued_at_ms":10,"granted_at_ms":20,"reservation":[1000,1024]}),
        json!({"state":"RUNNING","queue_wait_ms":u64::MAX}),
        json!({"state":"RUNNING","queue_wait_ms":-1}),
        json!({"state":"RUNNING"}),
        json!({"state":"COMPLETED","queue_wait_ms":10,"outcome":{"kind":"EXITED","code":"0"}}),
        json!({"state":"UNCONFIRMED","job_id":id()}),
        json!({"state":"UNKNOWN"}),
        json!([]),
        Value::Null,
    ] {
        assert!(decode_job_result(&json!({"state":"STATUS","result":status})).is_err());
    }
    for payload in [
        json!({"state":"STATUS"}),
        json!({"state":"STATUS","result":{"state":"RUNNING","queue_wait_ms":10},"extra":true}),
    ] {
        assert!(decode_job_result(&payload).is_err());
    }
}

#[path = "governor/release_ack.rs"]
mod release_ack;

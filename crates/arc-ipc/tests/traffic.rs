use arc_ipc::{
    dispatch,
    jobs::StartRequest,
    read_message,
    traffic::{self, JobRequest, JobResult, ProbeResult, StatusResult},
    write_message, Request, Response,
};
use serde_json::{json, Value};
use std::io::Cursor;

const ID: &str = "0123456789abcdef0123456789abcdef";
const CONTEXT: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn identity() -> Value {
    json!({"pid":1,"start_token":"1","boot_id":"01234567-89ab-cdef-0123-456789abcdef"})
}
fn classification() -> Value {
    json!({"classification":"BUILD","locality":"HOST_LOCAL","reason":"CARGO_BUILD","uncertainty":"NONE"})
}
fn grant() -> Value {
    json!({"state":"GRANTED","sequence":1,"queued_at_ms":10,"granted_at_ms":12,
        "permit_generation":1,"reservation":{"cpu_millis":2000,"memory_bytes":1024},
        "lineage_context":CONTEXT})
}

#[test]
fn request_debug_never_formats_payload_values() {
    for payload in [
        json!("private command text"),
        json!(["private command text", {"lineage_context": CONTEXT}]),
        json!({"command":"private command text", "lineage_context":CONTEXT,
            "nested":[{"lineage_context":CONTEXT}, {"other":"hidden"}]}),
    ] {
        let mut request = Request::new("classify-id", arc_ipc::OPERATION_JOB_CLASSIFY);
        request.payload = payload;
        for debug in [format!("{request:?}"), format!("{request:#?}")] {
            assert!(debug.contains("payload: <redacted>"));
            assert!(debug.contains("classify-id"));
            assert!(debug.contains(arc_ipc::OPERATION_JOB_CLASSIFY));
            assert!(!debug.contains("private command text"));
            assert!(!debug.contains(CONTEXT));
            assert!(!debug.contains("hidden"));
        }
    }
}

#[test]
fn response_debug_never_formats_result_values() {
    for result in [
        json!("private response text"),
        json!(["private response text", {"lineage_context": CONTEXT}]),
        json!({"state":"private response text", "lineage_context":CONTEXT,
            "nested":[{"lineage_context":CONTEXT}, {"other":"hidden"}]}),
    ] {
        let response = Response::ok("response-id", result);
        for debug in [format!("{response:?}"), format!("{response:#?}")] {
            assert!(debug.contains("result: <redacted>"));
            assert!(debug.contains("response-id"));
            assert!(debug.contains("ok"));
            assert!(!debug.contains("private response text"));
            assert!(!debug.contains(CONTEXT));
            assert!(!debug.contains("hidden"));
        }
    }
}

#[test]
fn traffic_envelope_wire_round_trips_keep_lineage_context() {
    let enqueue = json!({"action":"ENQUEUE","job_id":ID,"observer":identity(),
        "classification":classification(),"lineage_context":CONTEXT});
    let typed = traffic::decode_job(&enqueue).unwrap();
    let mut request = Request::new("request-id", traffic::OPERATION_TRAFFIC_JOB);
    request.payload = serde_json::to_value(typed).unwrap();
    let granted: JobResult = serde_json::from_value(grant()).unwrap();
    let response = Response::ok("request-id", serde_json::to_value(granted).unwrap());

    for debug in [format!("{request:?}"), format!("{request:#?}")] {
        assert!(debug.contains("payload: <redacted>"));
        assert!(!debug.contains(CONTEXT));
        assert!(!debug.contains("ENQUEUE"));
    }
    for debug in [format!("{response:?}"), format!("{response:#?}")] {
        assert!(debug.contains("result: <redacted>"));
        assert!(!debug.contains(CONTEXT));
        assert!(!debug.contains("GRANTED"));
    }

    let mut request_frame = Vec::new();
    write_message(&mut request_frame, &request).unwrap();
    let decoded_request: Request = read_message(&mut Cursor::new(request_frame)).unwrap();
    assert_eq!(decoded_request.payload["lineage_context"], CONTEXT);
    assert_eq!(decoded_request, request);

    let mut response_frame = Vec::new();
    write_message(&mut response_frame, &response).unwrap();
    let decoded_response: Response = read_message(&mut Cursor::new(response_frame)).unwrap();
    assert_eq!(
        decoded_response.result.as_ref().unwrap()["lineage_context"],
        CONTEXT
    );
    assert_eq!(decoded_response, response);
}

#[test]
fn capability_probe_requires_matching_explicit_response_and_protocol_one() {
    let request = Request::new("probe-id", traffic::OPERATION_TRAFFIC_CAPABILITIES);
    assert!(traffic::decode_probe(&Value::Null).is_ok());
    assert_eq!(
        traffic::decode_probe(&json!({})),
        Err(arc_ipc::ERROR_INVALID_REQUEST)
    );
    // This is the frozen old dispatcher: an unknown operation must not activate scheduling.
    let old_response = dispatch(&request, || panic!("probe reached doctor"));
    assert_eq!(
        traffic::interpret_probe(&request, &old_response),
        ProbeResult::Unsupported
    );
    let supported = Response::ok(
        "probe-id",
        serde_json::to_value(traffic::Capabilities::supported()).unwrap(),
    );
    assert_eq!(
        traffic::interpret_probe(&request, &supported),
        ProbeResult::Supported
    );

    let mut malformed = supported.clone();
    malformed.request_id = "another-id".into();
    assert_eq!(
        traffic::interpret_probe(&request, &malformed),
        ProbeResult::Invalid
    );
    malformed = supported.clone();
    malformed.protocol_version = 2;
    assert_eq!(
        traffic::interpret_probe(&request, &malformed),
        ProbeResult::Invalid
    );
    malformed = supported.clone();
    malformed.result = Some(json!({"capability":"traffic.v1", "extra":true}));
    assert_eq!(
        traffic::interpret_probe(&request, &malformed),
        ProbeResult::Invalid
    );
    malformed.result = Some(json!(["traffic.v1"]));
    assert_eq!(
        traffic::interpret_probe(&request, &malformed),
        ProbeResult::Invalid
    );
    malformed.result = Some(json!({"capability":"traffic.v2"}));
    assert_eq!(
        traffic::interpret_probe(&request, &malformed),
        ProbeResult::Invalid
    );
    malformed = supported.clone();
    malformed.error = old_response.error.clone();
    assert_eq!(
        traffic::interpret_probe(&request, &malformed),
        ProbeResult::Invalid
    );
    let mut bad_error = Response::error("probe-id", "DB_BUSY");
    assert_eq!(
        traffic::interpret_probe(&request, &bad_error),
        ProbeResult::Invalid
    );
    bad_error.error = old_response.error.clone();
    bad_error.result = Some(json!({}));
    assert_eq!(
        traffic::interpret_probe(&request, &bad_error),
        ProbeResult::Invalid
    );
    let wrong_request = Request::new("probe-id", "job.start");
    assert_eq!(
        traffic::interpret_probe(&wrong_request, &old_response),
        ProbeResult::Invalid
    );
    let mut wrong_payload = request.clone();
    wrong_payload.payload = json!({});
    assert_eq!(
        traffic::interpret_probe(&wrong_payload, &old_response),
        ProbeResult::Invalid
    );
}

#[test]
fn traffic_requests_are_bounded_typed_and_redacted() {
    let requests = [
        json!({"action":"ENQUEUE","job_id":ID,"observer":identity(),"classification":classification(),"lineage_context":CONTEXT}),
        json!({"action":"POLL","job_id":ID,"observer":identity()}),
        json!({"action":"ATTACH","job_id":ID,"observer":identity(),"child":identity(),"permit_generation":1}),
        json!({"action":"COMPLETE","job_id":ID,"observer":identity(),"outcome":{"kind":"EXITED","code":0},"permit_generation":1}),
        json!({"action":"CANCEL","job_id":ID,"observer":identity(),"permit_generation":null}),
        json!({"action":"FALLBACK","job_id":ID,"observer":identity(),"permit_generation":null,"child":null,"outcome":null}),
        json!({"action":"STATUS","job_id":ID}),
    ];
    for value in requests {
        let request = traffic::decode_job(&value).unwrap();
        assert_eq!(serde_json::to_value(&request).unwrap(), value);
        assert!(!format!("{request:?}").contains(CONTEXT));
        let mut extra = value.clone();
        extra["environment"] = json!("secret");
        assert_eq!(
            traffic::decode_job(&extra),
            Err(arc_ipc::ERROR_INVALID_REQUEST)
        );
    }
    assert_eq!(
        traffic::decode_job(&json!([])),
        Err(arc_ipc::ERROR_INVALID_REQUEST)
    );
    assert_eq!(
        traffic::decode_job(&json!({"action":"STATUS","job_id":"x".repeat(9000)})),
        Err(arc_ipc::ERROR_INVALID_REQUEST)
    );
    let base = json!({"action":"ENQUEUE","job_id":ID,"observer":identity(),"classification":classification()});
    let mut bad = base.clone();
    bad["classification"]["locality"] = json!("REMOTE");
    assert!(traffic::decode_job(&bad).is_err());
    bad = base.clone();
    bad["classification"] = json!(["BUILD", "HOST_LOCAL", "CARGO_BUILD", "NONE"]);
    assert!(traffic::decode_job(&bad).is_err());
    bad = base.clone();
    bad["observer"]["pid"] = json!(0);
    assert!(traffic::decode_job(&bad).is_err());
    for context in ["", "a", "A".repeat(64).as_str()] {
        bad = base.clone();
        bad["lineage_context"] = json!(context);
        assert!(traffic::decode_job(&bad).is_err());
    }
    assert!(matches!(
        traffic::decode_job(&base),
        Ok(JobRequest::Enqueue {
            lineage_context: None,
            ..
        })
    ));
    bad = json!({"action":"COMPLETE","job_id":ID,"observer":identity(),"permit_generation":1,"outcome":{"kind":"SIGNALED","signal":0,"core_dumped":false}});
    assert!(traffic::decode_job(&bad).is_err());
    bad = json!({"action":"FALLBACK","job_id":ID,"observer":identity(),"child":[1,"1","01234567-89ab-cdef-0123-456789abcdef"]});
    assert!(traffic::decode_job(&bad).is_err());
    bad = json!({"action":"ATTACH","job_id":ID,"observer":identity(),"child":identity(),"permit_generation":0});
    assert!(traffic::decode_job(&bad).is_err());
    bad = json!({"action":"FALLBACK","job_id":ID,"observer":identity(),"child":identity(),"outcome":{"kind":"SPAWN_FAILED","reason":"OTHER","os_code":null}});
    assert!(traffic::decode_job(&bad).is_err());
}

#[test]
fn replies_validate_sequences_times_resources_and_outcomes() {
    let value = grant();
    let result: JobResult = serde_json::from_value(value.clone()).unwrap();
    assert!(result.is_valid());
    assert!(!format!("{result:?}").contains(CONTEXT));
    assert_eq!(serde_json::to_value(result).unwrap(), value);
    for (field, invalid) in [
        ("sequence", json!(0)),
        ("queued_at_ms", json!(13)),
        ("granted_at_ms", json!(9_007_199_254_740_992u64)),
        ("permit_generation", json!(0)),
    ] {
        let mut bad = value.clone();
        bad[field] = invalid;
        assert!(!serde_json::from_value::<JobResult>(bad).unwrap().is_valid());
    }
    let mut bad = value.clone();
    bad["reservation"]["cpu_millis"] = json!(0);
    assert!(!serde_json::from_value::<JobResult>(bad).unwrap().is_valid());
    bad = value.clone();
    bad["reservation"] = json!([2000, 1024]);
    assert!(serde_json::from_value::<JobResult>(bad).is_err());
    bad = value.clone();
    bad["unexpected"] = json!(true);
    assert!(serde_json::from_value::<JobResult>(bad).is_err());
    for value in [
        json!({"state":"BYPASSED","reason":"REMOTE"}),
        json!({"state":"QUEUED","sequence":1,"queued_at_ms":10,"reason":"FIFO"}),
        json!({"state":"REUSED","parent_job_id":ID,"permit_generation":1}),
        json!({"state":"RUNNING"}),
        json!({"state":"COMPLETED","queue_wait_ms":2,"outcome":{"kind":"EXITED","code":0}}),
        json!({"state":"CANCELLED"}),
        json!({"state":"ABANDONED"}),
        json!({"state":"RESERVATION_RETAINED"}),
        json!({"state":"NOT_FOUND"}),
    ] {
        let result: JobResult = serde_json::from_value(value.clone()).unwrap();
        assert!(result.is_valid());
        assert_eq!(serde_json::to_value(result).unwrap(), value);
    }
    let result: JobResult = serde_json::from_value(json!({"state":"COMPLETED","queue_wait_ms":2,"outcome":{"kind":"SIGNALED","signal":0,"core_dumped":false}})).unwrap();
    assert!(!result.is_valid());
}

#[test]
fn public_status_never_contains_the_lineage_capability() {
    for value in [
        json!({"state":"QUEUED","sequence":1,"queued_at_ms":10}),
        json!({"state":"RESERVED","sequence":1,"queued_at_ms":10,"granted_at_ms":12,"reservation":{"cpu_millis":2000,"memory_bytes":1024}}),
        json!({"state":"RUNNING","queue_wait_ms":2}),
        json!({"state":"COMPLETED","queue_wait_ms":2,"outcome":{"kind":"EXITED","code":0}}),
        json!({"state":"CANCELLED"}),
        json!({"state":"ABANDONED"}),
        json!({"state":"NOT_FOUND"}),
    ] {
        let status: StatusResult = serde_json::from_value(value.clone()).unwrap();
        assert!(status.is_valid());
        let mut leaked = value;
        leaked["lineage_context"] = json!(CONTEXT);
        assert!(serde_json::from_value::<StatusResult>(leaked).is_err());
    }
    let invalid: StatusResult = serde_json::from_value(json!({"state":"RESERVED","sequence":1,"queued_at_ms":10,"granted_at_ms":9,"reservation":{"cpu_millis":1,"memory_bytes":1}})).unwrap();
    assert!(!invalid.is_valid());
}

#[test]
fn legacy_lifecycle_request_remains_byte_shape_compatible() {
    let old = json!({"action":"CLAIM","job_id":ID,"observer":identity()});
    let legacy: StartRequest = serde_json::from_value(old.clone()).unwrap();
    assert!(legacy.is_valid());
    assert_eq!(serde_json::to_value(legacy).unwrap(), old);
    let mut new_field = old;
    new_field["lineage_context"] = json!(CONTEXT);
    assert!(serde_json::from_value::<StartRequest>(new_field).is_err());
}

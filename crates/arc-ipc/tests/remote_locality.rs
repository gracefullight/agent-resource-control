use arc_ipc::{
    dispatch,
    locality::{classify_response_at, OPERATION_CLASSIFY},
    Request,
};
use serde_json::json;

fn request(kind: serde_json::Value) -> Request {
    let mut request = Request::new("context", OPERATION_CLASSIFY);
    request.payload = json!({"command": kind, "context": {"captured_at_ms": 10000}});
    request
}

#[test]
fn operation_preserves_protocol_one_and_accepts_shell_and_native_argv() {
    for command in [
        json!({"kind":"SHELL","command":"cargo build","shell_kind":"sh"}),
        json!({"kind":"ARGV","argv":["cargo","build"]}),
    ] {
        let response = classify_response_at(&request(command), 10_000);
        assert_eq!(response.protocol_version, 1);
        assert_eq!(response.result.unwrap()["reason"], "CARGO_BUILD");
    }
    let mut old = Request::new("legacy", arc_ipc::OPERATION_JOB_CLASSIFY);
    old.payload = json!({"command":"gh workflow run build.yml"});
    let response = dispatch(&old, || panic!("classification accessed database"));
    assert_eq!(response.result.unwrap()["reason"], "GH_WORKFLOW_RUN");
    old.payload["context"] = json!({"captured_at_ms":10000});
    assert_eq!(
        dispatch(&old, || panic!()).error.unwrap().code,
        arc_ipc::ERROR_INVALID_REQUEST
    );
}

#[test]
fn unsupported_invalid_stale_and_inconsistent_data_never_become_local() {
    let mut q = request(json!({"kind":"SHELL","command":"cargo build","shell_kind":"sh"}));
    let stale = classify_response_at(&q, 15_001);
    assert_eq!(stale.result.unwrap()["classification"], "UNKNOWN");
    q.payload["context"]["facts"] = json!([
        {"kind":"DOCKER_CONTEXT","selector":"prod","endpoints":["unix:///tmp/docker.sock"]},
        {"kind":"DOCKER_CONTEXT","selector":"prod","endpoints":["tcp://192.0.2.1:2376"]}
    ]);
    assert_eq!(
        classify_response_at(&q, 10_000).result.unwrap()["classification"],
        "UNKNOWN"
    );
    q.payload["context"]["surprise"] = json!(true);
    assert_eq!(
        classify_response_at(&q, 10_000).error.unwrap().code,
        arc_ipc::ERROR_INVALID_REQUEST
    );
    q.protocol_version = 2;
    assert_eq!(
        classify_response_at(&q, 10_000).error.unwrap().code,
        arc_ipc::ERROR_PROTOCOL_INCOMPATIBLE
    );
    q.protocol_version = 1;
    q.operation = "job.classify.future".into();
    assert_eq!(
        dispatch(&q, || panic!()).error.unwrap().code,
        arc_ipc::ERROR_UNSUPPORTED_CAPABILITY
    );
}

use super::{protocol, runtime::Fixture, support};
use serde_json::json;

#[test]
fn contextual_and_legacy_classification_reach_the_real_daemon() {
    let _serial = support::serial();
    let fixture = Fixture::new();
    let mut daemon = fixture.spawn("test");
    // A zero timestamp is invalid independently of real-clock scheduling.
    // Fresh/stale boundaries are tested through the injected daemon clock.
    let response = protocol::call(
        &fixture,
        arc_ipc::locality::OPERATION_CLASSIFY,
        json!({"command":{"kind":"ARGV","argv":["cargo","build"]}, "context":{"captured_at_ms":0}}),
    );
    assert_eq!(response.protocol_version, 1);
    assert_eq!(response.status, "ok", "{response:?}");
    assert_eq!(response.result.unwrap()["classification"], "UNKNOWN");
    let response = protocol::call(
        &fixture,
        arc_ipc::locality::OPERATION_CLASSIFY,
        json!({"command":{"kind":"SHELL","command":"gh workflow run build.yml","shell_kind":"sh"},
            "context":{"captured_at_ms":0,"environment":{"GH_HOST":"github.com"},
                "facts":[{"kind":"CI_HOST","selector":"gh","endpoints":["https://github.com"]}]}}),
    );
    assert_eq!(response.status, "ok", "{response:?}");
    assert_eq!(response.result.unwrap()["classification"], "UNKNOWN");
    let response = protocol::call(
        &fixture,
        arc_ipc::OPERATION_JOB_CLASSIFY,
        json!({"command":"gh workflow run build.yml"}),
    );
    assert_eq!(response.result.unwrap()["reason"], "GH_WORKFLOW_RUN");
    daemon.stop(libc::SIGTERM);
    fixture.report();
}
